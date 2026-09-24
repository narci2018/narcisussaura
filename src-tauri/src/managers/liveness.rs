//! Whether the relays in the VPNGate / Residential lists really carry traffic.
//!
//! A list can only say that somebody registered a server; only a handshake says
//! whether it works. Startup ranks the relays, `vpngate_sources` assembles the
//! candidate exits, and this module dials every one of them through the winning
//! relay for real: a server that completes a round trip becomes Alive, one that
//! does not becomes Dead, and the ones not reached yet stay Unknown so the UI
//! shows "未测" instead of a guess.
//!
//! The shape of the sweep is a resource decision, not an implementation detail:
//! one lane at a time, [`BATCH_SIZE`] servers per lane (a lane is a full mihomo
//! process holding that many OpenVPN configs), verdicts written back and
//! published after each batch so the list fills in progressively, and the whole
//! pass gives up the moment a real tunnel takes the device.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::connection_manager::ConnectionManager;
use crate::managers::lane_core::{Lane, NodeVerdict, LANE_LIVENESS_FIRST, PROBE_204_URL};
use crate::managers::node_manager::NodeManager;
use crate::models::{ConnectionStatus, NodeStatus, ProtocolType, UnifiedNode};

/// Servers probed by one lane. The agreed ceiling for what a phone may spend in
/// the background: 50 configs is the largest set measured to start cleanly.
pub const BATCH_SIZE: usize = 50;

/// Lists made of other people's volunteer servers, where "registered" says
/// nothing about "working".
pub const LIVENESS_GROUPS: [&str; 2] = ["VPNGate", "Residential"];

/// Batches rotate over a few lane indices instead of reusing one: a listener the
/// previous batch tore down can still hold its port for a moment, and on a phone
/// "lane 2 did not start" is undiagnosable after the fact.
const LANE_WINDOW: usize = 4;

fn lane_index_for(batch: usize) -> usize {
    LANE_LIVENESS_FIRST + batch % LANE_WINDOW
}

/// One progress beat. Emitted per batch while the sweep runs and once more when
/// it ends, so each tab can show where its own list stands.
#[derive(Debug, Clone, Serialize)]
pub struct LivenessProgress {
    /// The list this beat describes.
    pub group: String,
    pub tested: usize,
    pub total: usize,
    pub alive: usize,
    /// The sweep as a whole is still working.
    pub running: bool,
    /// Nothing more will be learned about this group in this sweep.
    pub done: bool,
    /// The sweep stopped before the end: a real tunnel took the device, or no
    /// lane could be started. Never "the remaining servers are dead".
    pub aborted: bool,
}

fn emit_progress(app: &AppHandle, p: &LivenessProgress) {
    let _ = app.emit("nodes:liveness", p);
}

/// One lane set, one sweep: they bind fixed ports, and a phone cannot afford two
/// mihomo instances probing at once.
static IN_PROGRESS: AtomicBool = AtomicBool::new(false);
/// Set when someone asks for a pass while one is already running — the running
/// sweep redoes the lists from scratch, which is what a manual node update wants.
static RERUN_REQUESTED: AtomicBool = AtomicBool::new(false);

pub struct PassGuard;

impl Drop for PassGuard {
    fn drop(&mut self) {
        IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

fn claim() -> Option<PassGuard> {
    if IN_PROGRESS.swap(true, Ordering::SeqCst) {
        RERUN_REQUESTED.store(true, Ordering::SeqCst);
        return None;
    }
    Some(PassGuard)
}

/// Rows of one list that a lane can actually dial. Anything that is not OpenVPN
/// has no representation in a probe lane, and skipping it must not read as
/// "this server is dead" — it keeps whatever verdict it already had.
pub fn liveness_candidates(all: &[UnifiedNode], group: &str) -> Vec<UnifiedNode> {
    all.iter()
        .filter(|n| n.group == group && n.protocol == ProtocolType::Openvpn)
        .cloned()
        .collect()
}

/// Write one handshake outcome back onto a node record. Bandwidth is left
/// alone: this pass answers "does it connect", not "how fast".
pub fn record_verdict(node: &UnifiedNode, verdict: &NodeVerdict, now: i64) -> UnifiedNode {
    let mut measured = node.clone();
    measured.status = if verdict.alive {
        NodeStatus::Alive
    } else {
        NodeStatus::Dead
    };
    measured.latency_ms = verdict.latency_ms;
    measured.last_checked = Some(now);
    measured
}

/// What one group's sweep got through.
#[derive(Debug, Clone, Default)]
struct GroupStats {
    tested: usize,
    total: usize,
    alive: usize,
    aborted: bool,
}

/// Run the whole sweep: every list, in batches, publishing as it goes.
///
/// Returns `Err` only when another sweep already holds the lanes — that one has
/// been told to redo these lists, so the caller's request is not lost.
pub async fn run_pass(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
) -> Result<(), String> {
    let _guard = match claim() {
        Some(g) => g,
        None => return Err("节点测活已在进行中，本次更新将在当前一轮结束后自动重测".to_string()),
    };

    let mut last: HashMap<&'static str, GroupStats> = HashMap::new();
    loop {
        RERUN_REQUESTED.store(false, Ordering::SeqCst);
        last.clear();

        let mut stopped = false;
        for group in LIVENESS_GROUPS {
            match probe_group(app, node_manager, conn, preferred_id, group).await {
                Ok(Some(stats)) => {
                    stopped = stats.aborted;
                    last.insert(group, stats);
                }
                // Nothing to probe, or no relay to dial through: no events, so
                // the list keeps showing 未测 instead of a fabricated verdict.
                Ok(None) => {}
                Err(e) => log::warn!("liveness {group}: {e}"),
            }
            if stopped || RERUN_REQUESTED.load(Ordering::SeqCst) {
                break;
            }
        }

        if stopped || !RERUN_REQUESTED.load(Ordering::SeqCst) {
            break;
        }
        log::info!("liveness: node lists changed mid-sweep, measuring them again");
    }

    for (group, stats) in &last {
        emit_progress(
            app,
            &LivenessProgress {
                group: group.to_string(),
                tested: stats.tested,
                total: stats.total,
                alive: stats.alive,
                running: false,
                done: stats.tested >= stats.total,
                aborted: stats.aborted,
            },
        );
    }
    Ok(())
}

/// Dial one list through the preferred relay. `Ok(None)` means this group had
/// nothing to probe or no way to reach the internet through a relay.
async fn probe_group(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    group: &str,
) -> Result<Option<GroupStats>, String> {
    let nodes = liveness_candidates(&node_manager.get_all(), group);
    if nodes.is_empty() {
        return Ok(None);
    }

    let relay = match node_manager.get_best_relay_node(preferred_id) {
        Some(r) => r,
        None => {
            log::warn!("liveness {group}: no relay candidate to dial through, skipped");
            return Ok(None);
        }
    };
    let relay_yaml = match ConnectionManager::format_mihomo_relay_proxy(&relay, "relay") {
        Some(y) => y,
        None => {
            log::warn!(
                "liveness {group}: relay {} cannot be expressed in mihomo, skipped",
                relay.name
            );
            return Ok(None);
        }
    };
    let binary = conn.locate_binary(app, "mihomo")?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {}", e))?;

    let total = nodes.len();
    emit_progress(
        app,
        &LivenessProgress {
            group: group.to_string(),
            tested: 0,
            total,
            alive: 0,
            running: true,
            done: false,
            aborted: false,
        },
    );

    let mut stats = GroupStats {
        total,
        ..Default::default()
    };
    log::info!(
        "liveness {group}: dialling {} servers through {} in batches of {}",
        total,
        relay.name,
        BATCH_SIZE
    );

    for (batch_no, chunk) in nodes.chunks(BATCH_SIZE).enumerate() {
        if tunnel_took_over(conn) {
            stats.aborted = true;
            break;
        }

        let mut blocks: Vec<(String, String)> = Vec::with_capacity(chunk.len());
        let mut targets: Vec<(String, &UnifiedNode)> = Vec::with_capacity(chunk.len());
        for (i, node) in chunk.iter().enumerate() {
            let name = format!("p{}", i);
            blocks.push((
                name.clone(),
                ConnectionManager::mihomo_openvpn_proxy_block(node, &name, Some("relay")),
            ));
            targets.push((name, node));
        }

        let index = lane_index_for(batch_no);
        let (b, d, r, bl) = (binary.clone(), dir.clone(), relay_yaml.clone(), blocks);
        let lane = match tokio::task::spawn_blocking(move || {
            Lane::start(index, &d, &b, Some(r.as_str()), &bl)
        })
        .await
        {
            Ok(Ok(lane)) => lane,
            Ok(Err(e)) => {
                // These servers stay Unknown: a core that will not start says
                // nothing about them.
                log::warn!("liveness {group}: lane {} did not start: {}", index, e);
                stats.aborted = true;
                break;
            }
            Err(e) => {
                log::warn!("liveness {group}: lane {} join failed: {}", index, e);
                stats.aborted = true;
                break;
            }
        };

        let accepted = lane.group_members().await;
        if accepted.len() < targets.len() + 1 {
            log::warn!(
                "liveness {group}: lane {} took {} of {} configs, the rest read as dead",
                index,
                accepted.len().saturating_sub(1),
                targets.len()
            );
        }

        let now = chrono::Utc::now().timestamp();
        let mut measured: Vec<UnifiedNode> = Vec::with_capacity(targets.len());
        for (name, node) in &targets {
            if tunnel_took_over(conn) {
                stats.aborted = true;
                break;
            }
            let verdict = lane.test_node(name, PROBE_204_URL).await;
            if verdict.alive {
                stats.alive += 1;
            }
            stats.tested += 1;
            measured.push(record_verdict(node, &verdict, now));
        }
        drop(lane);

        if let Err(e) = node_manager.update_nodes(&measured) {
            log::warn!("liveness {group}: verdicts not stored: {}", e);
        }
        emit_progress(
            app,
            &LivenessProgress {
                group: group.to_string(),
                tested: stats.tested,
                total,
                alive: stats.alive,
                running: true,
                done: false,
                aborted: false,
            },
        );
        if stats.aborted {
            break;
        }
    }

    log::info!(
        "liveness {group}: {}/{} servers reachable{}",
        stats.tested,
        total,
        if stats.aborted { " (sweep stopped early)" } else { "" }
    );
    Ok(Some(stats))
}

/// A probe burst must never compete with a tunnel the user actually opened.
fn tunnel_took_over(conn: &ConnectionManager) -> bool {
    conn.get_status() != ConnectionStatus::Disconnected
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, group: &str, protocol: ProtocolType, status: NodeStatus) -> UnifiedNode {
        UnifiedNode {
            id: id.to_string(),
            name: id.to_string(),
            protocol,
            address: "1.2.3.4".to_string(),
            port: 443,
            country_code: "JP".to_string(),
            country_name: "Japan".to_string(),
            city: String::new(),
            group: group.to_string(),
            tags: vec![],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status,
            config: json!({ "ca": "AAAA", "proto": "tcp" }),
        }
    }

    #[test]
    fn only_the_groups_openvpn_servers_are_dialled() {
        let all = vec![
            node("v1", "VPNGate", ProtocolType::Openvpn, NodeStatus::Unknown),
            node("v2", "VPNGate", ProtocolType::Vless, NodeStatus::Unknown),
            node("r1", "Residential", ProtocolType::Openvpn, NodeStatus::Unknown),
            node("d1", "Default", ProtocolType::Openvpn, NodeStatus::Unknown),
        ];
        assert_eq!(
            liveness_candidates(&all, "VPNGate")
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>(),
            vec!["v1"],
            "a non-openvpn row has no lane representation and must not be judged"
        );
        assert_eq!(
            liveness_candidates(&all, "Residential")
                .iter()
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>(),
            vec!["r1"],
            "别的组的服务器不会混进这一份名单"
        );
        assert!(
            !LIVENESS_GROUPS.contains(&"Default"),
            "订阅节点由中转优选那条链路负责,不在真连接测活范围内"
        );
    }

    #[test]
    fn a_handshake_decides_the_label() {
        let alive = record_verdict(
            &node("a", "VPNGate", ProtocolType::Openvpn, NodeStatus::Unknown),
            &NodeVerdict { alive: true, latency_ms: Some(240) },
            1_700_000_000,
        );
        assert_eq!(alive.status, NodeStatus::Alive);
        assert_eq!(alive.latency_ms, Some(240));
        assert_eq!(alive.last_checked, Some(1_700_000_000));

        let dead = record_verdict(
            &node("b", "VPNGate", ProtocolType::Openvpn, NodeStatus::Unknown),
            &NodeVerdict { alive: false, latency_ms: None },
            1_700_000_000,
        );
        assert_eq!(dead.status, NodeStatus::Dead);
        assert_eq!(dead.latency_ms, None, "no round trip, no latency to show");

        assert_eq!(BATCH_SIZE, 50, "the agreed mobile page size");
    }

    #[test]
    fn batches_rotate_lane_indices_without_leaving_the_window() {
        let indices: Vec<usize> = (0..(LANE_WINDOW * 3 + 1)).map(lane_index_for).collect();
        assert!(
            indices
                .iter()
                .all(|i| *i >= LANE_LIVENESS_FIRST && *i < LANE_LIVENESS_FIRST + LANE_WINDOW),
            "{indices:?}"
        );
        // Consecutive batches never share an index, so a port the previous core
        // has not finished releasing cannot block the next batch.
        for pair in indices.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
        assert_eq!(lane_index_for(0), LANE_LIVENESS_FIRST);
    }

    #[test]
    fn one_sweep_at_a_time_and_a_late_request_is_queued() {
        let held = claim().expect("the first caller owns the sweep");
        assert!(!RERUN_REQUESTED.load(Ordering::SeqCst), "nothing queued yet");

        assert!(claim().is_none(), "a second sweep must not open a second lane set");
        assert!(
            RERUN_REQUESTED.load(Ordering::SeqCst),
            "the refused caller's lists become the running sweep's job"
        );

        drop(held);
        assert!(claim().is_some(), "releasing the sweep lets the next one in");
        RERUN_REQUESTED.store(false, Ordering::SeqCst);
    }
}
