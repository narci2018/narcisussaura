import React, { useEffect } from 'react';
import { Zap, RefreshCw, ChevronDown } from 'lucide-react';
import { useAppStore } from '../../../stores/appStore';

interface RelayBarProps {
  description?: string;
}

/** 实测带宽是字节/秒,换算成看得懂的 MB/s */
const formatBandwidth = (bytesPerSecond?: number | null): string | null => {
  if (!bytesPerSecond || bytesPerSecond <= 0) return null;
  return `${(bytesPerSecond / 1048576).toFixed(1)}MB/s`;
};

export const RelayBar: React.FC<RelayBarProps> = ({
  description = '国内网络直连该网络易受 GFW 阻断，开启链式中转将通过您的翻墙节点中继加速，保障 100% 成功建联。',
}) => {
  const {
    relayEnabled,
    setRelayEnabled,
    selectedRelayNodeId,
    setSelectedRelayNodeId,
    relayCandidates,
    fetchRelayCandidates,
    preferredRelay,
    isRankingRelays,
    refreshRelays,
    relayRank,
  } = useAppStore();

  useEffect(() => {
    if (relayCandidates.length === 0) {
      fetchRelayCandidates();
    }
  }, []);

  // 「自动优选」不再等于列表第一个:启动期实测出的胜者才算数。
  // 没实测过之前退回启发式首位,且不显示带宽(那是握手延迟而已)。
  const measured = preferredRelay?.preferred_id
    ? relayCandidates.find((n) => n.id === preferredRelay.preferred_id) ?? null
    : null;
  const bestCandidate = measured ?? (relayCandidates.length > 0 ? relayCandidates[0] : null);
  const autoDetail = bestCandidate
    ? [
        bestCandidate.country_code,
        bestCandidate.latency_ms != null ? `${bestCandidate.latency_ms}ms` : null,
        measured ? formatBandwidth(preferredRelay?.speed_bps ?? bestCandidate.speed_bps) : null,
      ].filter(Boolean).join(' · ')
    : '';
  const autoLabel = `⚡ 自动优选${measured ? '（已实测）' : ''}${autoDetail ? ` (${autoDetail})` : ''}`;

  return (
    <div className="mx-6 my-2.5 p-3 bg-[#121520]/95 border border-blue-500/25 rounded-2xl flex flex-col md:flex-row md:items-center justify-between gap-3 shadow-lg shadow-black/20 backdrop-blur-sm">
      <div className="flex items-start gap-3">
        <div className={`mt-0.5 p-2 rounded-xl flex items-center justify-center transition-colors ${
          relayEnabled ? 'bg-blue-500/15 text-blue-400 border border-blue-500/30' : 'bg-gray-800 text-gray-500 border border-gray-700'
        }`}>
          <Zap className="w-4 h-4" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-gray-200">
              链式中转加速 (Relay Proxy)
            </span>
            <span className={`px-1.5 py-0.5 text-[10px] font-bold rounded uppercase tracking-wide border ${
              relayEnabled
                ? 'bg-blue-500/20 text-blue-300 border-blue-500/30'
                : 'bg-gray-800 text-gray-400 border-gray-700'
            }`}>
              {relayEnabled ? '已启用 (Active)' : '已关闭 (Direct)'}
            </span>
          </div>
          <p className="text-[11px] text-gray-400 mt-0.5 leading-relaxed">
            {description}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2.5 self-end md:self-auto shrink-0">
        {/* Toggle Switch */}
        <button
          onClick={() => setRelayEnabled(!relayEnabled)}
          className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
            relayEnabled ? 'bg-blue-600' : 'bg-gray-700'
          }`}
          title={relayEnabled ? '点击关闭中转加速' : '点击开启中转加速'}
        >
          <span
            className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
              relayEnabled ? 'translate-x-5' : 'translate-x-0'
            }`}
          />
        </button>

        {/* Relay Node Selector */}
        {relayEnabled && (
          <div className="flex items-center gap-1.5">
            <div className="relative">
              <select
                value={selectedRelayNodeId}
                onChange={(e) => setSelectedRelayNodeId(e.target.value)}
                className="appearance-none bg-[#191d2c] hover:bg-[#1f2437] text-gray-200 text-xs font-medium pl-3 pr-8 py-1.5 rounded-xl border border-[#2e354e] focus:outline-none focus:border-blue-500 transition-colors max-w-[240px] truncate"
              >
                <option value="auto">{autoLabel}</option>
                {relayCandidates.map((node) => (
                  <option key={node.id} value={node.id}>
                    [{node.country_code}] {node.name} ({node.protocol.toUpperCase()} ·{' '}
                    {node.latency_ms ? `${node.latency_ms}ms` : '未测'}
                    {formatBandwidth(node.speed_bps) ? ` · ${formatBandwidth(node.speed_bps)}` : ''}
                    {node.status === 'dead' ? ' · 实测不通' : ''})
                  </option>
                ))}
              </select>
              <ChevronDown className="w-3.5 h-3.5 text-gray-400 absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            </div>

            {/* 一个按钮做完"刷新"该做的事:拉最新候选 → 真实建联实测 → 把选择
                落到可用那台上。以前这里有两个按钮,一个只重拉列表(按完什么都没变),
                另一个才实测;用户按的是前者,于是"刷新没有价值"。 */}
            <button
              onClick={() => refreshRelays()}
              disabled={isRankingRelays}
              title={isRankingRelays ? '正在逐个真实建联测速…' : '刷新候选并实测延迟与带宽，自动选用当前可用的中转'}
              className="p-1.5 rounded-lg bg-[#191d2c] hover:bg-[#22283d] text-gray-400 hover:text-gray-200 border border-[#2e354e] transition-colors disabled:opacity-60"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isRankingRelays ? 'animate-spin text-blue-400' : ''}`} />
            </button>
          </div>
        )}
      </div>

      {/* 刷新这一下必须留下一句话:成功是选中了谁,失败是为什么没能测。 */}
      {relayRank && (
        <p
          className={`self-start text-[11px] leading-relaxed ${
            relayRank.ok ? 'text-emerald-400' : 'text-amber-400'
          }`}
        >
          {relayRank.message}
        </p>
      )}
    </div>
  );
};
