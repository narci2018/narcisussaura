import React, { useEffect, useState, useMemo } from 'react';
import {
  Power,
  ArrowDown,
  ArrowUp,
  Clock,
  Settings2,
  AlertTriangle,
  WifiOff,
} from 'lucide-react';
import { useAppStore } from '../../stores/appStore';
import { CountryNodeSelector, getBestNode } from './CountryNodeSelector';
import { ExpertModeGate } from './ExpertModeGate';

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec <= 0) return '0 B/s';
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

interface SimpleDashboardProps {
  onSwitchToExpert: () => void;
}

export const SimpleDashboard: React.FC<SimpleDashboardProps> = ({ onSwitchToExpert }) => {
  const {
    status,
    connectedNode,
    traffic,
    nodes,
    connect,
    disconnect,
    errorMessage,
    setErrorMessage,
  } = useAppStore();

  // 'smart' means auto-pick best node; otherwise it's a specific node id
  const [selectedValue, setSelectedValue] = useState<string>('smart');
  const [showExpertGate, setShowExpertGate] = useState(false);

  const isConnected = status === 'connected';
  const isConnecting = status === 'connecting' || status === 'disconnecting';

  // Compute which node to actually connect
  const globalBest = useMemo(() => getBestNode(nodes), [nodes]);

  const resolvedNodeId = useMemo(() => {
    if (selectedValue === 'smart') return globalBest?.id ?? null;
    return selectedValue;
  }, [selectedValue, globalBest]);

  // Auto-dismiss error after 8 seconds
  useEffect(() => {
    if (errorMessage) {
      const t = setTimeout(() => setErrorMessage(null), 8000);
      return () => clearTimeout(t);
    }
  }, [errorMessage, setErrorMessage]);

  const handleToggle = () => {
    if (isConnected) {
      disconnect();
    } else {
      if (!resolvedNodeId && nodes.length === 0) return;
      connect(resolvedNodeId ?? undefined);
    }
  };

  const noNodes = nodes.length === 0 && !isConnected;
  const smartNoNodes =
    selectedValue === 'smart' && !globalBest && nodes.length > 0;

  // Determine node name shown under button
  const displayNodeName = isConnected
    ? connectedNode?.name
    : selectedValue === 'smart'
    ? globalBest?.name
    : nodes.find((n) => n.id === selectedValue)?.name;

  const displayCountry = isConnected
    ? connectedNode?.country_name
    : selectedValue === 'smart'
    ? globalBest?.country_name
    : nodes.find((n) => n.id === selectedValue)?.country_name;

  return (
    <div className="flex flex-col h-full w-full bg-[#090a0f] text-gray-100 overflow-hidden select-none relative">
      {/* Top bar */}
      <div className="flex items-center justify-between px-6 pt-4 pb-2">
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-lg bg-indigo-500/20 border border-indigo-500/30 flex items-center justify-center">
            <span className="text-indigo-400 text-xs font-bold">NA</span>
          </div>
          <span className="text-sm font-semibold text-gray-300">NarcissusAura</span>
        </div>
        <button
          onClick={() => setShowExpertGate(true)}
          className="flex items-center gap-1.5 text-xs text-gray-500 hover:text-indigo-400 border border-[#212637] hover:border-indigo-500/40 rounded-xl px-3 py-1.5 transition-all"
          title="切换到专家模式"
        >
          <Settings2 className="w-3.5 h-3.5" />
          <span>专家模式</span>
        </button>
      </div>

      {/* Error banner */}
      {errorMessage && (
        <div className="mx-4 mb-2 p-3 rounded-2xl bg-[#1c1216] border border-red-500/30 text-red-300 text-xs flex items-start gap-2 animate-in fade-in slide-in-from-top-2 duration-200">
          <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5 text-red-400" />
          <div className="flex-1 min-w-0">
            <p className="font-semibold text-red-300 mb-0.5">连接失败</p>
            <p className="font-mono text-[11px] leading-relaxed break-all text-red-200">{errorMessage}</p>
          </div>
          <button onClick={() => setErrorMessage(null)} className="shrink-0 text-gray-500 hover:text-gray-300">
            ✕
          </button>
        </div>
      )}

      {/* No-nodes warning */}
      {smartNoNodes && (
        <div className="mx-4 mb-2 p-3 rounded-2xl bg-amber-500/5 border border-amber-500/20 text-amber-400 text-xs flex items-center gap-2">
          <WifiOff className="w-4 h-4 shrink-0" />
          <span>无可用节点，错峰上网</span>
        </div>
      )}

      {/* Main content - vertically centered */}
      <div className="flex-1 flex flex-col items-center justify-center px-6 pb-4">
        {/* Connection glow + button */}
        <div className="relative flex items-center justify-center p-8 mb-2">
          {/* Ambient glow */}
          <div
            className={`absolute w-64 h-64 rounded-full blur-3xl transition-all duration-700 pointer-events-none ${
              isConnected
                ? 'bg-emerald-500/20'
                : isConnecting
                ? 'bg-blue-500/20 animate-pulse'
                : 'bg-indigo-500/5'
            }`}
          />

          {/* Outer ring */}
          <div
            className={`w-52 h-52 rounded-full border flex items-center justify-center transition-all duration-500 ${
              isConnected
                ? 'border-emerald-500/40 bg-emerald-500/5 shadow-[0_0_40px_rgba(16,185,129,0.18)]'
                : isConnecting
                ? 'border-blue-500/40 bg-blue-500/5 animate-spin'
                : 'border-[#1e2437] bg-[#0d1018]'
            }`}
          >
            {/* Center button */}
            <button
              onClick={handleToggle}
              disabled={isConnecting || (noNodes && !isConnected)}
              className={`w-40 h-40 rounded-full flex flex-col items-center justify-center gap-2 transition-all active:scale-95 shadow-xl disabled:opacity-50 disabled:cursor-not-allowed ${
                isConnected
                  ? 'bg-gradient-to-br from-emerald-500 to-teal-600 text-white shadow-emerald-500/25'
                  : isConnecting
                  ? 'bg-[#181c28] text-blue-400 cursor-wait'
                  : 'bg-gradient-to-br from-[#1b1f2d] to-[#121520] hover:from-[#212638] hover:to-[#171b29] text-gray-200 border border-[#2b3145]'
              }`}
            >
              <Power
                className={`w-10 h-10 transition-transform duration-300 ${
                  isConnected ? 'text-white' : 'text-gray-400'
                }`}
              />
              <span className="text-sm font-bold uppercase tracking-wider">
                {isConnected ? '断开' : isConnecting ? '连接中' : '连接'}
              </span>
            </button>
          </div>
        </div>

        {/* Status text */}
        <div className="text-center mb-6">
          <div className="text-lg font-bold flex items-center justify-center gap-2 mb-1">
            {isConnected ? (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 shadow-sm shadow-emerald-400" />
                <span className="text-emerald-300">已连接</span>
              </>
            ) : isConnecting ? (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-blue-400 animate-ping" />
                <span className="text-blue-300">正在连接...</span>
              </>
            ) : (
              <>
                <span className="w-2.5 h-2.5 rounded-full bg-gray-600" />
                <span className="text-gray-400">未连接</span>
              </>
            )}
          </div>
          {(displayCountry || displayNodeName) && (
            <p className="text-xs text-gray-500">
              {displayCountry && <span>{displayCountry}</span>}
              {displayCountry && displayNodeName && <span> · </span>}
              {displayNodeName && <span className="truncate">{displayNodeName}</span>}
            </p>
          )}
        </div>

        {/* Traffic stats — shown when connected */}
        {isConnected && (
          <div className="w-full flex items-center justify-around bg-[#0d1018] border border-[#1e2437] rounded-2xl px-4 py-3 mb-6">
            <div className="flex flex-col items-center gap-1">
              <div className="flex items-center gap-1 text-[11px] text-gray-500">
                <ArrowDown className="w-3.5 h-3.5 text-emerald-400" />
                <span>下载</span>
              </div>
              <span className="text-sm font-bold font-mono text-gray-100">
                {formatSpeed(traffic.download_speed)}
              </span>
            </div>
            <div className="w-px h-8 bg-[#1e2437]" />
            <div className="flex flex-col items-center gap-1">
              <div className="flex items-center gap-1 text-[11px] text-gray-500">
                <ArrowUp className="w-3.5 h-3.5 text-blue-400" />
                <span>上传</span>
              </div>
              <span className="text-sm font-bold font-mono text-gray-100">
                {formatSpeed(traffic.upload_speed)}
              </span>
            </div>
            <div className="w-px h-8 bg-[#1e2437]" />
            <div className="flex flex-col items-center gap-1">
              <div className="flex items-center gap-1 text-[11px] text-gray-500">
                <Clock className="w-3.5 h-3.5 text-indigo-400" />
                <span>时长</span>
              </div>
              <span className="text-sm font-bold font-mono text-gray-100">
                {formatDuration(traffic.uptime_seconds)}
              </span>
            </div>
          </div>
        )}

        {/* Node selector */}
        <div className="w-full">
          <p className="text-xs text-gray-600 mb-2 px-1">选择节点</p>
          <CountryNodeSelector
            nodes={nodes}
            value={selectedValue}
            onChange={setSelectedValue}
          />
        </div>
      </div>

      {/* Expert mode gate */}
      {showExpertGate && (
        <ExpertModeGate
          onConfirm={() => {
            setShowExpertGate(false);
            onSwitchToExpert();
          }}
          onCancel={() => setShowExpertGate(false)}
        />
      )}
    </div>
  );
};
