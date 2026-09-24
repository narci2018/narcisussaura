import React, { useState, useEffect } from 'react';
import { Shield, RefreshCw, CheckCircle2, Signal, ArrowUpRight, Search, Globe2, Square } from 'lucide-react';
import { api } from '../../../services/api';
import { useAppStore } from '../../../stores/appStore';
import { RelayBar } from './RelayBar';
import { LivenessBadge, LivenessProgressTag, byLiveness } from './liveness';

import { matchNodeKeywords } from '../../../components/SimpleMode/countries';

export const VPNGateView: React.FC = () => {
  const { status, connectedNode, connect, disconnect, nodes, livenessProgress } = useAppStore();
  // 只用 store 里的节点:后台每测完一批都会 refreshNodes,本地再存一份快照
  // 就会把"未测"永远留在卡片上。
  const gateNodes = React.useMemo(() => nodes.filter((n) => n.group === 'VPNGate'), [nodes]);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');

  const loadVPNGate = async () => {
    setLoading(true);
    try {
      // 重新采集会顺带触发一轮真连接测活(后端 spawn_liveness)
      await api.fetchVPNGateNodes();
    } catch (e) {
      console.error('Failed to load VPNGate nodes:', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadVPNGate();
  }, []);

  const filtered = gateNodes.filter((n) => matchNodeKeywords(n, search)).sort(byLiveness);

  return (
    <div className="flex-1 flex flex-col h-full bg-[#08090d] text-gray-100 overflow-hidden">
      {/* Top Header */}
      <div className="px-4 py-3.5 border-b border-[#1b1f2e] bg-[#0d0f17]/80 flex flex-col justify-between gap-4">
        <div>
          <div className="flex items-center gap-2.5">
            <div className="w-7 h-7 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400">
              <Shield className="w-4 h-4" />
            </div>
            <h1 className="text-lg font-bold tracking-tight text-gray-100">VPNGate Public Relay Network</h1>
            <span className="px-2 py-0.5 text-[12px] font-semibold uppercase tracking-wider rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">
              SoftEther VPN
            </span>
          </div>
          <p className="text-[13px] text-gray-400 mt-1">
            Global volunteer proxy relays operated by University of Tsukuba SoftEther Project to bypass strict censorship.
          </p>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            onClick={loadVPNGate}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 active:bg-blue-500 disabled:opacity-50 text-white rounded-xl text-[13px] font-medium transition-all shadow-sm shadow-blue-600/30"
            title="重新采集多来源清单,并排队一轮真连接测活"
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
            className="w-full bg-[#131620] border border-[#222738] rounded-xl pl-9 pr-3 py-1.5 text-[13px] text-gray-200 placeholder-gray-500 focus:outline-none focus:border-emerald-500/50"
          />
        </div>
        <div className="flex flex-col items-end gap-1 shrink-0">
          <LivenessProgressTag progress={livenessProgress['VPNGate']} />
          <div className="text-[13px] text-gray-500 font-mono">
            {filtered.length} relays available
          </div>
        </div>
      </div>

      {/* Relay Proxy Toolbar */}
      <RelayBar description="国内直连 VPNGate 志愿者节点易被封锁。开启链式中转将通过您的翻墙节点中继访问，保障建联成功率。" />

      {/* Relays List */}
      <div className="flex-1 overflow-y-auto p-6">
        {filtered.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center text-gray-500">
            <Globe2 className="w-10 h-10 mb-2 opacity-30 text-emerald-400" />
            <p className="text-sm">No VPNGate relays found.</p>
            <button
              onClick={loadVPNGate}
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
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="flex items-center gap-2">
                        <div className="w-8 h-8 rounded-xl bg-[#1b2030] flex items-center justify-center text-[13px] font-bold text-emerald-400">
                          {node.country_code}
                        </div>
                        <div>
                          <div className="text-[13px] font-semibold text-gray-100 flex items-center gap-1.5">
                            <span>{node.country_name || 'VPNGate Relay'}</span>
                            {isConnected && (
                              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                            )}
                          </div>
                          <div className="text-[12px] text-gray-500 font-mono">
                            {node.address}:{node.port}
                          </div>
                        </div>
                      </div>

                      <span className="px-2 py-0.5 text-[12px] font-mono uppercase tracking-wider rounded bg-emerald-500/10 text-emerald-300 border border-emerald-500/20">
                        SoftEther
                      </span>
                    </div>

                    <div className="flex flex-wrap items-center gap-1.5 my-3">
                      <LivenessBadge status={node.status} />
                      <span className="px-1.5 py-0.5 bg-[#171b28] text-gray-400 rounded text-[12px] border border-[#23293d]">
                        Public Relay
                      </span>
                      {node.speed_bps && node.speed_bps > 0 && (
                        <span className="px-1.5 py-0.5 bg-[#171b28] text-emerald-400 rounded text-[12px] border border-[#23293d] font-mono">
                          {Math.round(node.speed_bps / (1024 * 1024))} Mbps
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="pt-3 border-t border-[#1c2133] flex items-center justify-between gap-3">
                    <div
                      className="flex items-center gap-1.5 text-[12px] font-mono text-gray-400"
                      title="后台经中转真连接测得的往返延迟"
                    >
                      <Signal className={`w-3.5 h-3.5 ${node.latency_ms && node.latency_ms > 0 ? 'text-emerald-400' : 'text-gray-500'}`} />
                      <span>{node.latency_ms && node.latency_ms > 0 ? `${node.latency_ms}ms` : '未测'}</span>
                    </div>

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
