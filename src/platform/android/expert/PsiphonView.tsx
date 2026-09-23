import React, { useState, useEffect } from 'react';
import { Globe, RefreshCw, Activity, CheckCircle2, Signal, ArrowUpRight, Search, Radio, AlertCircle, X, Copy, Check, Server, Square } from 'lucide-react';
import { api } from '../../../services/api';
import { useAppStore } from '../../../stores/appStore';
import { UnifiedNode } from '../../../types';
import { RelayBar } from './RelayBar';
import { matchNodeKeywords } from '../../../components/SimpleMode/countries';

export const PsiphonView: React.FC = () => {
  const { status, connectedNode, connect, disconnect, testLatency, testingLatencyIds, refreshNodes, nodes, errorMessage, setErrorMessage } = useAppStore();
  const storeNodes = React.useMemo(() => nodes.filter((n) => n.group === 'Psiphon'), [nodes]);
  const [psiphonNodes, setPsiphonNodes] = useState<UnifiedNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [copiedError, setCopiedError] = useState(false);

  const displayNodes = psiphonNodes.length > 0 ? psiphonNodes : storeNodes;

  const totalAvailableNodes = React.useMemo(() => {
    return displayNodes.reduce((acc, n) => {
      const count = Number(n.config?.node_count || n.config?.available_nodes) || 0;
      return acc + count;
    }, 0);
  }, [displayNodes]);

  const loadPsiphon = async () => {
    setLoading(true);
    try {
      const fetched = await api.fetchPsiphonNodes();
      setPsiphonNodes(fetched);
      await refreshNodes();
    } catch (e) {
      console.error('Failed to load Psiphon nodes:', e);
    } finally {
      setLoading(false);
    }
  };

  const handleCopyError = () => {
    if (!errorMessage) return;
    navigator.clipboard.writeText(errorMessage);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  useEffect(() => {
    loadPsiphon();
  }, []);

  const handleTestAllPing = async () => {
    for (const n of displayNodes) {
      testLatency(n.id);
    }
  };

  const filtered = displayNodes.filter((n) => matchNodeKeywords(n, search));

  return (
    <div className="flex-1 flex flex-col h-full bg-[#08090d] text-gray-100 overflow-hidden">
      {/* Top Header */}
      <div className="p-6 border-b border-[#1b1f2e] bg-[#0d0f17]/80 flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400">
              <Globe className="w-4 h-4" />
            </div>
            <h1 className="text-lg font-bold tracking-tight text-gray-100">Psiphon Obfuscated Tunnel Fleet</h1>
            <span className="px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">
              psiphon-tunnel-core
            </span>
          </div>
          <p className="text-xs text-gray-400 mt-1">
            动态获取官方经过加密签名的出境节点，多协议自动混淆与智能穿透。已剔除无节点区域，当前仅显示真实可用国家。
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleTestAllPing}
            className="px-3 py-1.5 bg-[#171b28] hover:bg-[#202638] text-gray-300 border border-[#262c3e] rounded-xl text-xs font-medium transition-all flex items-center gap-1.5"
            title="测试所有节点延迟"
          >
            <Activity className="w-3.5 h-3.5 text-indigo-400" />
            <span>测速全部</span>
          </button>
          <button
            onClick={loadPsiphon}
            disabled={loading}
            className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-medium transition-all shadow-sm shadow-indigo-600/20 flex items-center gap-1.5 disabled:opacity-50"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>{loading ? '同步中...' : '刷新出境池'}</span>
          </button>
        </div>
      </div>

      {/* Filter / Search Bar */}
      <div className="px-6 py-3 bg-[#0a0c13] border-b border-[#171a26] flex items-center justify-between gap-4">
        <div className="relative flex-1 max-w-md">
          <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索 Psiphon 出境地区或国家代码 (如 JP, US, 英国)..."
            className="w-full bg-[#131620] border border-[#222738] rounded-xl pl-9 pr-3 py-1.5 text-xs text-gray-200 placeholder-gray-500 focus:outline-none focus:border-indigo-500/50"
          />
        </div>
        <div className="flex items-center gap-3 text-xs font-mono text-gray-400">
          <span className="flex items-center gap-1.5 bg-[#141724] px-2.5 py-1 rounded-lg border border-[#22283a]">
            <Globe className="w-3.5 h-3.5 text-indigo-400" />
            <strong className="text-gray-200">{filtered.length}</strong> 个有效出境国家
          </span>
          <span className="flex items-center gap-1.5 bg-[#141724] px-2.5 py-1 rounded-lg border border-[#22283a]">
            <Server className="w-3.5 h-3.5 text-emerald-400" />
            共计 <strong className="text-emerald-300">{totalAvailableNodes}</strong> 个可用节点
          </span>
        </div>
      </div>

      {/* Relay Proxy Toolbar */}
      <RelayBar description="Psiphon 部分海外出境地区入口受到 GFW 阻断。开启中转加速将借道翻墙节点，保障日本 (JP)、美国 (US) 等全部可用地区秒速建联。" />

      {/* Connection Failure Notice */}
      {errorMessage && (
        <div className="mx-6 mt-4 p-3.5 bg-red-950/80 border border-red-500/60 rounded-xl flex items-start justify-between gap-3 text-red-200 backdrop-blur-md">
          <div className="flex items-start gap-2.5 flex-1 min-w-0">
            <AlertCircle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
            <div className="text-xs font-mono whitespace-pre-wrap leading-relaxed select-text break-words">
              {errorMessage}
            </div>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              onClick={handleCopyError}
              className="flex items-center gap-1 text-[11px] px-2 py-1 rounded-lg bg-red-900/70 hover:bg-red-800 text-red-200 border border-red-500/50 transition-colors shadow-sm"
              title="拷贝错误信息到剪贴板"
            >
              {copiedError ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
              <span>{copiedError ? '已拷贝' : '拷贝错误'}</span>
            </button>
            <button
              onClick={() => setErrorMessage(null)}
              className="text-red-400 hover:text-white p-1 rounded-lg hover:bg-white/10 transition-colors"
              title="关闭"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Regions Grid */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map((node) => {
            const isConnected = connectedNode?.id === node.id && status === 'connected';
            const isConnecting = connectedNode?.id === node.id && status === 'connecting';
            const isPinging = testingLatencyIds.includes(node.id);
            const nodeCount = Number(node.config?.node_count || node.config?.available_nodes) || 0;

            return (
              <div
                key={node.id}
                className={`relative flex flex-col justify-between p-4 rounded-2xl border transition-all ${
                  isConnected
                    ? 'bg-gradient-to-br from-emerald-950/30 to-[#0e1713] border-emerald-500/40 shadow-lg shadow-emerald-950/20'
                    : 'bg-[#11141e] border-[#1d2232] hover:border-[#2e3650] hover:bg-[#141824]'
                }`}
              >
                <div>
                  <div className="flex items-start justify-between gap-2 mb-2">
                    <div className="flex items-center gap-2">
                      <div className="w-8 h-8 rounded-xl bg-[#1b2030] flex items-center justify-center text-xs font-bold text-indigo-400">
                        {node.country_code}
                      </div>
                      <div>
                        <div className="text-xs font-semibold text-gray-100 flex items-center gap-1.5">
                          <span>{node.country_name}</span>
                          {isConnected && (
                            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                          )}
                        </div>
                        <div className="text-[11px] text-gray-500 font-mono flex items-center gap-2 mt-0.5">
                          <span>{node.city}</span>
                        </div>
                      </div>
                    </div>

                    <span className="px-2 py-0.5 text-[10px] font-mono uppercase tracking-wider rounded bg-indigo-500/10 text-indigo-300 border border-indigo-500/20">
                      OSSH / QUIC
                    </span>
                  </div>

                  <div className="flex flex-wrap items-center gap-1.5 my-3">
                    <span className="px-2 py-0.5 bg-emerald-500/15 text-emerald-300 rounded-lg text-[10px] font-medium border border-emerald-500/25 flex items-center gap-1">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                      {nodeCount > 0 ? `${nodeCount} 个可用节点` : '已就绪'}
                    </span>
                    <span className="px-1.5 py-0.5 bg-[#171b28] text-gray-400 rounded text-[10px] border border-[#23293d] flex items-center gap-1">
                      <Radio className="w-2.5 h-2.5 text-indigo-400" />
                      混淆核心
                    </span>
                    <span className="px-1.5 py-0.5 bg-[#171b28] text-indigo-300 rounded text-[10px] border border-[#23293d]">
                      Multi-Hop
                    </span>
                  </div>
                </div>

                <div className="pt-3 border-t border-[#1c2133] flex items-center justify-between gap-3">
                  <button
                    onClick={() => testLatency(node.id)}
                    disabled={isPinging}
                    className="flex items-center gap-1.5 text-[11px] font-mono text-gray-400 hover:text-gray-200 transition-colors"
                    title="Test latency"
                  >
                    <Signal className={`w-3.5 h-3.5 ${isPinging ? 'animate-pulse text-amber-400' : 'text-gray-500'}`} />
                    <span>
                      {isPinging ? 'Testing...' : node.latency_ms && node.latency_ms > 0 ? `${node.latency_ms}ms` : 'Ping'}
                    </span>
                  </button>

                  {isConnected ? (
                    <button
                      onClick={() => disconnect()}
                      className="px-3 py-1.5 bg-emerald-600/20 hover:bg-red-600/30 text-emerald-400 hover:text-red-400 border border-emerald-500/40 rounded-xl text-xs font-medium transition-all flex items-center gap-1.5"
                    >
                      <CheckCircle2 className="w-3.5 h-3.5" />
                      <span>Connected</span>
                    </button>
                  ) : isConnecting ? (
                    <button
                      onClick={() => disconnect()}
                      className="px-3.5 py-1.5 bg-red-600/25 hover:bg-red-600/40 border border-red-500/40 text-red-300 hover:text-red-100 rounded-xl text-xs font-medium transition-all shadow-sm shadow-red-500/10 flex items-center gap-1.5 active:scale-95"
                      title="点击终止连接"
                    >
                      <Square className="w-3.5 h-3.5 fill-red-400 text-red-400 animate-pulse" />
                      <span>终止</span>
                    </button>
                  ) : (
                    <button
                      onClick={() => connect(node.id)}
                      className="px-3.5 py-1.5 bg-blue-600 hover:bg-blue-500 text-white rounded-xl text-xs font-medium transition-all shadow-sm shadow-blue-600/20 flex items-center gap-1.5 active:scale-95"
                    >
                      <ArrowUpRight className="w-3.5 h-3.5" />
                      <span>Connect</span>
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
