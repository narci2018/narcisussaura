import React, { useState, useEffect } from 'react';
import { Home, RefreshCw, CheckCircle2, Signal, ArrowUpRight, Search, Globe2, Building2, Square, Zap } from 'lucide-react';
import { api } from '../../../services/api';
import { useAppStore } from '../../../stores/appStore';
import { RelayBar } from './RelayBar';
import { LivenessBadge, LivenessProgressTag, LastProbeBanner, ProbeOutcomeLine, byLiveness, cardVerdict, isLivenessRunning, latencyText, ProbingBanner, ProbingChip, withProbeDeadline } from './liveness';
import { matchNodeKeywords } from '../../../components/SimpleMode/countries';
import { UnifiedNode } from '../../../types';

export const ResidentialView: React.FC = () => {
  const {
    status,
    connectedNode,
    connect,
    disconnect,
    nodes,
    loadResidentialNodes,
    residentialSubUrl,
    relayEnabled,
    setRelayEnabled,
    livenessProgress,
    probeOutcomes,
    setProbeOutcome,
    lastProbe,
    dismissLastProbe,
    applyMeasuredNode,
  } = useAppStore();

  // 只用 store 里的节点:测活每写回一批结论都会 refreshNodes,本地再存一份快照
  // 就会把"未测"永远留在卡片上。清单本身由启动任务采集一次,进页面不再拉。
  const residentialNodes = React.useMemo(() => nodes.filter((n) => n.group === 'Residential'), [nodes]);
  const [loading, setLoading] = useState(false);
  const [probing, setProbing] = useState<string | null>(null);
  const [probeStartedAt, setProbeStartedAt] = useState(0);
  const [measureError, setMeasureError] = useState<string | null>(null);
  const [now, setNow] = useState(Date.now());
  const [search, setSearch] = useState('');

  // Requirement #2: Default enable relay proxy for residential broadband nodes
  useEffect(() => {
    if (!relayEnabled) {
      setRelayEnabled(true);
    }
  }, []);

  const handleSync = async () => {
    if (!residentialSubUrl) return;
    setLoading(true);
    try {
      await loadResidentialNodes();
    } catch (e) {
      console.error('Failed to sync residential nodes:', e);
    } finally {
      setLoading(false);
    }
  };

  const beat = livenessProgress['Residential'];
  const measuring = isLivenessRunning(beat, now);
  // 一轮在跑时靠新播报刷新;一轮死了就没有新播报,所以自己也要定时看一眼,
  // 否则"没有响应"这条永远来不及显示。单节点拨号中要按秒走,那个计数才动。
  useEffect(() => {
    if (probing !== null) {
      const t = setInterval(() => setNow(Date.now()), 1000);
      return () => clearInterval(t);
    }
    if (!beat?.running) return;
    const t = setInterval(() => setNow(Date.now()), 3000);
    return () => clearInterval(t);
  }, [beat?.running, probing]);

  // 手机端资源有限,一个节点实测约 5 秒,整轮只能由用户主动发起。
  const measureAll = async () => {
    setMeasureError(null);
    try {
      await api.measureGroupNodes('Residential');
    } catch (e) {
      // 挨着按钮说,不用用户去顶部找那条会被顶掉的横幅。
      setMeasureError(`测活没能开始：${e instanceof Error ? e.message : String(e)}`);
    }
  };

  // 单节点测活:结论一定要落到这张卡片上,而且一定要落成一句人话。四类结论(可用 /
  // 出口不可用 / 中转不可用 / 未判定)由后端给出,前端只负责在兜底期限到了还没回音时自己
  // 补一句 —— 静默就是用户不知道自己有没有测过。
  const measureOne = async (node: UnifiedNode) => {
    setProbing(node.id);
    setProbeStartedAt(Date.now());
    // 上一台节点的结论不能压在这一轮拨号头上:卡片会重排,横幅不会 —— 用户看到的
    // 就是"下面写着出口节点不可用,上面这张卡却还在测活"(v0.2.116 投诉)。
    dismissLastProbe();
    try {
      const outcome = await withProbeDeadline(api.measureNode(node.id));
      setProbeOutcome(node.id, outcome, node.name);
      // 只就地改这一行,不重拉清单:全量刷新 + 重排会让刚测完的卡片跑到大列表
      // 尽头,用户看到的就是"提示一闪而过,状态还是未测"(v0.2.114 投诉)。
      if (outcome.node) applyMeasuredNode(outcome.node);
    } finally {
      setProbing(null);
    }
  };

  const probingNode = probing === null ? null : residentialNodes.find((n) => n.id === probing) ?? null;
  // 刚测过的那一张钉在最前面:用户问的就是它,不该被排序挪走。
  const pinnedId = lastProbe?.nodeId ?? null;
  // 一条隧道只容得下一个连接:正在连接时,别的卡片的 Connect 必须按不动,直到用户
  // 点"终止"。只认 connecting 这一个状态 —— 连接失败会把状态留在 error,拿它当
  // "忙"就把所有按钮永久锁死了(v0.2.109 的教训)。
  const connectBusy = status === 'connecting';
  const filtered = residentialNodes
    .filter((n) => matchNodeKeywords(n, search))
    .sort((a, b) =>
      a.id === pinnedId ? -1 : b.id === pinnedId ? 1 : byLiveness(a, b)
    );
  const lastHere = pinnedId && residentialNodes.some((n) => n.id === pinnedId) ? lastProbe : null;

  return (
    <div className="flex-1 flex flex-col h-full bg-[#08090d] text-gray-100 overflow-hidden">
      {/* Top Header */}
      <div className="px-4 py-3.5 border-b border-[#1b1f2e] bg-[#0d0f17]/80 flex flex-col justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-amber-500/10 border border-amber-500/20 flex items-center justify-center text-amber-400">
              <Home className="w-4 h-4" />
            </div>
            <h1 className="text-lg font-bold tracking-tight text-gray-100">Premium Residential IP Network</h1>
            <span className="px-2 py-0.5 text-[12px] font-semibold uppercase tracking-wider rounded bg-amber-500/20 text-amber-300 border border-amber-500/30">
              优质住宅IP
            </span>
          </div>
          <p className="text-[13px] text-gray-400 mt-1">
            Authentic global residential broadband and ISP dynamic endpoints for maximum anonymity and anti-censorship.
          </p>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={handleSync}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-amber-600 active:bg-amber-500 disabled:opacity-50 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-amber-600/30"
            title="重新拉取住宅节点清单(不会自动测活)"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>{loading ? 'Fetching...' : 'Sync Residential'}</span>
          </button>
          <button
            onClick={measureAll}
            disabled={measuring || residentialNodes.length === 0}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-600 active:bg-emerald-500 disabled:opacity-50 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-emerald-600/30"
            title="通过当前中转节点逐个真连接测试整份名单,一个节点约 5 秒"
          >
            <Zap className={`w-3.5 h-3.5 ${measuring ? 'animate-pulse' : ''}`} />
            <span>{measuring ? '测活中...' : '测活全部节点'}</span>
          </button>
        </div>

        {measureError && (
          <div className="text-[12px] leading-snug text-red-300">{measureError}</div>
        )}

        {/* 刚才那一次单节点测活的结论钉在这里:卡片会随重排走远,这句话不能走。
            正在拨下一个时,这里改说"是谁在测、测了几秒",不拿旧结论冒充新结论。 */}
        {probingNode ? (
          <ProbingBanner nodeName={probingNode.name} startedAt={probeStartedAt} now={now} />
        ) : lastHere ? (
          <LastProbeBanner
            outcome={lastHere.outcome}
            nodeName={lastHere.nodeName}
            onDismiss={dismissLastProbe}
          />
        ) : null}
      </div>

      {/* Filter and Search Bar */}
      <div className="px-6 py-3 border-b border-[#171a26] bg-[#0c0e14] flex items-center justify-between gap-4">
        <div className="relative flex-1 max-w-md">
          <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Filter by country, ISP, server IP, region..."
            className="w-full bg-[#131620] border border-[#222738] rounded-xl pl-9 pr-3 py-1.5 text-[13px] text-gray-200 placeholder-gray-500 focus:outline-none focus:border-amber-500/50"
          />
        </div>
        <div className="flex flex-col items-end gap-1 shrink-0">
          <LivenessProgressTag progress={livenessProgress['Residential']} />
          <div className="text-[13px] text-gray-500 font-mono">
            {filtered.length} residential nodes available
          </div>
        </div>
      </div>

      {/* Relay Proxy Toolbar */}
      <RelayBar description="优质住宅IP为真实家庭/商业宽带，国内直连易受阻断。开启链式中转将通过您的翻墙节点中继访问，显著提升建联成功率。" />

      {/* Relays List */}
      <div className="flex-1 overflow-y-auto p-6">
        {filtered.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-gray-500">
            <Globe2 className="w-10 h-10 mb-2 opacity-30 text-amber-400" />
            <p className="text-sm">No residential nodes found.</p>
            <button
              onClick={handleSync}
              className="mt-3 text-[13px] text-amber-400 active:underline flex items-center gap-1"
            >
              <RefreshCw className="w-3 h-3" /> Click to fetch nodes
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4">
            {filtered.map((node) => {
              const isConnected = connectedNode?.id === node.id && status === 'connected';
              const isConnecting = connectedNode?.id === node.id && status === 'connecting';
              // 徽标/延迟位和卡片下面那句结论必须同源,不能一个说"未测"一个说"不可用"。
              const verdict = cardVerdict(node, probeOutcomes[node.id]);
              const ispName = (node.config as any)?.isp || node.city || 'Residential ISP';

              return (
                <div
                  key={node.id}
                  className={`relative flex flex-col justify-between p-4 rounded-2xl border transition-all ${
                    isConnected
                      ? 'bg-gradient-to-br from-amber-950/30 to-[#141108] border-amber-500/40 shadow-lg shadow-amber-950/20'
                      : 'bg-[#11141e] border-[#1d2232] active:border-[#2e3650] active:bg-[#141824]'
                  }`}
                >
                  <div>
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="flex items-center gap-2">
                        <div className="w-8 h-8 rounded-xl bg-[#1b2030] flex items-center justify-center text-[13px] font-bold text-amber-400">
                          {node.country_code}
                        </div>
                        <div>
                          <div className="text-[13px] font-semibold text-gray-100 flex items-center gap-1.5">
                            <span>{node.country_name || 'Residential Node'}</span>
                            {isConnected && (
                              <span className="w-2 h-2 rounded-full bg-amber-400 animate-pulse" />
                            )}
                          </div>
                          <div className="text-[12px] text-gray-500 font-mono">
                            {node.address}:{node.port}
                          </div>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 text-[12px] font-mono uppercase tracking-wider rounded bg-amber-500/10 text-amber-300 border border-amber-500/20">
                        Residential
                      </span>
                    </div>

                    <div className="flex flex-wrap items-center gap-1.5 my-3">
                      {probing === node.id ? (
                        <ProbingChip startedAt={probeStartedAt} now={now} />
                      ) : (
                        <LivenessBadge status={verdict.status} measuredAt={verdict.measuredAt} />
                      )}
                      <span className="flex items-center gap-1 px-1.5 py-0.5 bg-[#171b28] text-amber-300/90 rounded text-[12px] border border-[#23293d]">
                        <Building2 className="w-3 h-3 text-amber-400" />
                        <span className="max-w-[140px] truncate">{ispName}</span>
                      </span>
                      {node.speed_bps && node.speed_bps > 0 && (
                        <span className="px-1.5 py-0.5 bg-[#171b28] text-emerald-400 rounded text-[12px] border border-[#23293d] font-mono">
                          {Math.round(node.speed_bps / (1024 * 1024))} Mbps
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="pt-3 border-t border-[#1c2133] flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2">
                      <div
                        className="flex items-center gap-1.5 text-[12px] font-mono text-gray-400"
                        title="经中转真连接测得的往返延迟"
                      >
                        <Signal className={`w-3.5 h-3.5 ${node.latency_ms && node.latency_ms > 0 ? 'text-emerald-400' : 'text-gray-500'}`} />
                        <span>{probing === node.id ? '拨号中…' : latencyText(node, verdict.status)}</span>
                      </div>
                      <button
                        onClick={() => measureOne(node)}
                        disabled={probing !== null}
                        className="flex items-center gap-1 px-2 py-1 rounded-lg border border-[#2a3145] text-[12px] text-gray-300 active:border-emerald-500/50 active:text-emerald-300 disabled:opacity-40 transition-colors"
                        title="只测这一个节点:经当前中转建立一次真实连接"
                      >
                        <Zap className={`w-3.5 h-3.5 ${probing === node.id ? 'animate-pulse text-emerald-400' : ''}`} />
                        <span>{probing === node.id ? '测活中' : '测活'}</span>
                      </button>
                    </div>

                    {isConnected ? (
                      <button
                        onClick={() => disconnect()}
                        className="px-3 py-1.5 bg-amber-600/20 active:bg-red-600/30 text-amber-400 active:text-red-400 border border-amber-500/40 rounded-xl text-[13px] font-medium transition-all flex items-center gap-1.5"
                      >
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        <span>Connected</span>
                      </button>
                    ) : isConnecting ? (
                      <div className="flex items-center gap-2">
                        <span className="flex items-center gap-1.5 text-[13px] font-medium text-red-300">
                          <span className="w-2 h-2 rounded-full bg-red-400 animate-pulse" />
                          <span>连接中</span>
                        </span>
                        <button
                          onClick={() => disconnect()}
                          className="px-3 py-1.5 bg-red-600/25 active:bg-red-600/40 border border-red-500/40 text-red-300 active:text-red-100 rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-red-500/10 flex items-center gap-1.5 active:scale-95"
                          title="点击终止连接"
                        >
                          <Square className="w-3.5 h-3.5 fill-red-400 text-red-400" />
                          <span>终止</span>
                        </button>
                      </div>
                    ) : (
                      <button
                        onClick={() => connect(node.id)}
                        disabled={connectBusy}
                        className="px-3.5 py-1.5 bg-amber-600 active:bg-amber-500 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-amber-600/20 flex items-center gap-1.5 active:scale-95 disabled:opacity-40"
                        title={connectBusy ? '正在连接其他节点,先点“终止”再换' : undefined}
                      >
                        <ArrowUpRight className="w-3.5 h-3.5" />
                        <span>Connect</span>
                      </button>
                    )}
                  </div>

                  <ProbeOutcomeLine outcome={probeOutcomes[node.id]} />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};
