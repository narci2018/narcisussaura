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
    /// 到目前为止,这一轮里有哪台中转被**实测**判死并记为不可用(只取最后一台)。
    ///
    /// 中转栏拿它把"已实测并自动选用「X」"立刻撤掉。从前这个事件只带一句话,栏上
    /// 那半句绿的还留着,于是同一屏出现两个相反的结论 —— 用户据此判定 app 在撒谎
    /// (v0.2.114 现场:红字"连拨 3 次都不通"和"已实测并自动选用"同屏)。
    /// 本地故障不会填这个字段:那种情形没有任何中转被定罪。
    pub relay_retired: Option<String>,
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
        relay_retired: stats.relay_retired.clone(),
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
        relay_retired: None,
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

/// 真连接结论的唯一落笔处:节点行和账本签名同批写。分开两处写早晚会出现"行上有
/// 结论、账本上没签名"(或反过来)的分家 —— 那正是这张卡一句"可用"一句"未测"的老路。
fn commit_verdicts(node_manager: &NodeManager, measured: &[UnifiedNode]) -> Result<(), String> {
    node_manager.update_nodes(measured)?;
    node_manager.stamp_liveness(measured)
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
    /// 这一轮里被实测判死、已经记为不可用的那台中转(最后一台)。见
    /// [`LivenessProgress::relay_retired`]。
    relay_retired: Option<String>,
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

/// 一台中转被判死、而且**本轮已经没有第二台可换**时的那半句。
///
/// 光说"中转不可用"等于把球踢回给用户。这句必须如实交代"换不了"这件事 —— 因为
/// 同一轮里换成功的时候是会说"已改用某某继续"的,两种话术的差别就是用户判断 app
/// 有没有在做事的依据。
const RELAY_RETIRED_TAIL: &str = "本轮已经没有第二台可换的中转；它已被记为不可用，下次测活会重新实测它，也可以先在中转栏里换一台";

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

    let mut setup = prepare_lane(app, node_manager, conn, preferred_id)?;
    // 单节点也换机,但只换一次:用户问的是一个节点,不是整份名单。第二次仍拨不通
    // 就停下来,因为那时"再点一次"也不会给出别的答案,必须如实说清楚。
    let mut switches = 0usize;
    let mut convicted: Vec<String> = Vec::new();
    loop {
        let dead_id = setup.relay_id.clone();
        let single = measure_once(node_manager, group, &node, &setup).await;
        match single {
            Single::Done(outcome) => return Ok(outcome),
            Single::RelayDown(reason) => {
                convicted.push(setup.relay_name.clone());
                retire_relay(app, node_manager, &setup);
                let next = (switches < 1)
                    .then(|| prepare_lane(app, node_manager, conn, None).ok())
                    .flatten()
                    .filter(|n| n.relay_id != dead_id);
                if let Some(next) = next {
                    switches += 1;
                    log::warn!(
                        "liveness {group}: 中转「{}」拨不通({}),单节点测活改用「{}」重试",
                        convicted.last().unwrap(),
                        reason,
                        next.relay_name
                    );
                    setup = next;
                    continue;
                }
                let who = if convicted.len() == 1 {
                    format!("中转节点「{}」", convicted[0])
                } else {
                    format!("中转节点「{}」", convicted.join("」「"))
                };
                return Ok(ProbeOutcome::new(
                    "relay-dead",
                    format!(
                        "{}连拨 {} 次都不通(最后一次:{}),所以这个出口节点未判定 —— 不是它不可用。已把这些中转记为不可用；再点一次「测活」会重测这个节点,列表里还有别的可用中转时会自动改用",
                        who, RELAY_ATTEMPTS, reason
                    ),
                    None,
                ));
            }
            Single::LaneSick(reason) => {
                return Ok(ProbeOutcome::new(
                    "not-judged",
                    format!(
                        "未判定:{} —— 这是本机测活核心的问题,没有把中转记为不可用,也没有给这个节点任何结论；再点一次「测活」可以重试",
                        reason
                    ),
                    None,
                ));
            }
        }
    }
}

/// 一次单节点拨号的结果:要么已经有最终结论,要么这次失败根本不该算在节点头上。
enum Single {
    Done(ProbeOutcome),
    /// 中转被实测判死,带着那句理由。
    RelayDown(String),
    /// 核心没能把流量交给中转 —— 本地故障,不定任何人的罪。
    LaneSick(String),
}

async fn measure_once(
    node_manager: &NodeManager,
    group: &'static str,
    node: &UnifiedNode,
    setup: &LaneSetup,
) -> Single {
    let name = "p0".to_string();
    let blocks = vec![(
        name.clone(),
        ConnectionManager::mihomo_openvpn_proxy_block(node, &name, Some(RELAY_MEMBER)),
    )];
    let (b, d, r) = (setup.binary.clone(), setup.dir.clone(), setup.relay_yaml.clone());
    let lane = match tokio::task::spawn_blocking(move || {
        Lane::start(LANE_LIVENESS_FIRST, &d, &b, Some(r.as_str()), &blocks)
    })
    .await
    {
        Ok(Ok(lane)) => lane,
        // 核心没能启动 / 任务被中断:同样是"没有被拨号",不是"拨不通"。这些话留在
        // 结论里,而不是变成一条转瞬即逝的 reject —— 用户投诉过那个形态。
        Ok(Err(e)) => {
            return Single::Done(ProbeOutcome::new(
                "not-judged",
                format!("未判定:测活核心未能启动({}),这个节点没有被拨号", brief(&e)),
                None,
            ))
        }
        Err(e) => {
            return Single::Done(ProbeOutcome::new(
                "not-judged",
                format!(
                    "未判定:测活任务被系统中断({}),这个节点没有被拨号",
                    brief(&e.to_string())
                ),
                None,
            ))
        }
    };

    // The core dropped this server's config, so nothing was ever dialled. That is
    // not a verdict about the server, and saying so is the whole point.
    if !lane.group_members().await.iter().any(|m| m == &name) {
        drop(lane);
        return Single::Done(ProbeOutcome::new(
            "not-judged",
            format!(
                "未判定:核心拒绝了「{}」的 OpenVPN 配置(通常是不支持的加密或参数),这个节点没有被拨号",
                node.name
            ),
            None,
        ));
    }

    let verdict = lane.test_node(&name).await;
    if verdict.alive {
        // 这台中转刚刚真的把一条拨号送出去了:给后面每一次"中转死了"的指控留一份
        // 硬证据(见 [`RELAY_CARRIED_PROOF`])。
        note_carried(&setup.relay_name);
    }
    let mut watch = RelayWatch::new(setup.relay_name.clone());
    let relay_verdict = if verdict.alive {
        RelayVerdict::Reachable
    } else {
        watch.judge(&lane).await
    };
    let relay_name = setup.relay_name.clone();
    drop(lane);

    // 单节点也要两条核心都说不通才定中转的罪:否则一次误判就把用户正在用的中转
    // 记成不可用,而屏幕上几秒前还写着"已实测并自动选用"。
    let relay_verdict = match relay_verdict {
        RelayVerdict::Unreachable(reason) => match second_opinion(
            &relay_name,
            reason,
            confirm_relay_dead(LANE_LIVENESS_FIRST, &setup).await,
        ) {
            RelayFate::Convicted(r) => RelayVerdict::Unreachable(r),
            RelayFate::NotConvicted(r) => RelayVerdict::LocalFault(r),
        },
        other => other,
    };

    match relay_verdict {
        RelayVerdict::Unreachable(reason) => return Single::RelayDown(reason),
        RelayVerdict::LocalFault(reason) => return Single::LaneSick(reason),
        RelayVerdict::Reachable => {}
    }

    let measured = match store_verdict(node_manager, node, &verdict) {
        Ok(m) => m,
        Err(e) => {
            return Single::Done(ProbeOutcome::new(
                "not-judged",
                format!("未判定:已经拨过号,但结论没能存下来({}),卡片仍是未测", brief(&e)),
                None,
            ))
        }
    };
    log::info!(
        "liveness {group}: {}:{} 单节点测活 → {:?}",
        node.address,
        node.port,
        measured.status
    );
    Single::Done(if verdict.alive {
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
    })
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
    commit_verdicts(node_manager, std::slice::from_ref(&measured))
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

/// 节点拨失败时,对"这是谁的锅"的三种回答。
///
/// 从前这里只有"通 / 不通"两格,而"不通"混装了两种完全相反的事实:中转真的拨不
/// 通,以及本机核心没能把流量交给中转(API 拒绝、客户端建不起来)。后者是本地故障,
/// 拿它去写死一台中转,就是屏幕上"几秒前才实测可用"和"连拨 3 次都不通"同时出现的
/// 原因(v0.2.114 现场)。
#[derive(Debug, Clone, PartialEq, Eq)]
enum RelayVerdict {
    /// 中转经得住拨号:那次失败该记在出口节点头上。
    Reachable,
    /// 连拨 [`RELAY_ATTEMPTS`] 次都没有任何探测站点经它应答 —— 判死的唯一证据。
    Unreachable(String),
    /// 本地故障:核心没有把流量交给中转。既不定中转的罪,也不定出口节点的罪。
    LocalFault(String),
}

/// 中转自身是否拨得通。一次判定要连续拨 [`RELAY_ATTEMPTS`] 次才算数 —— 只拨一次
/// 的话,一条刚失败过的 openvpn 拨号留下的抖动会把中转误判成死了,而"几秒前才
/// 用它测出可用"和"中转不可用"同时出现在屏幕上(v0.2.110 现场就是这个矛盾)。
struct RelayWatch {
    name: String,
    /// 最近一次"通"的时刻。"不通"不缓存:下一张卡片必须重新问一遍。
    alive_at: Option<std::time::Instant>,
}

/// 判定"中转不可用"所需的连续失败次数。每一次都是真实握手 + 一个 204 往返。
const RELAY_ATTEMPTS: usize = 3;
/// 两次自检之间给核心一点时间:上一条失败拨号的连接还在拆,立刻重拨最容易又超时。
const RELAY_RETRY_GAP: std::time::Duration = std::time::Duration::from_millis(600);

/// 复用"通"这个结论的时间窗。"不通"不复用:下一张卡片必须重新问一遍。
const RELAY_RECHECK: std::time::Duration = std::time::Duration::from_secs(60);

/// 「不在场证明」的有效期:这台中转多久之前还真的把某个节点的流量送出去过。
///
/// 比 `RELAY_RECHECK` 短得多是故意的。自检是在**问**中转"你还在吗",而一次成功的
/// 节点拨号是它**已经答过**的事 —— 请求经它出去、204 经它回来。v0.2.113/.114 的
/// 现场矛盾都是同一件事:同一轮里十来个节点经这台中转拨通,下一个失败之后自检却
/// 说"连拨 3 次都不通"。硬证据不该被一次自检的抖动推翻。
///
/// 代价要说清楚:如果中转真的在跑的过程中死了,最多有 `RELAY_CARRIED_PROOF` 这一
/// 窗(约两三个节点)会被记成"出口节点不可用";窗口一过,三次自检 + 换核心复核还
/// 是会定它的罪。冤枉一台中转要推翻整轮的结论,反过来只影响几个节点。
const RELAY_CARRIED_PROOF: std::time::Duration = std::time::Duration::from_secs(15);

/// 最近一次"某台中转真的送出去过流量":(中转名, 时刻)。
static LAST_CARRIED: std::sync::Mutex<Option<(String, std::time::Instant)>> =
    std::sync::Mutex::new(None);

fn note_carried_at(relay_name: &str, at: std::time::Instant) {
    if let Ok(mut guard) = LAST_CARRIED.lock() {
        *guard = Some((relay_name.to_string(), at));
    }
}

/// 记一笔"这台中转刚刚把一条拨号送出去了"。
fn note_carried(relay_name: &str) {
    note_carried_at(relay_name, std::time::Instant::now());
}

/// 这台中转在 `within` 之内有没有真的送出去过流量(返回距那次成功的时长)。
/// 名字必须对得上:换过中转之后,上一台的功劳不能替它顶罪。
fn carried_within(
    relay_name: &str,
    within: std::time::Duration,
    now: std::time::Instant,
) -> Option<std::time::Duration> {
    let guard = LAST_CARRIED.lock().ok()?;
    let (name, at) = guard.as_ref()?;
    if name != relay_name {
        return None;
    }
    let ago = now.checked_duration_since(*at)?;
    (ago < within).then_some(ago)
}

impl RelayWatch {
    fn new(name: String) -> Self {
        Self { name, alive_at: None }
    }

    fn name(&self) -> &str {
        &self.name
    }

    /// 换一条 lane 就把上一次的答复忘掉:中转是在跑的过程中挂掉的,新 lane 上的
    /// 第一次失败必须重新问一遍。
    fn forget(&mut self) {
        self.alive_at = None;
    }

    /// 节点拨失败时问一次中转:连拨 `RELAY_ATTEMPTS` 次都不通才说它不可用。
    async fn judge(&mut self, lane: &Lane) -> RelayVerdict {
        if let Some(at) = self.alive_at {
            if at.elapsed() < RELAY_RECHECK {
                return RelayVerdict::Reachable;
            }
        }
        // 自检之前先问一条更硬的证据:这台中转多久之前还真的把某个节点的流量送出
        // 去(见 RELAY_CARRIED_PROOF)。有这份证明在,就不必拿一次抖动去定它的罪。
        if let Some(ago) = carried_within(&self.name, RELAY_CARRIED_PROOF, std::time::Instant::now())
        {
            log::info!(
                "liveness: 中转「{}」{} 秒前还承载过一次成功拨号 —— 不用再问它自己",
                self.name,
                ago.as_secs()
            );
            return RelayVerdict::Reachable;
        }
        let mut last_reason: Option<String> = None;
        for attempt in 0..RELAY_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(RELAY_RETRY_GAP).await;
            }
            match lane.check_relay().await {
                Ok(_) => {
                    log::info!(
                        "liveness: 中转「{}」自检 → 可用(第 {} 次拨通)",
                        self.name,
                        attempt + 1
                    );
                    self.alive_at = Some(std::time::Instant::now());
                    return RelayVerdict::Reachable;
                }
                Err(failure) if failure.is_local_fault() => {
                    // 本地故障不必拨满三次:重拨问的是同一个生病的核心,能救它的是
                    // 换一条 lane,而不是再多花 24 秒。
                    log::warn!(
                        "liveness: 中转「{}」自检遇到本地故障(第 {} 次)—— 这不记在中转头上: {}",
                        self.name,
                        attempt + 1,
                        failure.brief()
                    );
                    return RelayVerdict::LocalFault(failure.brief());
                }
                Err(failure) => {
                    log::warn!(
                        "liveness: 中转「{}」第 {}/{} 次自检拨不通: {}",
                        self.name,
                        attempt + 1,
                        RELAY_ATTEMPTS,
                        failure.brief()
                    );
                    last_reason = Some(failure.brief());
                }
            }
        }
        log::warn!(
            "liveness: 中转「{}」连拨 {} 次都不通,失败该记在它头上",
            self.name,
            RELAY_ATTEMPTS
        );
        RelayVerdict::Unreachable(last_reason.unwrap_or_else(|| {
            format!("连拨 {} 次都没有一个探测站点经它应答", RELAY_ATTEMPTS)
        }))
    }
}

/// 两条核心一起给出的最终意见:要么定罪,要么谁都不定罪。
///
/// 只有这一种两格划分,才不会让"一条核心说不通"被顺手当成"中转死了" —— 之前就是
/// 这种混装让屏幕上同时出现"已实测并自动选用"和"连拨 3 次都不通"。
enum RelayFate {
    /// 两条不同的核心都连拨 3 次拨不通:记为不可用,换一台继续。
    Convicted(String),
    /// 没能凑够证据:不写 Dead,按本地故障处理。
    NotConvicted(String),
}

/// 换一条**全新的核心**再问一遍这台中转。
///
/// 同一条 lane 上连拨三次都不通,证明的只是"这条 lane 出不去"。核心是在测活过程
/// 里一台一台往下拨的,它中途垮掉之后控制端口就不再应答,而那份账会被记在中转头
/// 上 —— v0.2.114 的现场就是这样:一台刚实测过 403ms/37Mbps 的 HK 中转,在顺利测
/// 完 13 个节点之后被判"连拨 3 次都不通"。一台中转的死罪要两个不同的核心共同签字。
/// 调用之前必须已经把原来那条 lane drop 掉,否则会连上旧进程假装就绪。
async fn confirm_relay_dead(index: usize, setup: &LaneSetup) -> RelayVerdict {
    let (b, d, r) = (
        setup.binary.clone(),
        setup.dir.clone(),
        setup.relay_yaml.clone(),
    );
    let lane = match tokio::task::spawn_blocking(move || {
        Lane::start(index, &d, &b, Some(r.as_str()), &[])
    })
    .await
    {
        Ok(Ok(lane)) => lane,
        Ok(Err(e)) => {
            return RelayVerdict::LocalFault(format!(
                "复核用的新核心没能启动（{}），这台中转的\"不通\"没有被确认",
                brief(&e)
            ))
        }
        Err(e) => {
            return RelayVerdict::LocalFault(format!(
                "复核用的新核心被系统中断（{}），这台中转的\"不通\"没有被确认",
                brief(&e.to_string())
            ))
        }
    };
    let verdict = RelayWatch::new(setup.relay_name.clone()).judge(&lane).await;
    drop(lane);
    verdict
}

/// 第二条核心的意见合进第一条的结论里。
///
/// 新核心**通了**是最要紧的一种:它说明死的是刚才那条 lane,中转一个节点都没失去,
/// 所以这里把它折算成本地故障 —— 后面那条路径不会写 Dead,也不会中止整轮。
fn second_opinion(relay_name: &str, first_reason: String, verdict: RelayVerdict) -> RelayFate {
    match verdict {
        RelayVerdict::Unreachable(second) => RelayFate::Convicted(format!(
            "{}；换一条全新的测活核心复核后依然不通（{}）",
            first_reason, second
        )),
        RelayVerdict::LocalFault(reason) => RelayFate::NotConvicted(reason),
        RelayVerdict::Reachable => RelayFate::NotConvicted(format!(
            "刚才那条测活核心出不去（{}），但一条全新的核心经中转「{}」是能通的 —— 出问题的是本机核心，不是中转",
            first_reason, relay_name
        )),
    }
}

/// 换中转的预算。一轮里最多换这么几台;超过之后必须停下来,因为连着数台中转都拨
/// 不通说明问题在这台机器的出口网络上,继续换只会让用户看着进度条空转。
const RELAY_ROTATIONS_MAX: usize = 2;
/// 本地故障后重建测活核心的预算。
const LANE_REBUILDS_MAX: usize = 2;

/// 一条 lane 为什么提前结束 —— 三种原因的处置完全不同,所以必须分开说。
enum LaneStop {
    /// 用户开了真隧道,让路。
    Yielded,
    /// 中转被实测判定拨不通(有证据):定罪它,然后换一台继续。
    RelayDown(String),
    /// 本机核心没能把流量交给中转(无证据):不定罪任何人,重建一条 lane 继续。
    LaneSick(String),
}

/// 一轮还在跑、但刚刚发生了一件必须让用户看见的事(换中转、重启核心)时的一拍。
fn message_beat(group: &str, stats: &GroupStats, message: &str) -> LivenessProgress {
    let mut p = beat(group, stats, true, false, true);
    p.message = Some(message.to_string());
    p
}

/// 换中转那一拍的话。
///
/// 它必须说"正在改用哪一台",而不是"本轮中止" —— 从前这里只留下一句定罪,而屏幕上
/// 那台中转几秒前才刚被实测选用,用户只能得出一个结论:app 在撒谎。
fn rotation_message(dead: &str, reason: &str, next: &str) -> String {
    format!(
        "中转节点「{dead}」连拨 {attempts} 次都不通（{reason}），已记为不可用，现在改用「{next}」继续测：刚才那个节点会用新中转重测",
        attempts = RELAY_ATTEMPTS
    )
}

/// 本地故障那一拍的话。**不许出现"记为不可用"** —— 什么都没有发生,谁也不该定罪。
fn local_fault_message(reason: &str) -> String {
    format!(
        "测活核心自身出了问题（{reason}），正在重启测活核心后继续：刚才那个节点没有经过中转，不算它的结论"
    )
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
    let mut setup = match prepare_lane(app, node_manager, conn, preferred_id) {
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
    // 换机与重建的预算。上限不是为了省时间,而是不让"再换一台试试"变成死循环:
    // 连着几台中转都拨不通,那不是中转运气差,是这台机器出不去网络 —— 那种事必须
    // 停下来如实说,而不是把剩下的节点一路"未判定"下去。
    let mut rotations = 0usize;
    let mut rebuilds = 0usize;
    // cursor 精确停在"下一个还没有判定的节点":换中转或重建 lane 之后从它重拨,
    // 一个节点都不会被跳过,也不会被算两次。
    let mut cursor = 0usize;
    let mut round = 0usize;
    while cursor < rows.len() {
        if tunnel_took_over(conn) {
            stats.aborted = true;
            stats.note = Some(format!(
                "已让路给你正在使用的连接：测了 {}/{} 个，其余未测",
                stats.tested, stats.total
            ));
            break;
        }
        let start = cursor;
        let end = (start + BATCH_SIZE).min(rows.len());
        let chunk = &rows[start..end];

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

        let index = lane_index_for(round);
        round += 1;
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
        // 这一条 lane 为什么结束;None 表示这一段节点全都有结论了。
        let mut stop: Option<LaneStop> = None;
        for (i, (name, node)) in targets.iter().enumerate() {
            cursor = start + i;
            if tunnel_took_over(conn) {
                stop = Some(LaneStop::Yielded);
                break;
            }
            // The core refused this server's OpenVPN config, so nothing was ever
            // dialled: that is not evidence about the server.
            if !accepted.iter().any(|m| m == name) {
                rejected += 1;
                cursor += 1;
                continue;
            }
            let verdict = lane.test_node(name).await;
            if verdict.alive {
                // 每一张拨通的卡片都在给这台中转作不在场证明(见 RELAY_CARRIED_PROOF)。
                note_carried(relay.name());
            }
            if !verdict.alive {
                match relay.judge(&lane).await {
                    RelayVerdict::Reachable => {
                        // 中转自检正常,这一票就记在出口节点自己身上。
                    }
                    RelayVerdict::Unreachable(reason) => {
                        stop = Some(LaneStop::RelayDown(reason));
                        break;
                    }
                    RelayVerdict::LocalFault(reason) => {
                        stop = Some(LaneStop::LaneSick(reason));
                        break;
                    }
                }
            }
            cursor += 1;
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
        // 本地故障那一拍同样要留证据:核心中途死了(select 拨不动)只有它的日志知道。
        let want_tail =
            stats.alive == alive_before_batch || matches!(&stop, Some(LaneStop::LaneSick(_)));
        let core_tail = if want_tail { lane.tail_log(20) } else { String::new() };
        drop(lane);

        if !core_tail.is_empty() {
            log::warn!(
                "liveness {group}: batch {} dialled {} servers, none connected — core log tail: {}",
                round - 1,
                targets.len(),
                core_tail
            );
        }

        let stored = flush(node_manager, group, &mut pending);
        emit_progress(app, &beat(group, &stats, true, false, stored));
        log::info!(
            "liveness {group}: batch {} of {} servers in {:.0}s ({}/{} measured)",
            round - 1,
            chunk.len(),
            batch_start.elapsed().as_secs_f64(),
            stats.tested,
            stats.total
        );

        // 停下来的三种原因里,只有"中转被实测拨不通"才会写死它,也只有"实在换不动
        // 了"才中止整轮。v0.2.113 之前这里一律 break,所以那句"下一轮会自动改用
        // 其他中转"从来不成立 —— 用户只能再点一次,再看一次同样的红字。
        let Some(stop) = stop else { continue };
        // 一条 lane 说的"不通"要换一条全新的核心复核过才算数(见 confirm_relay_dead)。
        let stop = match stop {
            LaneStop::RelayDown(reason) => {
                let name = relay.name().to_string();
                emit_progress(
                    app,
                    &message_beat(
                        group,
                        &stats,
                        &format!(
                            "中转节点「{name}」在这条测活核心上连拨 {attempts} 次都不通，先换一条全新的核心复核一遍再定它 —— 现在下结论还太早",
                            attempts = RELAY_ATTEMPTS
                        ),
                    ),
                );
                let verdict = confirm_relay_dead(lane_index_for(round - 1), &setup).await;
                match second_opinion(&name, reason, verdict) {
                    RelayFate::Convicted(reason) => LaneStop::RelayDown(reason),
                    RelayFate::NotConvicted(reason) => LaneStop::LaneSick(reason),
                }
            }
            other => other,
        };
        match stop {
            LaneStop::Yielded => {
                stats.aborted = true;
                stats.note = Some(format!(
                    "已让路给你正在使用的连接：测了 {}/{} 个，其余未测",
                    stats.tested, stats.total
                ));
                break;
            }
            LaneStop::RelayDown(reason) => {
                let dead_name = relay.name().to_string();
                let dead_id = setup.relay_id.clone();
                retire_relay(app, node_manager, &setup);
                stats.relay_retired = Some(dead_name.clone());
                let next = (rotations < RELAY_ROTATIONS_MAX)
                    .then(|| prepare_lane(app, node_manager, conn, None).ok())
                    .flatten()
                    .filter(|n| n.relay_id != dead_id);
                if let Some(next) = next {
                    rotations += 1;
                    setup = next;
                    relay = RelayWatch::new(setup.relay_name.clone());
                    let message = rotation_message(&dead_name, &reason, &setup.relay_name);
                    log::warn!("liveness {group}: {message}");
                    emit_progress(app, &message_beat(group, &stats, &message));
                    continue;
                }
                // 换不动了:要么列表里没有第二台中转,要么已经换到预算上限。
                stats.not_judged += 1;
                stats.aborted = true;
                let tail = if rotations > 0 {
                    format!(
                        "本轮已换用 {} 台中转，它们都拨不通 —— 这是这台机器出不去网络，不是出口节点的问题",
                        rotations
                    )
                } else {
                    RELAY_RETIRED_TAIL.to_string()
                };
                stats.note = Some(format!(
                    "中转节点「{}」连拨 {} 次都不通（{}）：本轮中止，{} 个节点未判定（不是它们不可用），已测 {}/{} 个{}；{}",
                    dead_name,
                    RELAY_ATTEMPTS,
                    reason,
                    stats.not_judged,
                    stats.tested,
                    stats.total,
                    unmeasured_tail(stats.total, stats.tested),
                    tail
                ));
                break;
            }
            LaneStop::LaneSick(reason) => {
                // 本地故障:中转一个都没被拨到,所以它既不能被定罪,这一轮也还有救
                // —— 换一条全新的 lane 就是比重拨三次更强的补救。
                if rebuilds < LANE_REBUILDS_MAX {
                    rebuilds += 1;
                    let message = local_fault_message(&reason);
                    log::warn!("liveness {group}: {message}");
                    emit_progress(app, &message_beat(group, &stats, &message));
                    continue;
                }
                stats.not_judged += 1;
                stats.aborted = true;
                stats.note = Some(format!(
                    "测活核心自身出了问题（{}），重启 {} 次仍未能把流量交给中转，本轮中止：已测 {}/{} 个{}，{} 个节点未判定（不是它们不可用，中转也没有因此被记为不可用）",
                    reason,
                    rebuilds,
                    stats.tested,
                    stats.total,
                    unmeasured_tail(stats.total, stats.tested),
                    stats.not_judged
                ));
                break;
            }
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
            if rotations > 0 {
                // 报最后那台中转的名字会撒谎:前面的节点是经被换掉的那几台测的。
                summary.push_str(&format!(
                    "；本轮换用过 {} 台中转，前面的节点是经旧中转测得的",
                    rotations
                ));
            }
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
    match commit_verdicts(node_manager, pending) {
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

    /// 冤案的形状:本机核心没能把流量交给中转,却说成"中转连拨 3 次都不通"。
    #[test]
    fn a_local_fault_blames_the_probe_core_and_convicts_nobody() {
        let message = local_fault_message("核心没能把流量交给中转(本地故障:connection refused)");
        assert!(message.contains("测活核心"), "{message}");
        assert!(
            !message.contains("不可用"),
            "本地故障的一句话里不许出现任何定罪: {message}"
        );
        assert!(!message.contains("本轮中止"), "{message}");
        assert!(
            message.contains("没有经过中转"),
            "要说清楚这次失败为什么不算节点的: {message}"
        );
    }

    /// 一台中转的死罪要**两条不同的核心**共同签字。旧核心说不通、刚起来的新核心
    /// 说得通,屏幕上必须是"出问题的是本机核心",不能把用户正在用的中转记死 ——
    /// v0.2.114 的现场就是几分钟前才实测 37Mbps 的中转被判了死刑。
    #[test]
    fn one_core_saying_no_is_not_a_death_sentence() {
        let fate = second_opinion(
            "HK 01",
            "8 秒内没有任何一个探测站点经它应答".to_string(),
            RelayVerdict::Reachable,
        );
        let RelayFate::NotConvicted(message) = fate else {
            panic!("新核心答得出来时绝不能定罪");
        };
        assert!(message.contains("不是中转"), "{message}");
        assert!(message.contains("HK 01"), "要点名是哪台中转没死: {message}");
        assert!(!message.contains("不可用"), "{message}");
    }

    #[test]
    fn two_cores_agreeing_is_the_only_death_sentence() {
        let fate = second_opinion(
            "HK 01",
            "旧核心:8 秒内无人应答".to_string(),
            RelayVerdict::Unreachable("新核心:8 秒内无人应答".to_string()),
        );
        let RelayFate::Convicted(message) = fate else {
            panic!("两条核心都拨不通就是证据,必须定罪");
        };
        assert!(message.contains("旧核心") && message.contains("新核心"), "{message}");
    }

    /// 复核本身没跑起来(新核心起不来)是第三种情况:它既不能定罪,也不能说没事。
    #[test]
    fn a_recheck_that_never_ran_convicts_nobody() {
        let fate = second_opinion(
            "HK 01",
            "旧核心:8 秒内无人应答".to_string(),
            RelayVerdict::LocalFault("复核用的新核心没能启动（binary not found）".to_string()),
        );
        let RelayFate::NotConvicted(message) = fate else {
            panic!("复核没跑起来就没有第二种意见: 不能定罪");
        };
        assert!(message.contains("新核心"), "{message}");
        assert!(!message.contains("记为不可用"), "{message}");
    }

    /// 比自检更硬的证据:这台中转**刚刚真的把某个节点的流量送出去了**。v0.2.113
    /// 的现场是同一轮 13 个节点经它拨通、第 14 个失败之后自检说它"连拨 3 次都不通"
    /// —— 有这份不在场证明在,自检的抖动根本不该进入定罪流程。
    #[test]
    fn a_relay_that_just_carried_a_successful_dial_is_not_asked_again() {
        use std::time::Duration;
        let now = std::time::Instant::now();
        note_carried_at("proof-carried-recent", now - Duration::from_secs(3));
        assert!(carried_within("proof-carried-recent", RELAY_CARRIED_PROOF, now).is_some());
    }

    /// 证明会过期,也只替它自己那一台说话:换过中转之后,上一台的功劳不能给新一台
    /// 脱罪,否则一轮里第一台中转的死罪就永远定不下来了。
    #[test]
    fn a_carried_proof_expires_and_never_covers_another_relay() {
        use std::time::Duration;
        let now = std::time::Instant::now();
        note_carried_at("proof-stale", now - (RELAY_CARRIED_PROOF + Duration::from_secs(2)));
        assert!(
            carried_within("proof-stale", RELAY_CARRIED_PROOF, now).is_none(),
            "过期之后的成功拨号不能再替中转脱罪"
        );
        note_carried_at("proof-other", now - Duration::from_secs(1));
        assert!(
            carried_within("proof-another", RELAY_CARRIED_PROOF, now).is_none(),
            "别的中转的功劳不能顶这一台的罪"
        );
    }

    /// 换机成功那一拍必须报出"改用谁",而且要让人看出来轮还没结束 —— 屏幕上那台
    /// 中转刚刚才被实测选用,再念一遍"中止"就是自相矛盾。
    #[test]
    fn switching_relay_names_the_replacement_and_keeps_the_sweep_running() {
        let message = rotation_message("HK 01", "8 秒内没有任何一个探测站点经它应答", "JP 02");
        assert!(message.contains("HK 01") && message.contains("JP 02"), "{message}");
        assert!(message.contains("继续测"), "换机之后轮还在跑: {message}");
        assert!(!message.contains("中止"), "{message}");
        assert!(
            message.contains(&format!("连拨 {RELAY_ATTEMPTS} 次")),
            "定罪的理由要和那句话里的次数一致: {message}"
        );
    }

    #[test]
    fn a_mid_pass_switch_beat_is_not_an_end_of_round() {
        let stats = GroupStats {
            tested: 13,
            total: 98,
            alive: 3,
            not_judged: 1,
            ..Default::default()
        };
        let p = message_beat("Residential", &stats, "已改用「JP 02」继续测");
        assert!(p.running && !p.aborted && !p.done, "这一拍之后还要接着拨");
        assert_eq!(p.message.as_deref(), Some("已改用「JP 02」继续测"));
        assert_eq!(p.tested, 13, "换中转不能让进度倒退");
        assert!(p.persisted, "换机之前已经把已有结论写回库存");
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
