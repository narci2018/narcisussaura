import React, { useState, useEffect } from 'react';
import { Zap, RefreshCw, Activity, CheckCircle2, Signal, ArrowUpRight, Search, AlertCircle, X, Copy, Check, Square } from 'lucide-react';
import { api } from '../../../services/api';
import { useAppStore } from '../../../stores/appStore';
import { UnifiedNode } from '../../../types';
import { RelayBar } from './RelayBar';

import { matchNodeKeywords } from '../../../components/SimpleMode/countries';

export const MegaVView: React.FC = () => {
  const { status, connectedNode, connect, disconnect, testLatency, testingLatencyIds, refreshNodes, nodes, errorMessage, setErrorMessage } = useAppStore();
  const storeNodes = React.useMemo(() => nodes.filter((n) => n.group === 'MegaV'), [nodes]);
  const [megaNodes, setMegaNodes] = useState<UnifiedNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [copiedError, setCopiedError] = useState(false);

  const displayNodes = megaNodes.length > 0 ? megaNodes : storeNodes;

  const handleCopyError = () => {
    if (!errorMessage) return;
    navigator.clipboard.writeText(errorMessage);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  const loadMegaV = async () => {
    setLoading(true);
    try {
      const fetched = await api.fetchMegaVNodes();
      setMegaNodes(fetched);
      await refreshNodes();
    } catch (e) {
      console.error('Failed to load MegaV nodes:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadMegaV();
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
      <div className="px-4 py-3.5 border-b border-[#1b1f2e] bg-[#0d0f17]/80 flex flex-col justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-amber-500/10 border border-amber-500/20 flex items-center justify-center text-amber-400">
              <Zap className="w-4 h-4" />
            </div>
            <h1 className="text-lg font-bold tracking-tight text-gray-100">MegaV High-Speed Fleet</h1>
            <span className="px-2 py-0.5 text-[12px] font-semibold uppercase tracking-wider rounded bg-amber-500/20 text-amber-300 border border-amber-500/30">
              Romaxa55/MegaV
            </span>
          </div>
          <p className="text-[13px] text-gray-400 mt-1">
            Optimized, decentralized VLESS Reality, Trojan, and Shadowsocks nodes curated from the MegaV Public repository.
          </p>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={handleTestAllPing}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#171a26] active:bg-[#202536] border border-[#2b3147] rounded-xl text-[13px] font-medium text-gray-300 transition-colors"
          >
            <Activity className="w-3.5 h-3.5 text-blue-400" />
            <span>Test Latency</span>
          </button>
          <button
            onClick={loadMegaV}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 active:bg-blue-500 disabled:opacity-50 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-blue-600/30"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>{loading ? 'Fetching...' : 'Sync MegaV'}</span>
          </button>
        </div>
      </div>

      {/* Filter and Search Bar */}
      <div className="px-6 py-3 border-b border-[#171a26] bg-[#0c0e14] flex items-center justify-between gap-4">
        <div className="relative flex-1 max-w-md">
          <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search MegaV nodes by country, city, protocol..."
            className="w-full bg-[#131620] border border-[#222738] rounded-xl pl-9 pr-3 py-1.5 text-[13px] text-gray-200 placeholder-gray-500 focus:outline-none focus:border-amber-500/50"
          />
        </div>
        <div className="text-[13px] text-gray-500 font-mono">
          {filtered.length} nodes available
        </div>
      </div>

      {/* Relay Proxy Toolbar */}
      <RelayBar description="MegaV 公开节点入口 IP 及 workers.dev 域名在国内被全面阻断。开启链式中转将通过您的翻墙节点中继访问，保障 100% 成功建联。" />

      {/* Connection Failure Notice */}
      {errorMessage && (
        <div className="mx-6 mt-4 p-3.5 bg-red-950/80 border border-red-500/60 rounded-xl flex items-start justify-between gap-3 text-red-200 backdrop-blur-md">
          <div className="flex items-start gap-2.5 flex-1 min-w-0">
            <AlertCircle className="w-4 h-4 text-red-400 shrink-0 mt-0.5" />
            <div className="text-[13px] font-mono whitespace-pre-wrap leading-relaxed select-text break-words">
              {errorMessage}
            </div>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              onClick={handleCopyError}
              className="flex items-center gap-1 text-[12px] px-2 py-1 rounded-lg bg-red-900/70 active:bg-red-800 text-red-200 border border-red-500/50 transition-colors shadow-sm"
              title="拷贝错误信息到剪贴板"
            >
              {copiedError ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
              <span>{copiedError ? '已拷贝' : '拷贝错误'}</span>
            </button>
            <button
              onClick={() => setErrorMessage(null)}
              className="text-red-400 active:text-white p-1 rounded-lg active:bg-white/10 transition-colors"
              title="关闭"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Nodes Cards Grid */}
      <div className="flex-1 overflow-y-auto p-6">
        {filtered.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-gray-500">
            <Zap className="w-10 h-10 mb-2 opacity-30 text-amber-400" />
            <p className="text-sm">No MegaV nodes found.</p>
            <button
              onClick={loadMegaV}
              className="mt-3 text-[13px] text-blue-400 active:underline flex items-center gap-1"
            >
              <RefreshCw className="w-3 h-3" /> Click to re-sync
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-4">
            {filtered.map((node) => {
              const isConnected = connectedNode?.id === node.id && status === 'connected';
              const isConnecting = connectedNode?.id === node.id && status === 'connecting';
              const isPinging = testingLatencyIds.includes(node.id);

              return (
                <div
                  key={node.id}
                  className={`relative flex flex-col justify-between p-4 rounded-2xl border transition-all ${
                    isConnected
                      ? 'bg-gradient-to-br from-emerald-950/30 to-[#0e1713] border-emerald-500/40 shadow-lg shadow-emerald-950/20'
                      : 'bg-[#11141e] border-[#1d2232] active:border-[#2e3650] active:bg-[#141824]'
                  }`}
                >
                  <div>
                    <div className="flex items-start justify-between gap-2 mb-2.5">
                      <div className="flex items-center gap-2">
                        <div className="w-8 h-8 rounded-xl bg-[#1b2030] flex items-center justify-center text-sm shadow-inner font-bold text-gray-300">
                          {node.country_code || '🌐'}
                        </div>
                        <div>
                          <div className="text-[13px] font-semibold text-gray-100 flex items-center gap-1.5">
                            <span>{node.city || node.country_name || 'MegaV Node'}</span>
                            {isConnected && (
                              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                            )}
                          </div>
                          <div className="text-[12px] text-gray-500 font-mono">
                            {node.address}:{node.port}
                          </div>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 text-[12px] font-mono uppercase tracking-wider rounded bg-purple-500/10 text-purple-300 border border-purple-500/20">
                        {node.protocol}
                      </span>
                    </div>

                    <div className="flex flex-wrap items-center gap-1.5 my-3">
                      {node.tags.map((tag) => (
                        <span
                          key={tag}
                          className="px-1.5 py-0.5 bg-[#171b28] text-gray-400 rounded text-[12px] border border-[#23293d]"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  </div>

                  <div className="pt-3 border-t border-[#1c2133] flex items-center justify-between gap-3">
                    <button
                      onClick={() => testLatency(node.id)}
                      disabled={isPinging}
                      className="flex items-center gap-1.5 text-[12px] font-mono text-gray-400 active:text-gray-200 transition-colors"
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
                        className="px-3 py-1.5 bg-emerald-600/20 active:bg-red-600/30 text-emerald-400 active:text-red-400 border border-emerald-500/40 rounded-xl text-[13px] font-medium transition-all flex items-center gap-1.5"
                      >
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        <span>Connected</span>
                      </button>
                    ) : isConnecting ? (
                      <button
                        onClick={() => disconnect()}
                        className="px-3.5 py-1.5 bg-red-600/25 active:bg-red-600/40 border border-red-500/40 text-red-300 active:text-red-100 rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-red-500/10 flex items-center gap-1.5 active:scale-95"
                        title="点击终止连接"
                      >
                        <Square className="w-3.5 h-3.5 fill-red-400 text-red-400 animate-pulse" />
                        <span>终止</span>
                      </button>
                    ) : (
                      <button
                        onClick={() => connect(node.id)}
                        className="px-3.5 py-1.5 bg-blue-600 active:bg-blue-500 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-blue-600/20 flex items-center gap-1.5 active:scale-95"
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
        )}
      </div>
    </div>
  );
};
