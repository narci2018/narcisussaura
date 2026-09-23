import React, { useState, useEffect } from 'react';
import { Shield, RefreshCw, Activity, CheckCircle2, Signal, ArrowUpRight, Search, Globe2, AlertCircle, X, Copy, Check, Square } from 'lucide-react';
import { api } from '../../../services/api';
import { useAppStore } from '../../../stores/appStore';
import { UnifiedNode } from '../../../types';
import { RelayBar } from './RelayBar';

import { matchNodeKeywords } from '../../../components/SimpleMode/countries';

export const VPNGateView: React.FC = () => {
  const { status, connectedNode, connect, disconnect, testLatency, testingLatencyIds, refreshNodes, nodes, errorMessage, setErrorMessage } = useAppStore();
  const storeNodes = React.useMemo(() => nodes.filter((n) => n.group === 'VPNGate'), [nodes]);
  const [gateNodes, setGateNodes] = useState<UnifiedNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [copiedError, setCopiedError] = useState(false);

  const displayNodes = gateNodes.length > 0 ? gateNodes : storeNodes;

  const handleCopyError = () => {
    if (!errorMessage) return;
    navigator.clipboard.writeText(errorMessage);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  const loadVPNGate = async () => {
    setLoading(true);
    try {
      const fetched = await api.fetchVPNGateNodes();
      setGateNodes(fetched);
      await refreshNodes();
    } catch (e) {
      console.error('Failed to load VPNGate nodes:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadVPNGate();
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
            <div className="w-7 h-7 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
              <Shield className="w-4 h-4" />
            </div>
            <h1 className="text-lg font-bold tracking-tight text-gray-100">VPNGate Public Relay Network</h1>
            <span className="px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
              SoftEther VPN
            </span>
          </div>
          <p className="text-xs text-gray-400 mt-1">
            Global volunteer proxy relays operated by University of Tsukuba SoftEther Project to bypass strict censorship.
          </p>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={handleTestAllPing}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-[#171a26] hover:bg-[#202536] border border-[#2b3147] rounded-xl text-xs font-medium text-gray-300 transition-colors"
          >
            <Activity className="w-3.5 h-3.5 text-blue-400" />
            <span>Test Latency</span>
          </button>
          <button
            onClick={loadVPNGate}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white rounded-xl text-xs font-medium transition-all shadow-sm shadow-blue-600/30"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>{loading ? 'Fetching...' : 'Sync VPNGate'}</span>
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
            placeholder="Filter by country, server IP, region..."
            className="w-full bg-[#131620] border border-[#222738] rounded-xl pl-9 pr-3 py-1.5 text-xs text-gray-200 placeholder-gray-500 focus:outline-none focus:border-emerald-500/50"
          />
        </div>
        <div className="text-xs text-gray-500 font-mono">
          {filtered.length} relays available
        </div>
      </div>

      {/* Relay Proxy Toolbar */}
      <RelayBar description="国内直连 VPNGate 志愿者节点易被 GFW 封锁。开启链式中转将通过您的翻墙节点中继访问，保障 100% 成功建联。" />

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

      {/* Relays List */}
      <div className="flex-1 overflow-y-auto p-6">
        {filtered.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-gray-500">
            <Globe2 className="w-10 h-10 mb-2 opacity-30 text-emerald-400" />
            <p className="text-sm">No VPNGate relays found.</p>
            <button
              onClick={loadVPNGate}
              className="mt-3 text-xs text-blue-400 hover:underline flex items-center gap-1"
            >
              <RefreshCw className="w-3 h-3" /> Click to re-sync
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
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
                      : 'bg-[#11141e] border-[#1d2232] hover:border-[#2e3650] hover:bg-[#141824]'
                  }`}
                >
                  <div>
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="flex items-center gap-2">
                        <div className="w-8 h-8 rounded-xl bg-[#1b2030] flex items-center justify-center text-xs font-bold text-emerald-400">
                          {node.country_code}
                        </div>
                        <div>
                          <div className="text-xs font-semibold text-gray-100 flex items-center gap-1.5">
                            <span>{node.country_name || 'VPNGate Relay'}</span>
                            {isConnected && (
                              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                            )}
                          </div>
                          <div className="text-[11px] text-gray-500 font-mono">
                            {node.address}:{node.port}
                          </div>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 text-[10px] font-mono uppercase tracking-wider rounded bg-emerald-500/10 text-emerald-300 border border-emerald-500/20">
                        SoftEther
                      </span>
                    </div>

                    <div className="flex flex-wrap items-center gap-1.5 my-3">
                      <span className="px-1.5 py-0.5 bg-[#171b28] text-gray-400 rounded text-[10px] border border-[#23293d]">
                        Public Relay
                      </span>
                      {node.speed_bps && node.speed_bps > 0 && (
                        <span className="px-1.5 py-0.5 bg-[#171b28] text-emerald-400 rounded text-[10px] border border-[#23293d] font-mono">
                          {Math.round(node.speed_bps / (1024 * 1024))} Mbps
                        </span>
                      )}
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
        )}
      </div>
    </div>
  );
};
