import React, { useState } from 'react';
import {
  FolderSync,
  Plus,
  RefreshCw,
  Trash2,
  Clock,
  Layers,
  AlertCircle,
  Database,
  Calendar,
  Globe,
  Check,
} from 'lucide-react';
import { useAppStore } from '../stores/appStore';
import { Subscription } from '../types';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatRelativeTime(timestamp?: number | null): string {
  if (!timestamp) return 'Never updated';
  const diffSec = Math.floor(Date.now() / 1000 - timestamp);
  if (diffSec < 60) return 'Just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)} minutes ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} hours ago`;
  return `${Math.floor(diffSec / 86400)} days ago`;
}

export const SubscriptionsView: React.FC = () => {
  const {
    subscriptions,
    status,
    settings,
    updateSubViaProxy,
    setUpdateSubViaProxy,
    addSubscription,
    deleteSubscription,
    updateSubscription,
    restoreDefaultSubscriptions,
    updateAllSubscriptions,
  } = useAppStore();

  const [isAdding, setIsAdding] = useState(false);
  const [subName, setSubName] = useState('');
  const [subUrl, setSubUrl] = useState('');
  const [addingError, setAddingError] = useState<string | null>(null);

  const isConnected = status === 'connected';

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!subName.trim() || !subUrl.trim()) return;

    if (!subUrl.startsWith('http://') && !subUrl.startsWith('https://')) {
      setAddingError('Subscription URL must start with http:// or https://');
      return;
    }

    setAddingError(null);
    const ok = await addSubscription(subName.trim(), subUrl.trim());
    if (ok) {
      setSubName('');
      setSubUrl('');
      setIsAdding(false);
    }
  };

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6 max-w-[1600px] mx-auto w-full">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-[#1f2433] mb-6">
        <div>
          <h1 className="text-base font-bold text-gray-100 flex items-center gap-2">
            <FolderSync className="w-4 h-4 text-blue-400" />
            <span>Subscriptions</span>
          </h1>
          <p className="text-xs text-gray-500">
            Manage your Clash, Base64, and V2Ray subscription endpoints
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => restoreDefaultSubscriptions()}
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-[#171b26] hover:bg-[#202536] text-gray-200 hover:text-white text-xs font-medium border border-[#242b3d] shadow-sm transition-all"
            title="Import 15+ verified free GitHub subscription endpoints"
          >
            <Database className="w-3.5 h-3.5 text-emerald-400" />
            <span>Load Free Sources</span>
          </button>

          <button
            onClick={() => updateAllSubscriptions()}
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl bg-[#171b26] hover:bg-[#202536] text-gray-200 hover:text-white text-xs font-medium border border-[#242b3d] shadow-sm transition-all"
            title="Fetch latest nodes from all subscriptions"
          >
            <RefreshCw className="w-3.5 h-3.5 text-blue-400" />
            <span>Update All</span>
          </button>

          <button
            onClick={() => setIsAdding(!isAdding)}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-semibold shadow-sm shadow-blue-500/20 transition-all"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Add Custom</span>
          </button>
        </div>
      </div>

      {/* Proxy & Accelerator Control Bar */}
      <div className="bg-[#10131d] border border-[#1e2333] rounded-2xl p-3.5 mb-6 flex flex-wrap items-center justify-between gap-3 shadow-sm">
        <div className="flex items-center gap-3">
          <button
            onClick={() => setUpdateSubViaProxy(!updateSubViaProxy)}
            className={`flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-semibold border transition-all ${
              updateSubViaProxy
                ? 'bg-blue-600/20 text-blue-300 border-blue-500/40 shadow-sm shadow-blue-500/10'
                : 'bg-[#151924] text-gray-400 border-[#262e42] hover:text-gray-200'
            }`}
          >
            <div className={`w-3.5 h-3.5 rounded flex items-center justify-center border ${
              updateSubViaProxy ? 'bg-blue-500 border-blue-400 text-white' : 'border-gray-600'
            }`}>
              {updateSubViaProxy && <Check className="w-2.5 h-2.5 stroke-[3]" />}
            </div>
            <span>通过代理节点更新订阅 (Update via Proxy)</span>
          </button>

          <div className="h-4 w-px bg-[#222736]" />

          <div className="flex items-center gap-1.5 text-xs">
            <span className="text-gray-400">代理状态:</span>
            {isConnected ? (
              <span className="flex items-center gap-1 text-emerald-400 font-mono text-[11px] bg-emerald-500/10 border border-emerald-500/20 px-2 py-0.5 rounded-lg font-medium">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                <span>127.0.0.1:{settings.mixed_port} (已就绪 - 通过当前节点安全拉取)</span>
              </span>
            ) : (
              <span className="flex items-center gap-1 text-gray-400 font-mono text-[11px] bg-gray-800/40 border border-gray-700/30 px-2 py-0.5 rounded-lg">
                <span className="w-1.5 h-1.5 rounded-full bg-gray-500" />
                <span>未连接 (自动启用 GitHub 镜像加速通道)</span>
              </span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 text-[11px] text-gray-400 bg-[#0d0f17] border border-[#1b202e] px-3 py-1.5 rounded-xl">
          <Globe className="w-3.5 h-3.5 text-indigo-400" />
          <span>大陆防墙优化: GitHub 源自动附加高速镜像加速</span>
        </div>
      </div>

      {/* Add Subscription Form Modal/Drawer */}
      {isAdding && (
        <form
          onSubmit={handleAdd}
          className="bg-[#12151f] border border-blue-500/30 rounded-2xl p-5 mb-6 shadow-xl space-y-4 animate-in fade-in slide-in-from-top-2 duration-200"
        >
          <div className="text-xs font-semibold text-gray-200">Add New Subscription</div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="text-[11px] text-gray-400 block mb-1">Subscription Name</label>
              <input
                type="text"
                placeholder="e.g. Premium VIP Provider"
                value={subName}
                onChange={(e) => setSubName(e.target.value)}
                className="w-full bg-[#0a0c12] border border-[#23283a] rounded-xl px-3 py-2 text-xs text-gray-200 focus:outline-none focus:border-blue-500/60"
                required
              />
            </div>

            <div>
              <label className="text-[11px] text-gray-400 block mb-1">Subscription URL</label>
              <input
                type="url"
                placeholder="https://example.com/api/v1/client/subscribe?token=..."
                value={subUrl}
                onChange={(e) => setSubUrl(e.target.value)}
                className="w-full bg-[#0a0c12] border border-[#23283a] rounded-xl px-3 py-2 text-xs text-gray-200 focus:outline-none focus:border-blue-500/60 font-mono"
                required
              />
            </div>
          </div>

          {addingError && (
            <div className="text-xs text-red-400 flex items-center gap-1.5">
              <AlertCircle className="w-3.5 h-3.5" />
              <span>{addingError}</span>
            </div>
          )}

          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={() => setIsAdding(false)}
              className="px-3 py-1.5 rounded-lg text-xs text-gray-400 hover:text-gray-200 hover:bg-[#1a1e2b]"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium"
            >
              Save & Fetch
            </button>
          </div>
        </form>
      )}

      {/* Subscriptions Grid */}
      <div className="space-y-4">
        {subscriptions.length === 0 ? (
          <div className="h-56 flex flex-col items-center justify-center text-gray-500 text-xs border border-dashed border-[#202535] rounded-2xl">
            <FolderSync className="w-8 h-8 mb-2 opacity-30 text-gray-400" />
            <span className="font-medium text-gray-400">No subscriptions configured yet</span>
            <span className="text-[11px] text-gray-600 mt-0.5">
              Click &quot;Add Subscription&quot; to import your Clash or Base64 provider URL
            </span>
          </div>
        ) : (
          subscriptions.map((sub: Subscription) => {
            const isUpdating = sub.status === 'updating';
            const isError = sub.status === 'error';

            const usedBytes = sub.traffic ? sub.traffic.upload + sub.traffic.download : 0;
            const totalBytes = sub.traffic ? sub.traffic.total : 0;
            const usagePercent =
              totalBytes > 0 ? Math.min(100, Math.round((usedBytes / totalBytes) * 100)) : 0;

            return (
              <div
                key={sub.id}
                className="bg-[#12151f] border border-[#212637] rounded-2xl p-5 hover:border-[#2b3348] transition-all space-y-4 shadow-sm"
              >
                {/* Main Row */}
                <div className="flex items-start justify-between gap-4">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl bg-[#181d2a] border border-[#242b3d] flex items-center justify-center text-blue-400">
                      <Database className="w-5 h-5" />
                    </div>

                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-gray-100">{sub.name}</span>
                        {isUpdating && (
                          <span className="px-2 py-0.5 rounded text-[10px] bg-blue-500/10 text-blue-400 border border-blue-500/20 animate-pulse font-mono">
                            Updating...
                          </span>
                        )}
                        {isError && (
                          <span className="px-2 py-0.5 rounded text-[10px] bg-red-500/10 text-red-400 border border-red-500/20 font-mono">
                            Update Failed
                          </span>
                        )}
                        {!isUpdating && !isError && sub.last_updated && (
                          <span className="px-2 py-0.5 rounded text-[10px] bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-mono">
                            Active
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-3 text-[11px] text-gray-400 mt-1">
                        <span className="flex items-center gap-1">
                          <Layers className="w-3.5 h-3.5 text-gray-500" />
                          <span className="font-semibold text-gray-300">{sub.node_count}</span> nodes
                        </span>
                        <span>·</span>
                        <span className="flex items-center gap-1">
                          <Clock className="w-3.5 h-3.5 text-gray-500" />
                          <span>{formatRelativeTime(sub.last_updated)}</span>
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => updateSubscription(sub.id)}
                      disabled={isUpdating}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#171b26] hover:bg-[#202536] text-gray-300 hover:text-white text-xs font-medium border border-[#242b3d] transition-all disabled:opacity-50"
                      title="Update nodes now"
                    >
                      <RefreshCw
                        className={`w-3.5 h-3.5 ${isUpdating ? 'animate-spin text-blue-400' : ''}`}
                      />
                      <span>Update</span>
                    </button>

                    <button
                      onClick={() => deleteSubscription(sub.id)}
                      className="p-2 rounded-xl hover:bg-red-500/10 text-gray-500 hover:text-red-400 transition-colors"
                      title="Delete subscription"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                {/* Traffic Quota Bar (if provided by subscription server) */}
                {sub.traffic && totalBytes > 0 && (
                  <div className="bg-[#0b0d13] border border-[#1d2232] rounded-xl p-3 space-y-2">
                    <div className="flex items-center justify-between text-[11px]">
                      <span className="text-gray-400">Data Traffic Quota</span>
                      <span className="font-mono text-gray-300">
                        {formatBytes(usedBytes)} / {formatBytes(totalBytes)} ({usagePercent}%)
                      </span>
                    </div>

                    <div className="w-full h-1.5 rounded-full bg-[#1b2030] overflow-hidden">
                      <div
                        className={`h-full rounded-full transition-all duration-500 ${
                          usagePercent > 90
                            ? 'bg-red-500'
                            : usagePercent > 75
                            ? 'bg-yellow-500'
                            : 'bg-blue-500'
                        }`}
                        style={{ width: `${usagePercent}%` }}
                      />
                    </div>

                    {sub.traffic.expire && (
                      <div className="flex items-center gap-1 text-[10px] text-gray-500">
                        <Calendar className="w-3 h-3" />
                        <span>
                          Expires: {new Date(sub.traffic.expire * 1000).toLocaleDateString()}
                        </span>
                      </div>
                    )}
                  </div>
                )}

                {/* Error Banner */}
                {sub.error_message && (
                  <div className="p-2.5 rounded-xl bg-red-500/10 border border-red-500/20 text-red-400 text-xs flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                      <span className="font-mono text-[11px] truncate select-text">{sub.error_message}</span>
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        navigator.clipboard.writeText(sub.error_message || '');
                      }}
                      className="px-2 py-0.5 rounded bg-red-500/20 hover:bg-red-500/30 text-red-300 text-[10px] shrink-0 border border-red-500/30 transition-colors"
                      title="拷贝错误信息"
                    >
                      拷贝
                    </button>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
