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
use crate::managers::lane_core::{Lane, NodeVerdict, RELAY_MEMBER, LANE_LIVENESS_FIRST};
use crate::managers::node_manager::NodeManager;
use crate::models::{NodeStatus, ProtocolType, UnifiedNode};

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
    /// What this round concluded, or why it could not run — in one line the user
    /// can act on. A sweep that ends with the badge still reading 未测 has to
    /// say why: silently learning nothing is what made the whole button feel
    /// pointless in the field (v0.2.107: "要么可用，要么不可用，要么报错，怎么能够静默").
    pub message: Option<String>,
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
        message: None,
    }
}

/// One core error, shortened for a UI line: the message goes in a label, and the
/// full text is already in the app log.
fn brief(e: &str) -> String {
    let line = e.trim().lines().last().unwrap_or("").trim();
    const MAX: usize = 120;
    if line.chars().count() > MAX {
        format!("{}…", line.chars().take(MAX).collect::<String>())
    } else {
        line.to_string()
    }
}

/// "；其余 N 个未测" when a sweep stopped early, nothing when it did not.
fn unmeasured_tail(total: usize, tested: usize) -> String {
    if tested < total {
        format!("；其余 {} 个未测", total - tested)
    } else {
        String::new()
    }
}

/// The last beat of a list this round will not learn anything more about, said
/// out loud. `total` is what the list holds so the counts stay honest about how
/// much was skipped.
fn stopped_beat(group: &str, total: usize, message: impl Into<String>) -> LivenessProgress {
    LivenessProgress {
        group: group.to_string(),
        tested: 0,
        total,
        alive: 0,
        running: false,
        done: false,
        aborted: true,
        persisted: false,
        message: Some(message.into()),
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

#[derive(Debug)]
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

/// Take the sweep. Returns `Err` when one already holds the lanes; the requested
/// lists then join the running sweep instead of being dropped, which is why the
/// caller must show this message and not just log it — a button that started
/// nothing looks exactly like a button that found nothing.
pub fn claim_pass(groups: &[&'static str]) -> Result<PassGuard, String> {
    if IN_PROGRESS.swap(true, Ordering::SeqCst) {
        for group in groups {
            if let Some(bit) = group_bit(group) {
                QUEUED.fetch_or(bit, Ordering::SeqCst);
            }
        }
        return Err("测活正在进行中，本轮结束后会补测这份名单".to_string());
    }
    Ok(PassGuard)
}

/// Claim for a single-server dial. Nothing gets queued here: the user pointed at
/// one card, and silently scheduling the whole list behind the running sweep
/// would measure servers they never asked about while its button sat there.
fn claim_single() -> Result<PassGuard, String> {
    if IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err("测活正在进行中，请等待当前一轮结束后再单独测这个节点".to_string());
    }
    Ok(PassGuard)
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
    /// Rows this list holds that a lane cannot dial (non-OpenVPN). Reported so
    /// "12 个未测" is never mistaken for "12 个待测".
    skipped: usize,
    /// Nodes whose dial failed **while the relay itself was unreachable** — they
    /// keep whatever status they had, and the summary has to blame the relay.
    not_judged: usize,
    /// Why the sweep stopped, when it did. Becomes the final beat's message.
    note: Option<String>,
}

/// The relay, core binary and writable dir a probe lane needs.
struct LaneSetup {
    binary: PathBuf,
    dir: PathBuf,
    relay_yaml: String,
    relay_id: String,
    relay_name: String,
}

/// Errors here are shown to the user verbatim (a list that cannot be measured at
/// all has to say why), so they are written as advice, not as log lines.
fn prepare_lane(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
) -> Result<LaneSetup, String> {
    let relay = node_manager
        .get_best_relay_node(preferred_id)
        .ok_or_else(|| "没有可用的中转节点：测活要经它拨号，请先连接成功一次，或在面板里手动指定一个中转".to_string())?;
    let relay_yaml = ConnectionManager::format_mihomo_relay_proxy(&relay, RELAY_MEMBER)
        .ok_or_else(|| format!("中转节点「{}」的协议无法用于测活，请换一个中转", relay.name))?;
    Ok(LaneSetup {
        binary: conn.locate_binary(app, "mihomo")?,
        dir: app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app_data_dir: {}", e))?,
        relay_yaml,
        relay_id: relay.id.clone(),
        relay_name: relay.name.clone(),
    })
}

/// Take a relay that just failed its own dial out of the rotation.
///
/// Writing `Dead` onto the record is what stops [`prepare_lane`] from handing the
/// same broken relay to the next pass (`pick_usable_relay` skips dead candidates),
/// and clearing `preferred_relay_id` is what stops the panel from still showing it
/// as "自动优选（已实测）". Without this the app would blame one dead relay for
/// every list forever: the verdict is only useful if it changes what happens next.
fn retire_relay(app: &AppHandle, node_manager: &NodeManager, setup: &LaneSetup) {
    if let Some(mut demoted) = node_manager
        .get_all()
        .into_iter()
        .find(|n| n.id == setup.relay_id)
    {
        demoted.status = NodeStatus::Dead;
        demoted.last_checked = Some(chrono::Utc::now().timestamp());
        if let Err(e) = node_manager.update_node(demoted) {
            log::warn!("liveness: 死中转「{}」未能写回库存: {}", setup.relay_name, e);
        }
    }
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let snapshot = {
        let mut settings = state.settings.write();
        if settings.preferred_relay_id.as_deref() != Some(setup.relay_id.as_str()) {
            None
        } else {
            settings.preferred_relay_id = None;
            Some(settings.clone())
        }
    };
    let Some(snapshot) = snapshot else {
        return;
    };
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write(dir.join("settings.json"), json);
    }
}

/// 一台中转被判死之后要对用户说的那半句:光说"中转不可用"等于把球踢回给用户,
/// 而这一台已经不会再被用到了。
const RELAY_RETIRED_TAIL: &str = "已把这台中转记为不可用，下一轮测活会自动改用其他中转（若列表里还有）";

/// Dial every server of one list, publishing as it goes.
///
/// The lanes are already claimed — see [`claim_pass`], which the command does
/// synchronously so a rejected button reports why instead of logging it in a
/// spawned task nobody watches.
pub async fn run_pass(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    scope: Option<&'static str>,
    _guard: PassGuard,
) {
    let mut queue = groups_for(scope);
    while !queue.is_empty() {
        let groups = std::mem::take(&mut queue);
        let mut yielded = false;
        for group in groups {
            match probe_group(app, node_manager, conn, preferred_id, group).await {
                Ok(stats) => yielded = stats.map(|s| s.aborted).unwrap_or(true),
                Err(e) => {
                    log::warn!("liveness {group}: {e}");
                    // 这条路从前只写日志。一轮没能跑起来而面板上一个字都不提,和
                    // 用户没点过这个按钮没有区别 —— 这正是被三次投诉的"静默"。
                    emit_progress(
                        app,
                        &stopped_beat(group, 0, format!("测活本轮无法进行：{}", brief(&e))),
                    );
                    yielded = true;
                }
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
}

/// Dial one server the user pointed at, and come back with its verdict.
///
/// The answer is a [`ProbeOutcome`], never a bare `()`: this entry point exists
/// because the user wants to know, for this one card, whether they just measured
/// it and what came of it. A dial that failed is only worth reporting once it has
/// been *attributed* — 中转拨不通 and 出口节点拨不通 look identical from the node's
/// side, and blaming the node for a broken relay is the misjudgment that made
/// whole lists read 不可用.
pub async fn measure_one(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    node_id: &str,
    preferred_id: Option<&str>,
) -> Result<ProbeOutcome, String> {
    let node = node_manager
        .get_all()
        .into_iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| "该节点已不在清单中,请点同步重新采集".to_string())?;
    let group = known_group(&node.group).ok_or("该名单不需要真连接测活")?;
    if node.protocol != ProtocolType::Openvpn {
        return Err("只有 OpenVPN 节点支持真连接测活".to_string());
    }
    let _guard = claim_single()?;

    let setup = prepare_lane(app, node_manager, conn, preferred_id)?;
    let name = "p0".to_string();
    let blocks = vec![(
        name.clone(),
        ConnectionManager::mihomo_openvpn_proxy_block(&node, &name, Some(RELAY_MEMBER)),
    )];
    let (b, d, r) = (setup.binary.clone(), setup.dir.clone(), setup.relay_yaml.clone());
    let lane = tokio::task::spawn_blocking(move || {
        Lane::start(LANE_LIVENESS_FIRST, &d, &b, Some(r.as_str()), &blocks)
    })
    .await
    .map_err(|e| format!("测活任务被中断: {}", e))?
    .map_err(|e| format!("测活核心未能启动: {}", e))?;

    // The core dropped this server's config, so nothing was ever dialled. That is
    // not a verdict about the server, and saying so is the whole point.
    if !lane.group_members().await.iter().any(|m| m == &name) {
        drop(lane);
        return Ok(ProbeOutcome::new(
            "not-judged",
            format!(
                "未判定:核心拒绝了「{}」的 OpenVPN 配置(通常是不支持的加密或参数),这个节点没有被拨号",
                node.name
            ),
            None,
        ));
    }

    let verdict = lane.test_node(&name).await;
    let mut relay = RelayWatch::new(setup.relay_name.clone());
    let relay_reachable = if verdict.alive { true } else { relay.reachable(&lane).await };
    let relay_name = setup.relay_name.clone();
    drop(lane);

    if !relay_reachable {
        retire_relay(app, node_manager, &setup);
        return Ok(ProbeOutcome::new(
            "relay-dead",
            format!(
                "中转不可用:中转节点「{}」连拨 {} 次都不通,所以这个出口节点未判定(不是它不可用)—— {}；再点一次「测活」可重试这个节点",
                relay_name, RELAY_ATTEMPTS, RELAY_RETIRED_TAIL
            ),
            None,
        ));
    }

    let measured = store_verdict(node_manager, &node, &verdict)?;
    log::info!(
        "liveness {group}: {}:{} 单节点测活 → {:?}",
        node.address,
        node.port,
        measured.status
    );
    let outcome = if verdict.alive {
        ProbeOutcome::new(
            "alive",
            format!(
                "可用:经中转「{}」真连接成功,{} 毫秒",
                relay_name,
                measured.latency_ms.unwrap_or(0)
            ),
            Some(measured),
        )
    } else {
        ProbeOutcome::new(
            failure_verdict(true),
            format!(
                "出口节点不可用:中转「{}」自检正常,是这个出口节点自己连不上({}:{})",
                relay_name, node.address, node.port
            ),
            Some(measured),
        )
    };
    Ok(outcome)
}

/// 拨不通时唯一的两种解释 —— 分不清就不能给结论。
fn failure_verdict(relay_reachable: bool) -> &'static str {
    if relay_reachable {
        "exit-dead"
    } else {
        "relay-dead"
    }
}

/// Write one dial's verdict to the store and hand back the measured row.
fn store_verdict(
    node_manager: &NodeManager,
    node: &UnifiedNode,
    verdict: &NodeVerdict,
) -> Result<UnifiedNode, String> {
    let measured = record_verdict(node, verdict, chrono::Utc::now().timestamp());
    node_manager
        .update_nodes(std::slice::from_ref(&measured))
        .map_err(|e| format!("测活结论未能保存: {}", e))?;
    Ok(measured)
}

/// 一次单节点测活的结论。`message` 一定是一句人话且永不为空:这个入口存在的意义
/// 就是让用户知道"我刚才到底测没测、测出了什么"。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeOutcome {
    /// `"alive"` | `"exit-dead"` | `"relay-dead"` | `"not-judged"`
    pub verdict: &'static str,
    pub message: String,
    /// 只有真判定过才有值;没判定时卡片保持"未测",不留下假结论。
    pub node: Option<UnifiedNode>,
}

impl ProbeOutcome {
    /// 造一条结论。`message` 是硬约定:空话就等于"静默失败",而这个入口存在的
    /// 理由就是不让它发生。
    fn new(verdict: &'static str, message: impl Into<String>, node: Option<UnifiedNode>) -> Self {
        let message = message.into();
        debug_assert!(!message.trim().is_empty(), "测活结论必须带一句话");
        debug_assert!(
            (verdict == "relay-dead" || verdict == "not-judged") == node.is_none(),
            "没判定就不该留下假的节点状态"
        );
        Self { verdict, message, node }
    }
}

/// 中转自身是否拨得通。一次判定要连续拨 [`RELAY_ATTEMPTS`] 次才算数 —— 只拨一次
/// 的话,一条刚失败过的 openvpn 拨号留下的抖动会把中转误判成死了,而"几秒前才
/// 用它测出可用"和"中转不可用"同时出现在屏幕上(v0.2.110 现场就是这个矛盾)。
struct RelayWatch {
    name: String,
    last: Option<(bool, std::time::Instant)>,
}

/// 判定"中转不可用"所需的连续失败次数。每一次都是真实握手 + 一个 204 往返。
const RELAY_ATTEMPTS: usize = 3;
/// 两次自检之间给核心一点时间:上一条失败拨号的连接还在拆,立刻重拨最容易又超时。
const RELAY_RETRY_GAP: std::time::Duration = std::time::Duration::from_millis(600);

/// 复用"通"这个结论的时间窗。"不通"不复用:下一张卡片必须重新问一遍。
const RELAY_RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

impl RelayWatch {
    fn new(name: String) -> Self {
        Self { name, last: None }
    }

    fn name(&self) -> &str {
        &self.name
    }

    /// 换一条 lane 就把上一次的答复忘掉:中转是在跑的过程中挂掉的,新 lane 上的
    /// 第一次失败必须重新问一遍。
    fn forget(&mut self) {
        self.last = None;
    }

    /// 节点拨失败时问一次中转:连拨 `RELAY_ATTEMPTS` 次都不通才说它不可用。
    async fn reachable(&mut self, lane: &Lane) -> bool {
        if let Some((true, at)) = self.last {
            if at.elapsed() < RELAY_RECHECK {
                return true;
            }
        }
        for attempt in 0..RELAY_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(RELAY_RETRY_GAP).await;
            }
            if lane.check_relay().await.is_some() {
                log::info!(
                    "liveness: 中转「{}」自检 → 可用(第 {} 次拨通)",
                    self.name,
                    attempt + 1
                );
                self.last = Some((true, std::time::Instant::now()));
                return true;
            }
        }
        log::warn!(
            "liveness: 中转「{}」连拨 {} 次都不通,失败该记在它头上",
            self.name,
            RELAY_ATTEMPTS
        );
        self.last = Some((false, std::time::Instant::now()));
        false
    }
}


/// Dial one list through the preferred relay, saying out loud what came of it.
///
/// Every exit from this function publishes a beat the panel can show: a list with
/// nothing to dial, a relay that cannot be used, a lane that would not start, and
/// the finished sweep all end with a sentence, because 未测 with no explanation is
/// indistinguishable from a button that does nothing.
///
/// `Ok(None)` means the dial path itself is dead (no relay, no lane) — the caller
/// stops rather than retry that per list.
async fn probe_group(
    app: &AppHandle,
    node_manager: &NodeManager,
    conn: &ConnectionManager,
    preferred_id: Option<&str>,
    group: &'static str,
) -> Result<Option<GroupStats>, String> {
    let all = node_manager.get_all();
    let rows = liveness_candidates(&all, group);
    // Rows of this list a lane cannot represent. They keep whatever verdict they
    // had; the summary says how many, so the count in the panel is explainable.
    let skipped = all.iter().filter(|n| n.group == group && n.protocol != ProtocolType::Openvpn).count();
    if rows.is_empty() {
        let message = if skipped > 0 {
            format!(
                "这份名单里 {} 个节点都不是 OpenVPN，真连接测活无法拨号",
                skipped
            )
        } else {
            "名单是空的：请先点「同步」采集节点，再测活".to_string()
        };
        log::info!("liveness {group}: {message}");
        emit_progress(app, &stopped_beat(group, skipped, message));
        return Ok(Some(GroupStats {
            total: 0,
            skipped,
            ..Default::default()
        }));
    }
    let setup = match prepare_lane(app, node_manager, conn, preferred_id) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("liveness {group}: {e}");
            emit_progress(app, &stopped_beat(group, rows.len(), e));
            return Ok(None);
        }
    };

    let mut stats = GroupStats {
        total: rows.len(),
        skipped,
        ..Default::default()
    };
    let mut relay = RelayWatch::new(setup.relay_name.clone());
    emit_progress(app, &beat(group, &stats, true, false, false));
    log::info!(
        "liveness {group}: dialling {} servers through {} in batches of {}",
        rows.len(),
        setup.relay_name,
        BATCH_SIZE
    );

    let now = chrono::Utc::now().timestamp();
    let mut rejected = 0usize;
    for (batch_no, chunk) in rows.chunks(BATCH_SIZE).enumerate() {
        if tunnel_took_over(conn) {
            stats.aborted = true;
            stats.note = Some(format!(
                "已让路给你正在使用的连接：测了 {}/{} 个，其余未测",
                stats.tested, stats.total
            ));
            break;
        }

        let mut blocks: Vec<(String, String)> = Vec::with_capacity(chunk.len());
        let mut targets: Vec<(String, &UnifiedNode)> = Vec::with_capacity(chunk.len());
        for (i, node) in chunk.iter().enumerate() {
            let name = format!("p{}", i);
            blocks.push((
                name.clone(),
                ConnectionManager::mihomo_openvpn_proxy_block(node, &name, Some(RELAY_MEMBER)),
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
                // nothing about them — but it has to say that.
                log::warn!("liveness {group}: lane {} did not start: {}", index, e);
                stats.aborted = true;
                stats.note = Some(format!(
                    "测活核心未能启动（{}），本轮中止：已测 {}/{} 个，其余未测",
                    brief(&e),
                    stats.tested,
                    stats.total
                ));
                break;
            }
            Err(e) => {
                log::warn!("liveness {group}: lane {} join failed: {}", index, e);
                stats.aborted = true;
                stats.note = Some(format!(
                    "测活任务被系统中断（{}），本轮中止：已测 {}/{} 个，其余未测",
                    brief(&e.to_string()),
                    stats.tested,
                    stats.total
                ));
                break;
            }
        };

        let accepted = lane.group_members().await;
        relay.forget();
        if accepted.len() < targets.len() {
            log::warn!(
                "liveness {group}: lane {} took {} of {} configs, the rest are left 未测",
                index,
                accepted.len(),
                targets.len()
            );
        }

        let batch_start = std::time::Instant::now();
        let alive_before_batch = stats.alive;
        let mut pending: Vec<UnifiedNode> = Vec::with_capacity(targets.len());
        for (name, node) in &targets {
            if tunnel_took_over(conn) {
                stats.aborted = true;
                stats.note = Some(format!(
                    "已让路给你正在使用的连接：测了 {}/{} 个，其余未测",
                    stats.tested, stats.total
                ));
                break;
            }
            // The core refused this server's OpenVPN config, so nothing was ever
            // dialled: that is not evidence about the server.
            if !accepted.iter().any(|m| m == name) {
                rejected += 1;
                continue;
            }
            let verdict = lane.test_node(name).await;
            if !verdict.alive && !relay.reachable(&lane).await {
                // 失败的第一种解释是承载它的那台中转自己拨不通 —— 那时候把这些出口
                // 节点标成"不可用"是假账:它们根本没被真正测到。停手,并说清是谁的问题。
                stats.not_judged += 1;
                stats.aborted = true;
                retire_relay(app, node_manager, &setup);
                stats.note = Some(format!(
                    "中转节点「{}」连拨 {} 次都不通，本轮中止：{} 个节点未判定（不是它们不可用），已测 {}/{} 个；{}",
                    relay.name(),
                    RELAY_ATTEMPTS,
                    stats.not_judged,
                    stats.tested,
                    stats.total,
                    RELAY_RETIRED_TAIL
                ));
                break;
            }
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

    let mut final_beat = beat(group, &stats, false, stats.aborted, true);
    final_beat.message = Some(match stats.note.take() {
        Some(reason) => reason,
        None => {
            let mut summary = format!(
                "测活完成（经中转「{}」）：{}/{} 个可连通{}",
                setup.relay_name,
                stats.alive,
                stats.tested,
                unmeasured_tail(stats.total, stats.tested)
            );
            if stats.tested > 0 && stats.alive == 0 {
                // 走到这里说明每次失败后都验过中转(见 RelayWatch),所以这句
                // "是出口节点连不上"是有依据的,不是猜的。
                summary.push_str("；中转自检正常，是这些出口节点自己连不上");
            }
            if rejected > 0 {
                summary.push_str(&format!(
                    "；{} 个节点的 OpenVPN 配置被核心拒绝，未判定",
                    rejected
                ));
            }
            if stats.skipped > 0 {
                summary.push_str(&format!(
                    "；{} 个非 OpenVPN 节点不参与真连接测活",
                    stats.skipped
                ));
            }
            summary
        }
    });
    emit_progress(app, &final_beat);
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
/// `Error` does not count — see [`crate::models::ConnectionStatus::holds_tunnel`],
/// and the field report that this predicate once silently emptied.
fn tunnel_took_over(conn: &ConnectionManager) -> bool {
    conn.get_status().holds_tunnel()
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
        let held = claim_pass(&["VPNGate"]).expect("the first caller owns the sweep");
        assert!(take_queued().is_empty(), "nothing queued yet");

        // The other panel has its own button; pressing it mid-sweep must not
        // restart this one, must not be dropped — and must say so out loud,
        // because a press that quietly did nothing is exactly "点了没反应".
        let busy = claim_pass(&["Residential"]);
        assert!(busy.is_err(), "a second sweep cannot share the lanes");
        assert!(busy.unwrap_err().contains("正在进行中"), "拒绝必须带上原因");
        assert_eq!(take_queued(), vec!["Residential"]);
        assert!(take_queued().is_empty(), "draining clears the request");

        assert!(claim_pass(&["VPNGate", "Residential"]).is_err());
        assert_eq!(take_queued(), LIVENESS_GROUPS.to_vec(), "一次排队可以攒下两份名单");

        // A single card asks about one server, so it must not book the whole list
        // behind the running sweep as a side effect.
        drop(held);
        let held_again = claim_single().expect("releasing the sweep lets the next one in");
        assert!(claim_single().is_err());
        assert_eq!(take_queued(), Vec::<&str>::new(), "单节点请求不排队");
        drop(held_again);
    }

    #[test]
    fn a_dead_relay_is_not_the_exit_nodes_fault() {
        assert_eq!(failure_verdict(true), "exit-dead");
        assert_eq!(
            failure_verdict(false),
            "relay-dead",
            "中转自己拨不通时,失败不能记到出口节点头上"
        );
    }

    #[test]
    fn every_single_node_verdict_leaves_a_sentence() {
        let judged = node("v1", "VPNGate", ProtocolType::Openvpn, NodeStatus::Alive);
        for (verdict, with_node) in [
            ("alive", true),
            ("exit-dead", true),
            ("relay-dead", false),
            ("not-judged", false),
        ] {
            let o = ProbeOutcome::new(
                verdict,
                format!("{verdict} 的结论"),
                if with_node { Some(judged.clone()) } else { None },
            );
            assert!(!o.message.trim().is_empty(), "{verdict} 不能没有话 — 那就是静默");
            assert_eq!(o.node.is_some(), with_node, "{verdict} 不该留下假的节点状态");
        }
    }

    #[test]
    fn a_sweep_that_cannot_start_says_why() {
        let p = stopped_beat("VPNGate", 137, "没有可用的中转节点：测活要经它拨号");
        assert!(!p.running && p.aborted && !p.done, "这是一条终止播报");
        assert_eq!(
            p.message.as_deref(),
            Some("没有可用的中转节点：测活要经它拨号")
        );
        assert_eq!(p.total, 137, "要说清楚有多少个因此没测");
        assert_eq!(p.tested, 0, "一个都没拨，不能装作测过");

        assert_eq!(unmeasured_tail(100, 30), "；其余 70 个未测");
        assert_eq!(unmeasured_tail(100, 100), "", "全部测完就不该提未测");
    }

    #[test]
    fn the_terminal_beat_says_done_only_when_the_list_is_finished() {
        let half = GroupStats {
            tested: 50,
            total: 100,
            alive: 9,
            ..Default::default()
        };
        let running = beat("VPNGate", &half, true, false, true);
        assert!(running.running && !running.done, "还在拨下一批");
        assert_eq!(running.message, None, "中途每一拍都只报进度");

        let done = GroupStats {
            tested: 100,
            total: 100,
            alive: 19,
            ..Default::default()
        };
        let stopped = beat("VPNGate", &done, false, false, true);
        assert!(stopped.done && !stopped.aborted);

        let early = GroupStats {
            tested: 30,
            total: 100,
            alive: 4,
            aborted: true,
            ..Default::default()
        };
        let yielded = beat("Residential", &early, false, true, true);
        assert!(!yielded.done, "提前结束不等于全部测完");
        assert!(yielded.aborted);
    }
}
