import React, { useEffect, useRef, useState } from 'react';
import { Zap, RefreshCw, ChevronDown, Check, Activity } from 'lucide-react';
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
    rankRelays,
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

  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLDivElement>(null);

  // Tap anywhere outside closes the popover (WebView has no native blur for this).
  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent | TouchEvent) => {
      if (boxRef.current && !boxRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', onDocDown);
    document.addEventListener('touchstart', onDocDown);
    return () => {
      document.removeEventListener('mousedown', onDocDown);
      document.removeEventListener('touchstart', onDocDown);
    };
  }, [open]);

  const current = selectedRelayNodeId === 'auto' ? null : relayCandidates.find((n) => n.id === selectedRelayNodeId);
  const triggerLabel = current ? `[${current.country_code}] ${current.name}` : autoLabel;

  const pick = (id: string) => {
    setSelectedRelayNodeId(id);
    setOpen(false);
  };

  return (
    // backdrop-blur below makes this card its own stacking context, so the
    // popover's z-50 cannot escape it — while open, the CARD itself must lift
    // above the later-DOM node cards that were covering it (v0.2.103 field
    // bug). z-40 stays under the global error banner (z-50).
    <div className={`mx-3 my-2.5 p-3 bg-[#121520]/95 border border-blue-500/25 rounded-2xl flex flex-col justify-between gap-3 shadow-lg shadow-black/20 backdrop-blur-sm ${open ? 'relative z-40' : ''}`}>
      <div className="flex items-start gap-3">
        <div className={`mt-0.5 p-2 rounded-xl flex items-center justify-center transition-colors ${
          relayEnabled ? 'bg-blue-500/15 text-blue-400 border border-blue-500/30' : 'bg-gray-800 text-gray-500 border border-gray-700'
        }`}>
          <Zap className="w-4 h-4" />
        </div>
        <div>
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-semibold text-gray-200">
              链式中转加速 (Relay Proxy)
            </span>
            <span className={`px-1.5 py-0.5 text-[12px] font-bold rounded uppercase tracking-wide border ${
              relayEnabled
                ? 'bg-blue-500/20 text-blue-300 border-blue-500/30'
                : 'bg-gray-800 text-gray-400 border-gray-700'
            }`}>
              {relayEnabled ? '已启用 (Active)' : '已关闭 (Direct)'}
            </span>
          </div>
          <p className="text-[12px] text-gray-400 mt-0.5 leading-relaxed">
            {description}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2.5 self-end shrink-0">
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
            <div className="relative" ref={boxRef}>
              <button
                onClick={() => setOpen((v) => !v)}
                className={`flex items-center gap-1.5 bg-[#191d2c] active:bg-[#1f2437] text-gray-200 text-[13px] font-medium pl-3 pr-2.5 py-1.5 rounded-xl border transition-colors max-w-[240px] ${
                  open ? 'border-blue-500' : 'border-[#2e354e]'
                }`}
              >
                <span className="truncate">{triggerLabel}</span>
                <ChevronDown className={`w-3.5 h-3.5 text-gray-400 shrink-0 transition-transform ${open ? 'rotate-180' : ''}`} />
              </button>

              {open && (
                <div className="absolute right-0 top-full mt-1.5 z-50 w-[min(320px,80vw)] max-h-72 overflow-y-auto overscroll-contain bg-[#151926] border border-[#2e354e] rounded-2xl shadow-2xl shadow-black/60 backdrop-blur-md py-1.5">
                  <button
                    onClick={() => pick('auto')}
                    className={`w-full flex items-center justify-between gap-2 px-3.5 py-2.5 text-left text-[13px] transition-colors ${
                      selectedRelayNodeId === 'auto' ? 'bg-blue-500/15 text-blue-300' : 'text-gray-200 active:bg-[#1f2437]'
                    }`}
                  >
                    <span className="truncate">{autoLabel}</span>
                    {selectedRelayNodeId === 'auto' && <Check className="w-3.5 h-3.5 shrink-0" />}
                  </button>
                  <div className="my-1.5 border-t border-[#232a3d]" />
                  {relayCandidates.map((node) => {
                    const active = selectedRelayNodeId === node.id;
                    return (
                      <button
                        key={node.id}
                        onClick={() => pick(node.id)}
                        className={`w-full flex items-center justify-between gap-2 px-3.5 py-2.5 text-left text-[13px] transition-colors ${
                          active ? 'bg-blue-500/15 text-blue-300' : 'text-gray-300 active:bg-[#1f2437]'
                        }`}
                      >
                        <span className="truncate">
                          [{node.country_code}] {node.name}
                          <span className="text-gray-500 ml-1.5">
                            ({node.protocol.toUpperCase()} · {node.latency_ms ? `${node.latency_ms}ms` : '未测'}
                            {formatBandwidth(node.speed_bps) ? ` · ${formatBandwidth(node.speed_bps)}` : ''})
                          </span>
                        </span>
                        {active && <Check className="w-3.5 h-3.5 shrink-0" />}
                      </button>
                    );
                  })}
                  {relayCandidates.length === 0 && (
                    <div className="px-3.5 py-2.5 text-[12px] text-gray-500">暂无候选节点，点右侧按钮刷新</div>
                  )}
                  <div className="my-1.5 border-t border-[#232a3d]" />
                  <button
                    onClick={() => {
                      setOpen(false);
                      rankRelays();
                    }}
                    disabled={isRankingRelays}
                    className="w-full flex items-center gap-2 px-3.5 py-2.5 text-left text-[13px] text-gray-300 active:bg-[#1f2437] transition-colors disabled:opacity-60"
                  >
                    <Activity className={`w-3.5 h-3.5 shrink-0 ${isRankingRelays ? 'animate-pulse text-blue-400' : 'text-blue-400'}`} />
                    <span className="truncate">
                      {isRankingRelays ? '正在真实建联测速…' : '重新实测延迟与带宽'}
                    </span>
                  </button>
                </div>
              )}
            </div>

            <button
              onClick={() => fetchRelayCandidates()}
              title="刷新可用中转候选节点"
              className="p-1.5 rounded-lg bg-[#191d2c] active:bg-[#22283d] text-gray-400 active:text-gray-200 border border-[#2e354e] transition-colors"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
