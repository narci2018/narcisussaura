import React, { useEffect, useRef, useState } from 'react';
import { Zap, RefreshCw, ChevronDown, Check } from 'lucide-react';
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
  const triggerLabel = current
    ? `[${current.country_code}] ${current.name}`
    : `⚡ 自动优选最快节点 ${bestCandidate ? `(${bestCandidate.country_code} · ${bestCandidate.latency_ms ?? '~'}ms)` : ''}`;

  const pick = (id: string) => {
    setSelectedRelayNodeId(id);
    setOpen(false);
  };

  return (
    <div className="mx-3 my-2.5 p-3 bg-[#121520]/95 border border-blue-500/25 rounded-2xl flex flex-col justify-between gap-3 shadow-lg shadow-black/20 backdrop-blur-sm">
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
                    <span className="truncate">
                      ⚡ 自动优选最快节点 {bestCandidate ? `(${bestCandidate.country_code} · ${bestCandidate.latency_ms ?? '~'}ms)` : ''}
                    </span>
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
                            ({node.protocol.toUpperCase()} · {node.latency_ms ? `${node.latency_ms}ms` : 'Alive'})
                          </span>
                        </span>
                        {active && <Check className="w-3.5 h-3.5 shrink-0" />}
                      </button>
                    );
                  })}
                  {relayCandidates.length === 0 && (
                    <div className="px-3.5 py-2.5 text-[12px] text-gray-500">暂无候选节点，点右侧按钮刷新</div>
                  )}
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
