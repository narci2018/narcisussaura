import React from 'react';
import { X, Zap, Signal } from 'lucide-react';
import { UnifiedNode } from '../../types';

function formatSpeed(bps?: number | null): string {
  if (!bps || bps <= 0) return '—';
  if (bps < 1024) return `${bps} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
}

function formatLatency(ms?: number | null): string {
  if (!ms || ms <= 0) return '—';
  return `${ms} ms`;
}

interface NodePickerDialogProps {
  countryName: string;
  nodes: UnifiedNode[];
  selectedNodeId: string | null;
  onSelect: (nodeId: string) => void;
  onClose: () => void;
}

export const NodePickerDialog: React.FC<NodePickerDialogProps> = ({
  countryName,
  nodes,
  selectedNodeId,
  onSelect,
  onClose,
}) => {
  const sorted = [...nodes].sort((a, b) => {
    const aAlive = a.status !== 'dead' ? 0 : 1;
    const bAlive = b.status !== 'dead' ? 0 : 1;
    if (aAlive !== bAlive) return aAlive - bAlive;
    
    let aSpeed = a.speed_bps || 0;
    let aLat = a.latency_ms || 99999;
    if (aSpeed === 0 || aLat === 99999) {
      const aMatch = a.name.match(/(\d+)ms-(\d+)Mbps/);
      if (aMatch) {
        if (aLat === 99999) aLat = parseInt(aMatch[1], 10);
        if (aSpeed === 0) aSpeed = parseInt(aMatch[2], 10) * 1024 * 1024 / 8;
      }
    }
    
    let bSpeed = b.speed_bps || 0;
    let bLat = b.latency_ms || 99999;
    if (bSpeed === 0 || bLat === 99999) {
      const bMatch = b.name.match(/(\d+)ms-(\d+)Mbps/);
      if (bMatch) {
        if (bLat === 99999) bLat = parseInt(bMatch[1], 10);
        if (bSpeed === 0) bSpeed = parseInt(bMatch[2], 10) * 1024 * 1024 / 8;
      }
    }

    if (bSpeed !== aSpeed) return bSpeed - aSpeed;
    return aLat - bLat;
  });

  return (
    <div className="fixed inset-0 z-50 flex items-end sm:items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-[#0f1219] border border-[#2a3050] rounded-t-3xl sm:rounded-3xl shadow-2xl shadow-black/60 w-full max-w-sm max-h-[70vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-5 pb-3 border-b border-[#1e2535]">
          <div>
            <h3 className="text-base font-bold text-gray-100">{countryName} 节点列表</h3>
            <p className="text-xs text-gray-500 mt-0.5">{sorted.length} 个节点可选</p>
          </div>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 transition-colors p-1"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Node list */}
        <div className="overflow-y-auto flex-1 px-4 py-3 space-y-2">
          {sorted.map((node) => {
            const isDead = node.status === 'dead';
            const isSelected = node.id === selectedNodeId;
            return (
              <button
                key={node.id}
                onClick={() => {
                  if (!isDead) {
                    onSelect(node.id);
                    onClose();
                  }
                }}
                disabled={isDead}
                className={`w-full flex items-center justify-between px-4 py-3 rounded-2xl border transition-all text-left ${
                  isSelected
                    ? 'bg-indigo-600/20 border-indigo-500/50 shadow-sm shadow-indigo-500/10'
                    : isDead
                    ? 'bg-[#13161f] border-[#1a1f2e] opacity-40 cursor-not-allowed'
                    : 'bg-[#13161f] border-[#1e2535] hover:border-[#3a4258] hover:bg-[#181d2a]'
                }`}
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span
                      className={`w-2 h-2 rounded-full shrink-0 ${
                        isDead
                          ? 'bg-gray-600'
                          : node.status === 'alive'
                          ? 'bg-emerald-400'
                          : 'bg-yellow-400'
                      }`}
                    />
                    <span className="text-sm font-medium text-gray-200 truncate">{node.name}</span>
                    {isSelected && (
                      <span className="text-[10px] font-bold text-indigo-400 bg-indigo-500/10 px-1.5 py-0.5 rounded-md shrink-0">
                        已选
                      </span>
                    )}
                  </div>
                  <div className="text-[11px] text-gray-500 mt-0.5 ml-4 font-mono uppercase">
                    {node.protocol}
                  </div>
                </div>

                <div className="flex items-center gap-3 ml-3 shrink-0">
                  {node.speed_bps && node.speed_bps > 0 ? (
                    <div className="flex items-center gap-1 text-emerald-400">
                      <Zap className="w-3 h-3" />
                      <span className="text-xs font-mono">{formatSpeed(node.speed_bps)}</span>
                    </div>
                  ) : node.latency_ms && node.latency_ms > 0 ? (
                    <div className="flex items-center gap-1 text-blue-400">
                      <Signal className="w-3 h-3" />
                      <span className="text-xs font-mono">{formatLatency(node.latency_ms)}</span>
                    </div>
                  ) : (
                    <span className="text-xs text-gray-600">未测速</span>
                  )}
                </div>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
};
