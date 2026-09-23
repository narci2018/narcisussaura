import React, { useEffect } from 'react';
import { Zap, RefreshCw, ChevronDown } from 'lucide-react';
import { useAppStore } from '../../../stores/appStore';

interface RelayBarProps {
  description?: string;
}

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
  } = useAppStore();

  useEffect(() => {
    if (relayCandidates.length === 0) {
      fetchRelayCandidates();
    }
  }, []);

  const bestCandidate = relayCandidates.length > 0 ? relayCandidates[0] : null;

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
                <option value="auto">
                  ⚡ 自动优选最快节点 {bestCandidate ? `(${bestCandidate.country_code} · ${bestCandidate.latency_ms ?? '~'}ms)` : ''}
                </option>
                {relayCandidates.map((node) => (
                  <option key={node.id} value={node.id}>
                    [{node.country_code}] {node.name} ({node.protocol.toUpperCase()} · {node.latency_ms ? `${node.latency_ms}ms` : 'Alive'})
                  </option>
                ))}
              </select>
              <ChevronDown className="w-3.5 h-3.5 text-gray-400 absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            </div>

            <button
              onClick={() => fetchRelayCandidates()}
              title="刷新可用中转候选节点"
              className="p-1.5 rounded-lg bg-[#191d2c] hover:bg-[#22283d] text-gray-400 hover:text-gray-200 border border-[#2e354e] transition-colors"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
