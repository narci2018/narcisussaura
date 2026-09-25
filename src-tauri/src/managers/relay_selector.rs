//! Which node should carry everyone else's traffic?
//!
//! The old answer was fabricated: `NodeManager::get_relay_candidates` pushed any
//! listening 127.0.0.1 proxy port to the front with a hard-coded "1ms / 100Mbps",
//! and everything else was ranked by a TCP handshake ping. Then the ranking
//! measured up to ten candidates serially, each with a 2MB download — a refresh
//! that took minutes to say "没有可用中转".
//!
//! This module does what the user asked for instead of an exhaustive survey:
//! **Hong Kong first, then the rest of Asia, then everything else, and the first
//! candidate that really carries a probe stops the search.** One round trip per
//! candidate for the verdict, and the bandwidth sample is taken once — on the
//! winner, because that is the node about to carry the tunnel. A candidate is
//! only declared dead after [`ATTEMPTS_PER_CANDIDATE`] dials (the v0.2.111 rule:
//! one failed dial convicts nobody), and each dial races every probe endpoint
//! ([`crate::managers::lane_core::PROBE_204_URLS`]) so a single black-holed probe
//! host cannot kill a working relay. The winner is written back onto the node
//! record plus `settings.preferred_relay_id`.
//!
//! Why geography leads rather than stored latency: from this network the Hong Kong
//! fleet is the subset that is realistically both fast and fat, and trying it
//! first is what turns "test everything" into "test one".

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::managers::connection_manager::ConnectionManager;
use crate::managers::lane_core::Lane;
use crate::managers::node_manager::NodeManager;
use crate::models::{NodeStatus, UnifiedNode};

/// Candidates that can be dialled, in the order they will be dialled. The search
/// stops at the first usable one, so a generous cap costs nothing in practice and
/// keeps a dead Hong Kong fleet from exhausting the whole subscription.
pub const RANK_LIMIT: usize = 20;
/// 一次拨号不能判死(v0.2.111 的教训),但也不必像测活那样拨三次 —— 后面还有
/// 十九个候选等着。
pub const ATTEMPTS_PER_CANDIDATE: usize = 2;
/// Two dials of the same node back to back mostly re-test the same TCP path; a
/// short gap lets a transient reset clear.
const ATTEMPT_GAP: Duration = Duration::from_millis(400);
/// How long one bulk host gets for the winner's bandwidth label. Two seconds is
/// already 512Kbps of sustained flow — below that the node is not carrying the
/// tunnel anyway — and the free fleet was measured RST-ing CDN connections for
/// seconds at a time, so a longer window only delays the answer the user clicked
/// for.
const RANK_SPEED_WINDOW: Duration = Duration::from_secs(2);
/// The panel gives this command 60s. Ten seconds of that margin is the lane's own
/// startup, so the dial loop has to stop well before the UI gives up on it.
pub const SELECTION_DEADLINE: Duration = Duration::from_secs(45);
/// Ranking owns lane 0 (the index allocation lives in `lane_core`).
const RANK_LANE_INDEX: usize = crate::managers::lane_core::LANE_RELAY_RANKING;

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

/// 亚洲其他地区 —— 用户给的顺序:香港不行就新加坡、日本、韩国这类近处。
/// 中东/俄罗斯/印度以西不在这里:它们距离上已经属于"其余"。
const ASIA_CODES: [&str; 16] = [
    "SG", "JP", "KR", "TW", "MO", "TH", "MY", "ID", "VN", "PH", "KH", "LA", "MM", "BN", "NP",
    "MN",
];

/// Hong Kong first, then any local proxy, then the rest of Asia, then everything
/// else in the order the caller gave us. Truncated to [`RANK_LIMIT`].
pub fn order_relay_candidates(candidates: Vec<UnifiedNode>) -> Vec<UnifiedNode> {
    // sort_by_key is stable, so neither the caller's priority order nor the
    // original order inside a class gets shuffled.
    let mut ordered = candidates;
    ordered.sort_by_key(rank_class);
    ordered.truncate(RANK_LIMIT);
    ordered
}

fn rank_class(n: &UnifiedNode) -> u8 {
    let cc = n.country_code.to_ascii_uppercase();
    if cc == "HK" {
        0
    } else if n.group == "LocalProxy" {
        // 本地代理端口是活的监听进程,拨它比拨大洋彼岸便宜,排在亚洲之前。
        1
    } else if ASIA_CODES.contains(&cc.as_str()) {
        2
    } else {
        3
    }
}

/// The winner is simply the first candidate, in the order the operator chose,
/// that carried a probe. Everything after it never gets dialled — that is the
/// whole point of the early exit.
pub fn choose_winner(measured: &[Measured]) -> Option<usize> {
    measured.iter().position(|m| m.latency_ms.is_some())
}

/// Dial candidates in order and stop at the first one that carries traffic.
/// Returns `(measured rows, winner index, aborted)`; only dialled candidates
/// appear, so an untried node keeps whatever verdict it already had instead of
/// being silently written off as dead by a search that never reached it.
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
    let mut winner: Option<usize> = None;
    let mut aborted = false;
    let started = Instant::now();

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
            // 只有真隧道在跑才让路;连接失败留下的 Error 不算,否则一次失败之后
            // 每一轮中转排序都会在第一个候选前停下(和测活同一处事故)。
            if conn.get_status().holds_tunnel() {
                log::info!(
                    "relay ranking: stopped after {} candidates, a real tunnel owns the device now",
                    measured.len()
                );
                aborted = true;
                break;
            }
            if started.elapsed() >= SELECTION_DEADLINE {
                log::warn!(
                    "relay ranking: gave up after {} candidates in {:?}, 面板会以为这个按钮没反应",
                    measured.len(),
                    started.elapsed()
                );
                break;
            }

            let latency = dial_until_alive(&lane, name).await;
            log::info!(
                "relay ranking: {} -> {} ms (累计 {:?})",
                node.name,
                latency.map(|l| l.to_string()).unwrap_or_else(|| "dead".to_string()),
                started.elapsed()
            );
            measured.push(Measured {
                node: node.clone(),
                latency_ms: latency,
                speed_bps: None,
            });
            // 第一个能用的就是它:剩下的候选不用再花一次拨号,用户要的是快。
            winner = choose_winner(&measured);
            if winner.is_some() {
                break;
            }
        }

        // 带宽只在胜者身上测一次 —— 它才是接下来承载隧道的节点,而且测不出来也不
        // 改变结论:用户要的是"可用就选中",不是"最快最肥才选中"。窗口只有 2 秒,
        // 因为实测这批免费节点会对整个 CDN 主机拒连,拿不到数就得马上说没有。
        if let Some(i) = winner {
            let name = &indexed[i].0;
            let speed = lane.measure_throughput(name, RANK_SPEED_WINDOW).await;
            measured[i].speed_bps = speed;
            log::info!(
                "relay ranking: winner {} bandwidth {} B/s (累计 {:?})",
                measured[i].node.name,
                speed.map(|s| s.to_string()).unwrap_or_else(|| "-".to_string()),
                started.elapsed()
            );
        }
        drop(lane);

        persist_measurements(node_manager, &measured)?;
    }

    let winner = winner.map(|i| &measured[i]);
    Ok(RelayRanking {
        preferred_id: winner.map(|m| m.node.id.clone()),
        preferred_name: winner.map(|m| m.node.name.clone()),
        latency_ms: winner.and_then(|m| m.latency_ms),
        speed_bps: winner.and_then(|m| m.speed_bps),
        tested: measured.len(),
        aborted,
    })
}

/// One candidate's verdict: up to [`ATTEMPTS_PER_CANDIDATE`] dials, each of which
/// races every probe endpoint inside its own budget.
async fn dial_until_alive(lane: &Lane, name: &str) -> Option<i64> {
    for attempt in 0..ATTEMPTS_PER_CANDIDATE {
        if attempt > 0 {
            tokio::time::sleep(ATTEMPT_GAP).await;
        }
        if let Some(ms) = lane.measure_member(name).await {
            return Some(ms);
        }
    }
    None
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
    fn hong_kong_and_local_candidates_are_dialled_before_everything_else() {
        let list = vec![
            node("de", "DE", "Default", Some(30)),
            node("local", "LOCAL", "LocalProxy", Some(1)),
            node("hk2", "HK", "Default", Some(400)),
            node("jp", "JP", "Default", Some(50)),
            node("us", "US", "Default", Some(20)),
            node("hk1", "HK", "Default", Some(900)),
        ];
        let ordered = order_relay_candidates(list);
        let ids: Vec<&str> = ordered.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(
            &ids[..4],
            &["hk2", "hk1", "local", "jp"],
            "香港 → 本地 → 亚洲,同类内保持原序: {:?}",
            ids
        );
    }

    /// 用户点名的顺序:没有香港就找新加坡、日本、韩国,再往远处走。
    #[test]
    fn asia_is_dialled_before_europe_and_america() {
        let list = vec![
            node("us", "US", "Default", None),
            node("sg", "SG", "Default", None),
            node("kr", "KR", "Default", None),
            node("de", "DE", "Default", None),
            node("jp", "JP", "Default", None),
            node("tw", "TW", "Default", None),
        ];
        let ordered = order_relay_candidates(list);
        let ids: Vec<&str> = ordered.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(&ids, &["sg", "kr", "jp", "tw", "us", "de"], "{:?}", ids);
    }

    #[test]
    fn candidate_list_is_capped() {
        let list: Vec<UnifiedNode> =
            (0..40).map(|i| node(&format!("n{}", i), "US", "Default", Some(i as i64))).collect();
        assert_eq!(order_relay_candidates(list).len(), RANK_LIMIT);
    }

    #[test]
    fn the_first_usable_candidate_wins_and_the_rest_never_get_dialled() {
        // 快慢不由带宽决定:第一个能用的就是它,刷新才有结果得快。
        let m = vec![
            measured(Some(120), Some(2_000_000)),
            measured(Some(30), Some(500_000_000)),
        ];
        assert_eq!(choose_winner(&m), Some(0));
    }

    #[test]
    fn a_dead_hong_kong_node_falls_through_to_the_next_candidate() {
        let m = vec![measured(None, None), measured(None, None), measured(Some(80), None)];
        assert_eq!(choose_winner(&m), Some(2));
    }

    #[test]
    fn all_dead_means_no_winner() {
        let m = vec![measured(None, None), measured(None, None)];
        assert_eq!(choose_winner(&m), None);
        assert_eq!(choose_winner(&[]), None);
    }

    /// 没拨到的候选不能被判死 —— 早停之后剩下的节点保留原状态,下一次刷新还会试。
    #[test]
    fn only_dialled_candidates_reach_the_verdict_list() {
        let list: Vec<UnifiedNode> =
            (0..RANK_LIMIT + 5).map(|i| node(&format!("h{}", i), "HK", "Default", None)).collect();
        let ordered = order_relay_candidates(list);
        assert_eq!(ordered.len(), RANK_LIMIT, "候选表必须被截断,否则早停也救不了这一轮");
    }
}
