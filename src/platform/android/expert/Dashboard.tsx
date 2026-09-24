import React from 'react';
import {
  Power,
  Globe,
  ArrowDown,
  ArrowUp,
  Clock,
  ChevronRight,
  Square,
} from 'lucide-react';
import { useAppStore } from '../../../stores/appStore';
import { QuoteBar } from './QuoteBar';

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`;
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
  return `${(bytesPerSec / (1024 * 1024)).toFixed(2)} MB/s`;
}

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

export const Dashboard: React.FC = () => {
  const {
    status,
    connectedNode,
    traffic,
    nodes,
    selectedNodeId,
    connect,
    disconnect,
    setActiveTab,
    setErrorMessage,
  } = useAppStore();

  const selectedNode = connectedNode || nodes.find((n) => n.id === selectedNodeId) || nodes[0] || null;

  const isConnected = status === 'connected';
  const isConnecting = status === 'connecting';

  const handleToggle = () => {
    if (isConnected || isConnecting) {
      disconnect();
    } else {
      if (nodes.length === 0) {
        setErrorMessage("节点库为空，正在后台自动更新 Default 订阅源并测速，请耐心等待1~2分钟...");
        useAppStore.getState().autoRefreshDefault().catch(console.error);
        return;
      }
      connect();
    }
  };

  return (
    <div className="flex-1 overflow-y-auto px-4 py-4 flex flex-col items-center justify-between max-w-4xl mx-auto w-full">
      {/* Main Connection Circle Area */}
      <div className="my-auto flex flex-col items-center">
        {/* Glow button wrapper */}
        <div className="relative flex items-center justify-center p-8">
          {/* Background Ambient Glow */}
          <div
            className={`absolute w-56 h-56 rounded-full blur-3xl transition-all duration-700 pointer-events-none ${
              isConnected
                ? 'bg-emerald-500/20'
                : isConnecting
                ? 'bg-blue-500/25 animate-pulse'
                : 'bg-indigo-500/5'
            }`}
          />

          {/* Outer Pulsing Ring */}
          <div
            className={`w-48 h-48 rounded-full border flex items-center justify-center transition-all duration-500 ${
              isConnected
                ? 'border-emerald-500/40 bg-emerald-500/5 shadow-[0_0_30px_rgba(16,185,129,0.2)]'
                : isConnecting
                ? 'border-blue-500/40 bg-blue-500/5 animate-spin'
                : 'border-[#222736] bg-[#10131c]'
            }`}
          >
            {/* Center Interactive Button */}
            <button
              onClick={handleToggle}
              className={`w-36 h-36 rounded-full flex flex-col items-center justify-center gap-1.5 transition-all active:scale-95 shadow-xl ${
                isConnected
                  ? 'bg-gradient-to-br from-emerald-500 to-teal-600 text-white shadow-emerald-500/25'
                  : isConnecting
                  ? 'bg-gradient-to-br from-[#271018] to-[#170a11] active:from-red-950/90 active:to-red-900/70 text-red-300 border border-red-500/40 shadow-red-500/15 cursor-pointer group'
                  : 'bg-gradient-to-br from-[#1b1f2d] to-[#121520] active:from-[#212638] active:to-[#171b29] text-gray-200 border border-[#2b3145]'
              }`}
              title={isConnecting ? "点击终止连接" : undefined}
            >
              {isConnecting ? (
                <Square className="w-8 h-8 text-red-400 fill-red-400 animate-pulse group-active:scale-110 transition-transform" />
              ) : (
                <Power
                  className={`w-9 h-9 transition-transform duration-300 ${
                    isConnected ? 'rotate-0' : 'text-gray-400'
                  }`}
                />
              )}
              <span className="text-[13px] font-semibold uppercase tracking-wider">
                {isConnected ? 'Disconnect' : isConnecting ? '终止连接' : 'Connect'}
              </span>
            </button>
          </div>
        </div>

        {/* Status Text & Location */}
        <div className="text-center mt-3">
          <div className="text-xl font-bold text-gray-100 flex items-center justify-center gap-2">
            {isConnected ? (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 shadow-sm shadow-emerald-400" />
                <span>Connected</span>
              </>
            ) : isConnecting ? (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-blue-400 animate-ping" />
                <span>Establishing Secure Tunnel...</span>
              </>
            ) : (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-gray-500" />
                <span>Not Connected</span>
              </>
            )}
          </div>
          {isConnecting && (
            <div className="mt-2.5 flex justify-center">
              <button
                onClick={() => disconnect()}
                className="px-4 py-1.5 rounded-full bg-red-500/15 active:bg-red-500/30 border border-red-500/40 text-red-300 active:text-red-100 text-[13px] font-semibold flex items-center gap-1.5 transition-all shadow-sm shadow-red-500/10 active:scale-95"
              >
                <Square className="w-3 h-3 fill-red-400 text-red-400" />
                <span>终止当前连接</span>
              </button>
            </div>
          )}
          <p className="text-[13px] text-gray-400 mt-1">
            {isConnected
              ? `${connectedNode?.country_name || 'Global Proxy'} · ${connectedNode?.name || ''}`
              : selectedNode
              ? `Ready to connect to ${selectedNode.name}`
              : 'Select a server to start'}
          </p>
        </div>

        {/* Inspirational Quote Card */}
        <QuoteBar />
      </div>

      {/* Selected Server Card & Quick Change */}
      <div className="w-full grid grid-cols-1 gap-4 mt-6">
        {/* Server Card */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-4 flex items-center justify-between active:border-[#2f364d] transition-all">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-[#1a1e2d] border border-[#282f45] flex items-center justify-center text-blue-400">
              <Globe className="w-5 h-5" />
            </div>
            <div>
              <div className="text-[13px] text-gray-400">Current Server</div>
              <div className="text-sm font-semibold text-gray-200 mt-0.5 truncate max-w-[200px]">
                {selectedNode?.name || 'No server selected'}
              </div>
              <div className="flex items-center gap-2 mt-1">
                <span className="px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 text-[12px] uppercase font-mono font-bold">
                  {selectedNode?.protocol || 'VLESS'}
                </span>
                {selectedNode?.latency_ms !== null && selectedNode?.latency_ms !== undefined && (
                  <span className="text-[12px] font-mono text-emerald-400">
                    {selectedNode.latency_ms} ms
                  </span>
                )}
              </div>
            </div>
          </div>

          <button
            onClick={() => setActiveTab('servers')}
            className="flex items-center gap-1 text-[13px] text-blue-400 active:text-blue-300 font-medium px-3 py-1.5 rounded-lg active:bg-blue-500/10 transition-colors"
          >
            <span>Change</span>
            <ChevronRight className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Live Telemetry / Traffic Metrics */}
        <div className="bg-[#12151f] border border-[#212637] rounded-2xl p-4 flex items-center justify-between">
          <div className="flex items-center gap-6 w-full justify-around text-center">
            <div>
              <div className="flex items-center justify-center gap-1 text-[12px] text-gray-400 mb-1">
                <ArrowDown className="w-3.5 h-3.5 text-emerald-400" />
                <span>Download</span>
              </div>
              <div className="text-sm font-bold font-mono text-gray-100">
                {formatSpeed(traffic.download_speed)}
              </div>
              <div className="text-[12px] text-gray-500 font-mono mt-0.5">
                {formatBytes(traffic.download_bytes)}
              </div>
            </div>

            <div className="h-8 w-px bg-[#202535]" />

            <div>
              <div className="flex items-center justify-center gap-1 text-[12px] text-gray-400 mb-1">
                <ArrowUp className="w-3.5 h-3.5 text-blue-400" />
                <span>Upload</span>
              </div>
              <div className="text-sm font-bold font-mono text-gray-100">
                {formatSpeed(traffic.upload_speed)}
              </div>
              <div className="text-[12px] text-gray-500 font-mono mt-0.5">
                {formatBytes(traffic.upload_bytes)}
              </div>
            </div>

            <div className="h-8 w-px bg-[#202535]" />

            <div>
              <div className="flex items-center justify-center gap-1 text-[12px] text-gray-400 mb-1">
                <Clock className="w-3.5 h-3.5 text-indigo-400" />
                <span>Uptime</span>
              </div>
              <div className="text-sm font-bold font-mono text-gray-100">
                {formatDuration(traffic.uptime_seconds)}
              </div>
              <div className="text-[12px] text-gray-500 mt-0.5">
                {isConnected ? 'Active' : 'Offline'}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
