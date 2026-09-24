//! Whether the relays in the VPNGate / Residential lists really carry traffic.
//!
//! A list can only say that somebody registered a server; only a handshake says
//! whether it works. Startup ranks the relays, `vpngate_sources` assembles the
//! candidate exits, and this module dials them through the winning relay for
//! real: a server that completes a round trip becomes Alive, one that does not
//! becomes Dead, and the ones not reached yet stay Unknown so the UI shows
//! "未测" instead of a guess.
//!
//! **Nothing here runs by itself.** A phone cannot afford a background sweep that
//! dials a hundred volunteer servers (~5s each, measured), so every pass starts
//! from a button: one server, or a whole list.
//!
//! The shape of a sweep is a resource decision: one lane at a time,
//! [`BATCH_SIZE`] servers per lane (a lane is a full mihomo process holding that
//! many OpenVPN configs), a progress beat after every dial, verdicts written back
//! every [`PERSIST_EVERY`] dials, and the whole pass giving up the moment a real
//! tunnel takes the device.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::connection_manager::ConnectionManager;
use crate::managers::lane_core::{Lane, NodeVerdict, LANE_LIVENESS_FIRST, PROBE_204_URL};
use crate::managers::node_manager::NodeManager;
use crate::models::{ConnectionStatus, NodeStatus, ProtocolType, UnifiedNode};

/// Servers probed by one lane. The agreed ceiling for what a phone may spend in
/// the background: 50 configs is the largest set measured to start cleanly.
pub const BATCH_SIZE: usize = 50;

/// Verdicts are stored this often. A dial costs about five seconds, and rewriting
/// a list of hundred-plus OpenVPN configs every five seconds is I/O the flash
/// does not need — so the counter is published per node and written back per ten.
pub const PERSIST_EVERY: usize = 10;

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

/// One progress beat. Emitted per probed server while the sweep runs and once
/// more when it ends, so each tab can show where its own list stands.
#[derive(Debug, Clone, Serialize)]
pub struct LivenessProgress {
    /// The list this beat describes.
    pub group: String,
    /// Servers this sweep has dialled.
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
    /// Whether these verdicts have reached the node store. The UI reloads the
    /// list on this flag rather than on every beat, so a hundred dials do not
    /// mean a hundred full list transfers.
    pub persisted: bool,
}

fn emit_progress(app: &AppHandle, p: &LivenessProgress) {
    let _ = app.emit("nodes:liveness", p);
}

fn beat(group: &str, stats: &GroupStats, running: bool, aborted: bool, persisted: bool) -> LivenessProgress {
    LivenessProgress {
        group: group.to_string(),
        tested: stats.tested,
        total: stats.total,
        alive: stats.alive,
        running,
        done: !running && stats.tested >= stats.total,
        aborted,
        persisted,
    }
}

/// One lane set, one sweep: they bind fixed ports, and a phone cannot afford two
/// mihomo instances probing at once.
static IN_PROGRESS: AtomicBool = AtomicBool::new(false);
/// Lists the user asked to measure while a sweep already held the lanes, one bit
/// per [`LIVENESS_GROUPS`] entry. Each panel has its own button, so a request for
/// the other list must join the running sweep instead of being dropped or
/// restarting this one.
static QUEUED: AtomicU8 = AtomicU8::new(0);

pub struct PassGuard;

impl Drop for PassGuard {
    fn drop(&mut self) {
        IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

fn group_bit(group: &str) -> Option<u8> {
    LIVENESS_GROUPS
        .iter()
        .position(|g| *g == group)
        .map(|i| 1u8 << i)
}

/// Which lists a request covers: one panel's button names its own list, and
/// `None` means every public list.
fn groups_for(scope: Option<&'static str>) -> Vec<&'static str> {
    match scope {
        Some(group) => vec![group],
        None => LIVENESS_GROUPS.to_vec(),
    }
}

/// Take the sweep, or queue the requested lists behind the one already running.
fn claim(groups: &[&'static str]) -> Option<PassGuard> {
    if IN_PROGRESS.swap(true, Ordering::SeqCst) {
        for group in groups {
            if let Some(bit) = group_bit(group) {
                QUEUED.fetch_or(bit, Ordering::SeqCst);
            }
        }
        return None;
    }
    Some(PassGuard)
}

/// Drain the lists that arrived while the sweep was busy.
fn take_queued() -> Vec<&'static str> {
    let bits = QUEUED.swap(0, Ordering::SeqCst);
    LIVENESS_GROUPS
        .iter()
        .enumerate()
        .filter(|(i, _)| bits & (1u8 << *i) != 0)
        .map(|(_, group)| *group)
        .collect()
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

/// Is `group` a list this module can measure, as a `'static` name?
pub fn known_group(group: &str) -> Option<&'static str> {
    LIVENESS_GROUPS.iter().find(|g| **g == group).copied()
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

/// The relay, core binary and writable dir a probe lane needs.
struct LaneSetup {
    binary: PathBuf,
    dir: PathBuf,
    relay_yaml: String,
    relay_name: String,
}

fn prepare_lane(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    group: &str,
) -> Result<LaneSetup, String> {
    let relay = node_manager
        .get_best_relay_node(preferred_id)
        .ok_or_else(|| format!("liveness {group}: 没有可用的中转节点"))?;
    let relay_yaml = ConnectionManager::format_mihomo_relay_proxy(&relay, "relay")
        .ok_or_else(|| format!("liveness {group}: 中转节点 {} 无法用 mihomo 表达", relay.name))?;
    Ok(LaneSetup {
        binary: conn.locate_binary(app, "mihomo")?,
        dir: app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app_data_dir: {}", e))?,
        relay_yaml,
        relay_name: relay.name.clone(),
    })
}

/// Dial every server of one list that has no fresh verdict, publishing as it goes.
///
/// Returns `Err` only when another sweep already holds the lanes; the requested
/// lists were queued, so they are measured by that sweep instead.
pub async fn run_pass(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    scope: Option<&'static str>,
) -> Result<(), String> {
    let wanted = groups_for(scope);
    let _guard = match claim(&wanted) {
        Some(g) => g,
        None => return Err("测活正在进行中,请等待当前一轮结束".to_string()),
    };

    let mut queue = wanted;
    while !queue.is_empty() {
        let groups = std::mem::take(&mut queue);
        let mut yielded = false;
        for group in groups {
            match probe_group(app, node_manager, conn, preferred_id, group).await {
                Ok(Some(stats)) => yielded = stats.aborted,
                // Nothing to probe, or no relay to dial through: no events, so the
                // list keeps showing 未测 instead of a fabricated verdict.
                Ok(None) => {}
                Err(e) => log::warn!("liveness {group}: {e}"),
            }
            if yielded {
                break;
            }
            queue.extend(take_queued());
        }
        if yielded {
            let dropped = take_queued();
            if !dropped.is_empty() {
                log::info!("liveness: 已让路给真实隧道,放弃排队的 {:?}", dropped);
            }
            break;
        }
    }
    Ok(())
}

/// Dial one server the user pointed at, and store its verdict.
pub async fn measure_one(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    node_id: &str,
    preferred_id: Option<&str>,
) -> Result<UnifiedNode, String> {
    let node = node_manager
        .get_all()
        .into_iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "该节点已不在清单中,请点同步重新采集".to_string())?;
    let group = known_group(&node.group).ok_or("该名单不需要真连接测活")?;
    if node.protocol != ProtocolType::Openvpn {
        return Err("只有 OpenVPN 节点支持真连接测活".to_string());
    }
    let _guard = match claim(&[group]) {
        Some(g) => g,
        None => return Err("测活正在进行中,请等待当前一轮结束".to_string()),
    };

    let setup = prepare_lane(app, node_manager, conn, preferred_id, group)?;
    let name = "p0".to_string();
    let blocks = vec![(
        name.clone(),
        ConnectionManager::mihomo_openvpn_proxy_block(&node, &name, Some("relay")),
    )];
    let (b, d, r) = (setup.binary.clone(), setup.dir.clone(), setup.relay_yaml.clone());
    let lane = tokio::task::spawn_blocking(move || {
        Lane::start(LANE_LIVENESS_FIRST, &d, &b, Some(r.as_str()), &blocks)
    })
    .await
    .map_err(|e| format!("测活任务被中断: {}", e))?
    .map_err(|e| format!("测活核心未能启动: {}", e))?;

    let verdict = if lane.group_members().await.iter().any(|m| m == &name) {
        lane.test_node(&name, PROBE_204_URL).await
    } else {
        return Err("核心拒绝了该节点的 OpenVPN 配置,无法判定".to_string());
    };
    drop(lane);

    let measured = record_verdict(&node, &verdict, chrono::Utc::now().timestamp());
    node_manager
        .update_nodes(std::slice::from_ref(&measured))
        .map_err(|e| format!("测活结论未能保存: {}", e))?;
    log::info!(
        "liveness {group}: {}:{} 单节点测活 → {:?}",
        node.address,
        node.port,
        measured.status
    );
    Ok(measured)
}

/// Dial one list through the preferred relay. `Ok(None)` means this group had
/// nothing to probe or no way to reach the internet through a relay.
async fn probe_group(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    group: &'static str,
) -> Result<Option<GroupStats>, String> {
    let rows = liveness_candidates(&node_manager.get_all(), group);
    if rows.is_empty() {
        return Ok(None);
    }
    let setup = match prepare_lane(app, node_manager, conn, preferred_id, group) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("{e}, 本次跳过");
            return Ok(None);
        }
    };

    let mut stats = GroupStats {
        total: rows.len(),
        ..Default::default()
    };
    emit_progress(app, &beat(group, &stats, true, false, false));
    log::info!(
        "liveness {group}: dialling {} servers through {} in batches of {}",
        rows.len(),
        setup.relay_name,
        BATCH_SIZE
    );

    let now = chrono::Utc::now().timestamp();
    for (batch_no, chunk) in rows.chunks(BATCH_SIZE).enumerate() {
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
        let (b, d, r, bl) = (setup.binary.clone(), setup.dir.clone(), setup.relay_yaml.clone(), blocks);
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

        let batch_start = std::time::Instant::now();
        let alive_before_batch = stats.alive;
        let mut pending: Vec<UnifiedNode> = Vec::with_capacity(targets.len());
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
            pending.push(record_verdict(node, &verdict, now));
            // Publish every dial so the count visibly moves — a dead server
            // costs about five seconds, so a batch of fifty is minutes of
            // otherwise silent work. Store only every PERSIST_EVERY dials, and
            // say so in the beat so the UI reloads the list just then.
            let persisted = pending.len() >= PERSIST_EVERY && {
                flush(node_manager, group, &mut pending)
            };
            emit_progress(app, &beat(group, &stats, true, false, persisted));
        }
        // One whole batch with no reachable server is not bad luck at fifty
        // volunteer relays — it is the dial path failing, and the core's own log
        // is the only witness. This app's phone diagnostic channel is its log.
        let core_tail = if stats.alive == alive_before_batch {
            lane.tail_log(20)
        } else {
            String::new()
        };
        drop(lane);

        if !core_tail.is_empty() {
            log::warn!(
                "liveness {group}: batch {} dialled {} servers, none connected — core log tail: {}",
                batch_no,
                targets.len(),
                core_tail
            );
        }

        let stored = flush(node_manager, group, &mut pending);
        emit_progress(app, &beat(group, &stats, true, false, stored));
        log::info!(
            "liveness {group}: batch {} of {} servers in {:.0}s ({}/{} measured)",
            batch_no,
            chunk.len(),
            batch_start.elapsed().as_secs_f64(),
            stats.tested,
            stats.total
        );
        if stats.aborted {
            break;
        }
    }

    emit_progress(app, &beat(group, &stats, false, stats.aborted, true));
    log::info!(
        "liveness {group}: {}/{} servers reachable{}",
        stats.alive,
        stats.total,
        if stats.aborted { " (sweep stopped early)" } else { "" }
    );
    Ok(Some(stats))
}

/// Write the verdicts gathered so far back to the store. Returns whether the
/// pending set is on disk; a failed write leaves them queued so the next flush
/// retries them instead of dropping those conclusions.
fn flush(node_manager: &NodeManager, group: &str, pending: &mut Vec<UnifiedNode>) -> bool {
    if pending.is_empty() {
        return true;
    }
    match node_manager.update_nodes(pending) {
        Ok(()) => {
            pending.clear();
            true
        }
        Err(e) => {
            log::warn!("liveness {group}: verdicts not stored: {}", e);
            false
        }
    }
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
        assert_eq!(known_group("residential"), None, "名单名区分大小写");
        assert_eq!(known_group("VPNGate"), Some("VPNGate"));
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
        assert_eq!(BATCH_SIZE % PERSIST_EVERY, 0, "a batch ends on a write boundary");
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
    fn one_sweep_at_a_time_and_a_request_for_another_list_is_queued() {
        let held = claim(&["VPNGate"]).expect("the first caller owns the sweep");
        assert!(take_queued().is_empty(), "nothing queued yet");

        // The other panel has its own button; pressing it mid-sweep must not
        // restart this one, and must not be dropped either.
        assert!(claim(&["Residential"]).is_none(), "a second sweep cannot share the lanes");
        assert_eq!(take_queued(), vec!["Residential"]);
        assert!(take_queued().is_empty(), "draining clears the request");

        assert!(claim(&["VPNGate", "Residential"]).is_none());
        assert_eq!(take_queued(), LIVENESS_GROUPS.to_vec(), "一次排队可以攒下两份名单");

        drop(held);
        assert!(claim(&["VPNGate"]).is_some(), "releasing the sweep lets the next one in");
        assert_eq!(take_queued(), Vec::<&str>::new());
    }

    #[test]
    fn the_terminal_beat_says_done_only_when_the_list_is_finished() {
        let half = GroupStats { tested: 50, total: 100, alive: 9, aborted: false };
        let running = beat("VPNGate", &half, true, false, true);
        assert!(running.running && !running.done, "还在拨下一批");

        let done = GroupStats { tested: 100, total: 100, alive: 19, aborted: false };
        let stopped = beat("VPNGate", &done, false, false, true);
        assert!(stopped.done && !stopped.aborted);

        let early = GroupStats { tested: 30, total: 100, alive: 4, aborted: true };
        let yielded = beat("Residential", &early, false, true, true);
        assert!(!yielded.done, "提前结束不等于全部测完");
        assert!(yielded.aborted);
    }
}
