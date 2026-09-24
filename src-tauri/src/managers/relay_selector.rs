//! Which node should carry everyone else's traffic?
//!
//! Until now the answer was fabricated: `NodeManager::get_relay_candidates`
//! pushed any listening 127.0.0.1 proxy port to the front with a hard-coded
//! "1ms / 100Mbps", and every other candidate was ranked by a TCP handshake
//! ping — a number that says nothing about whether the node tunnels traffic,
//! let alone how fast. The chain then dialed whatever that list said first,
//! which is why "auto" relay selection looked random at best.
//!
//! This module measures instead: a relay candidate is asked to carry a real
//! HTTPS round trip (latency) and a 2MB download (bandwidth) through a probe
//! lane, and the winner is written back onto the node record plus
//! `settings.preferred_relay_id`. The Hong Kong fleet is tried first because
//! it is the subset that is realistically both fast and fat — testing it first
//! stops the search from wandering through half the subscription.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::connection_manager::ConnectionManager;
use crate::managers::lane_core::{Lane, PROBE_204_URL, THROUGHPUT_URL};
use crate::managers::node_manager::NodeManager;
use crate::models::{NodeStatus, UnifiedNode};

/// Candidates that get a real dial. Each one is bounded by the lane's dial
/// timeout, so this also bounds how long startup preference can take.
pub const RANK_LIMIT: usize = 10;
/// Of the measured candidates, the quickest few also get a bandwidth sample.
pub const SPEED_TOP: usize = 3;
/// Ranking owns lane 0; the node-liveness lanes use 1 and up.
const RANK_LANE_INDEX: usize = 0;

/// One ranking at a time: they bind a fixed port pair, and a second concurrent
/// pass would just measure the same candidates twice on a phone.
static RANKING_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct RankingGuard;

impl Drop for RankingGuard {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        RANKING_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

/// One candidate plus whatever the lane actually observed.
#[derive(Debug, Clone)]
pub struct Measured {
    pub node: UnifiedNode,
    pub latency_ms: Option<i64>,
    pub speed_bps: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelayRanking {
    pub preferred_id: Option<String>,
    pub preferred_name: Option<String>,
    pub latency_ms: Option<i64>,
    pub speed_bps: Option<u64>,
    /// How many candidates were really dialled.
    pub tested: usize,
    /// True when a real tunnel came up mid-run and cut the ranking short.
    pub aborted: bool,
}

/// Hong Kong first, then any local proxy, then the rest in the order the
/// caller gave us. Truncated to [`RANK_LIMIT`].
pub fn order_relay_candidates(candidates: Vec<UnifiedNode>) -> Vec<UnifiedNode> {
    // sort_by_key is stable, so neither the caller's priority order nor the
    // original order inside a class gets shuffled.
    let mut ordered = candidates;
    ordered.sort_by_key(rank_class);
    ordered.truncate(RANK_LIMIT);
    ordered
}

/// Why Hong Kong leads: it is the subset of a subscription that is realistically
/// both low-latency and high-bandwidth from this network, so trying it first
/// keeps the search from wandering through the whole fleet.
fn rank_class(n: &UnifiedNode) -> u8 {
    if n.country_code.eq_ignore_ascii_case("HK") {
        0
    } else if n.group == "LocalProxy" {
        1
    } else {
        2
    }
}

/// Pick the winner from measured candidates: the fastest [`SPEED_TOP`] by
/// latency, and among those the one with the most bandwidth. A node that never
/// answered is out, whatever its stored numbers claim.
pub fn choose_winner(measured: &[Measured]) -> Option<usize> {
    let mut alive: Vec<usize> = (0..measured.len())
        .filter(|&i| measured[i].latency_ms.is_some())
        .collect();
    if alive.is_empty() {
        return None;
    }
    alive.sort_by_key(|&i| measured[i].latency_ms.unwrap_or(i64::MAX));

    let contenders = &alive[..alive.len().min(SPEED_TOP)];
    let any_speed = contenders.iter().any(|&i| measured[i].speed_bps.unwrap_or(0) > 0);
    if !any_speed {
        return Some(contenders[0]);
    }
    // contenders is already latency-ascending, and a strict `>` keeps the earlier
    // index on ties: same bandwidth, faster node wins.
    let mut best = contenders[0];
    for &i in contenders {
        if measured[i].speed_bps.unwrap_or(0) > measured[best].speed_bps.unwrap_or(0) {
            best = i;
        }
    }
    Some(best)
}

/// Measure the shortlist and return the winner, writing the observed latency
/// and bandwidth back onto each node record.
///
/// Two things can stop it early, and both are deliberate: another ranking is
/// already running (they own a fixed port pair), or the user opened a real
/// tunnel — on a phone a second core is not a cost worth paying while traffic
/// is flowing.
pub async fn select_preferred_relay(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    candidates: Vec<UnifiedNode>,
) -> Result<RelayRanking, String> {
    if RANKING_IN_PROGRESS.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("中转优选已在进行中".to_string());
    }
    let _guard = RankingGuard;

    let shortlist = order_relay_candidates(candidates);

    let mut lane_nodes: Vec<(String, String)> = Vec::with_capacity(shortlist.len());
    let mut indexed: Vec<(String, UnifiedNode)> = Vec::with_capacity(shortlist.len());
    for (i, node) in shortlist.into_iter().enumerate() {
        let name = format!("r{}", i);
        match ConnectionManager::format_mihomo_relay_proxy(&node, &name) {
            Some(block) => {
                lane_nodes.push((name.clone(), block));
                indexed.push((name, node));
            }
            None => log::warn!(
                "relay ranking: {} ({}) has no mihomo representation, skipped",
                node.name,
                node.id
            ),
        }
    }

    let mut measured: Vec<Measured> = Vec::with_capacity(indexed.len());
    let mut aborted = false;

    if indexed.is_empty() {
        log::warn!("relay ranking: no candidate can be expressed in mihomo, nothing to measure");
    } else {
        let binary = conn.locate_binary(app, "mihomo")?;
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app_data_dir: {}", e))?;
        let members = lane_nodes.clone();
        let lane = tokio::task::spawn_blocking(move || {
            Lane::start(RANK_LANE_INDEX, &dir, &binary, None, &members)
        })
        .await
        .map_err(|e| format!("ranking lane join: {}", e))??;

        for (name, node) in &indexed {
            if conn.get_status() != crate::models::ConnectionStatus::Disconnected {
                log::info!(
                    "relay ranking: stopped after {} candidates, a real tunnel owns the device now",
                    measured.len()
                );
                aborted = true;
                break;
            }
            let latency = lane.measure_member(name, PROBE_204_URL).await;
            let speed = if latency.is_some() {
                lane.measure_member_throughput(name, THROUGHPUT_URL).await
            } else {
                None
            };
            log::info!(
                "relay ranking: {} -> {} ms, {} B/s",
                node.name,
                latency.map(|l| l.to_string()).unwrap_or_else(|| "dead".to_string()),
                speed.map(|s| s.to_string()).unwrap_or_else(|| "-".to_string())
            );
            measured.push(Measured {
                node: node.clone(),
                latency_ms: latency,
                speed_bps: speed,
            });
        }
        drop(lane);

        persist_measurements(node_manager, &measured)?;
    }

    let winner = choose_winner(&measured).map(|i| &measured[i]);
    Ok(RelayRanking {
        preferred_id: winner.map(|m| m.node.id.clone()),
        preferred_name: winner.map(|m| m.node.name.clone()),
        latency_ms: winner.and_then(|m| m.latency_ms),
        speed_bps: winner.and_then(|m| m.speed_bps),
        tested: measured.len(),
        aborted,
    })
}

fn persist_measurements(node_manager: &NodeManager, measured: &[Measured]) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp();
    for m in measured {
        let mut node = m.node.clone();
        node.latency_ms = m.latency_ms;
        node.speed_bps = m.speed_bps;
        node.status = if m.latency_ms.is_some() {
            NodeStatus::Alive
        } else {
            NodeStatus::Dead
        };
        node.last_checked = Some(now);
        node_manager.update_node(node)?;
    }
    node_manager.save()
}

/// Emit the outcome for the UI (the relay bar shows who won and how it scored).
pub fn emit_ranking(app: &AppHandle, ranking: &RelayRanking) {
    let _ = app.emit("relay:preferred", ranking);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ProtocolType;
    use serde_json::json;

    fn node(id: &str, cc: &str, group: &str, latency: Option<i64>) -> UnifiedNode {
        UnifiedNode {
            id: id.to_string(),
            name: id.to_string(),
            protocol: ProtocolType::Vless,
            address: "1.2.3.4".to_string(),
            port: 443,
            country_code: cc.to_string(),
            country_name: String::new(),
            city: String::new(),
            group: group.to_string(),
            tags: vec![],
            favorite: false,
            latency_ms: latency,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({ "uuid": "u", "security": "tls" }),
        }
    }

    fn measured(latency: Option<i64>, speed: Option<u64>) -> Measured {
        Measured {
            node: node("x", "HK", "Default", latency),
            latency_ms: latency,
            speed_bps: speed,
        }
    }

    #[test]
    fn hk_and_local_candidates_are_tried_before_the_rest() {
        let list = vec![
            node("de", "DE", "Default", Some(30)),
            node("local", "LOCAL", "LocalProxy", Some(1)),
            node("hk2", "HK", "Default", Some(400)),
            node("jp", "JP", "Default", Some(50)),
            node("hk1", "HK", "Default", Some(900)),
        ];
        let ordered = order_relay_candidates(list);
        let ids: Vec<&str> = ordered.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(&ids[..3], &["hk2", "hk1", "local"], "HK first, then local, stable within a class: {:?}", ids);
    }

    #[test]
    fn candidate_list_is_capped() {
        let list: Vec<UnifiedNode> = (0..40).map(|i| node(&format!("n{}", i), "US", "Default", Some(i as i64))).collect();
        assert_eq!(order_relay_candidates(list).len(), RANK_LIMIT);
    }

    #[test]
    fn a_faster_but_skinnier_node_does_not_win() {
        // 20ms/2Mbps vs 40ms/80Mbps vs 60ms/500Mbps — all three are inside the
        // latency shortlist, so bandwidth decides.
        let m = vec![
            measured(Some(20), Some(2_000_000)),
            measured(Some(40), Some(80_000_000)),
            measured(Some(60), Some(500_000_000)),
        ];
        assert_eq!(choose_winner(&m), Some(2));
    }

    #[test]
    fn bandwidth_only_compares_the_latency_leaders() {
        // A 900ms node with huge pipes must not beat a 30ms node: it never
        // reaches the contenders.
        let m = vec![
            measured(Some(30), Some(5_000_000)),
            measured(Some(40), Some(6_000_000)),
            measured(Some(50), Some(6_000_000)),
            measured(Some(900), Some(900_000_000)),
        ];
        assert_eq!(choose_winner(&m), Some(1));
    }

    #[test]
    fn dead_candidates_never_win_on_stored_bandwidth() {
        let m = vec![
            measured(None, Some(900_000_000)),
            measured(Some(300), None),
        ];
        assert_eq!(choose_winner(&m), Some(1));
    }

    #[test]
    fn all_dead_means_no_winner() {
        let m = vec![measured(None, None), measured(None, None)];
        assert_eq!(choose_winner(&m), None);
    }
}
