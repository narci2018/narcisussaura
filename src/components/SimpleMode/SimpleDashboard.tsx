import React, { useEffect, useState, useMemo } from 'react';
import {
  Power,
  ArrowDown,
  ArrowUp,
  Clock,
  Settings2,
  AlertTriangle,
  WifiOff,
  Square,
} from 'lucide-react';
import { useAppStore } from '../../stores/appStore';
import { CountryNodeSelector, getBestNode, getCountryCodeForNode, isRegularSubscriptionNode } from './CountryNodeSelector';
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
    tunnelStage,
    errorMessage,
    setErrorMessage,
    authDisplayText,
    connectedChainId,
  } = useAppStore();

  // 'smart' means auto-pick best node; otherwise it's a specific node id (only for regular subscription nodes)
  const [selectedValue, setSelectedValue] = useState<string>(() => {
    if (connectedNode && isRegularSubscriptionNode(connectedNode)) {
      return connectedNode.id;
    }
    return 'smart';
  });
  const [showExpertGate, setShowExpertGate] = useState(false);

  const isConnected = status === 'connected';
  const isConnecting = status === 'connecting' || status === 'disconnecting';

  // Compute regular subscription nodes (strictly exclude all special/advanced proxy modes)
  const regularNodes = useMemo(() => nodes.filter(isRegularSubscriptionNode), [nodes]);
  const globalBest = useMemo(() => getBestNode(regularNodes), [regularNodes]);

  const resolvedNodeId = useMemo(() => {
    if (selectedValue === 'smart') return globalBest?.id ?? null;
    if (selectedValue.startsWith('country:')) {
      const code = selectedValue.substring(8);
      const nodesInCountry = regularNodes.filter(n => getCountryCodeForNode(n) === code);
      return getBestNode(nodesInCountry)?.id ?? null;
    }
    return selectedValue;
  }, [selectedValue, globalBest, regularNodes]);

  // Auto-dismiss error after 8 seconds
  useEffect(() => {
    if (errorMessage) {
      const t = setTimeout(() => setErrorMessage(null), 8000);
      return () => clearTimeout(t);
    }
  }, [errorMessage, setErrorMessage]);

  const handleToggle = () => {
    if (isConnected || isConnecting) {
      disconnect();
    } else {
      if (regularNodes.length === 0) {
        setErrorMessage("节点库为空，正在后台自动更新 Default 订阅源并测速，请耐心等待1~2分钟...");
        useAppStore.getState().autoRefreshDefault().catch(console.error);
        return;
      }
      if (selectedValue === 'smart') {
        const topNodes = [...regularNodes]
          .sort((a, b) => (a.latency_ms || 9999) - (b.latency_ms || 9999))
          .slice(0, 10)
          .map(n => n.id);
        if (topNodes.length > 0) {
          useAppStore.getState().connectSmartGroup(topNodes);
        }
      } else {
        if (!resolvedNodeId) return;
        connect(resolvedNodeId ?? undefined);
      }
    }
  };

  const smartNoNodes =
    selectedValue === 'smart' && !globalBest && regularNodes.length > 0;

  // Determine node name shown under button
  const displayNodeName = isConnected
    ? (connectedChainId === 'smart-group' ? `${connectedNode?.name} (智能漂移)` : connectedNode?.name)
    : selectedValue === 'smart'
    ? globalBest?.name
    : selectedValue.startsWith('country:')
    ? (resolvedNodeId ? nodes.find(n => n.id === resolvedNodeId)?.name : '未知节点')
    : nodes.find((n) => n.id === selectedValue)?.name;

  const displayCountry = isConnected
    ? connectedNode?.country_name
    : selectedValue === 'smart'
    ? globalBest?.country_name
    : selectedValue.startsWith('country:')
    ? (resolvedNodeId ? nodes.find(n => n.id === resolvedNodeId)?.country_name : '未知')
    : nodes.find((n) => n.id === selectedValue)?.country_name;

  return (
    <div className="flex flex-col h-full w-full bg-[#090a0f] text-gray-100 overflow-hidden select-none relative">
      {/* Top bar */}
      <div className="flex items-center justify-between px-6 pt-4 pb-2">
        <div className="flex items-center gap-2">
          <div className="w-7 h-7 rounded-lg bg-indigo-500/20 border border-indigo-500/30 flex items-center justify-center">
            <span className="text-indigo-400 text-xs font-bold">
              {authDisplayText ? authDisplayText.substring(0, 2).toUpperCase() : 'NA'}
            </span>
          </div>
          <span className="text-sm font-semibold text-gray-300">
            {authDisplayText || 'NarcissusAura'}
          </span>
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
              className={`w-40 h-40 rounded-full flex flex-col items-center justify-center gap-2 transition-all active:scale-95 shadow-xl ${
                isConnected
                  ? 'bg-gradient-to-br from-emerald-500 to-teal-600 text-white shadow-emerald-500/25'
                  : isConnecting
                  ? 'bg-gradient-to-br from-[#271018] to-[#170a11] hover:from-red-950/90 hover:to-red-900/70 text-red-300 border border-red-500/40 shadow-red-500/15 cursor-pointer group'
                  : 'bg-gradient-to-br from-[#1b1f2d] to-[#121520] hover:from-[#212638] hover:to-[#171b29] text-gray-200 border border-[#2b3145]'
              }`}
              title={isConnecting ? "点击立即终止连接" : undefined}
            >
              {isConnecting ? (
                <Square className="w-10 h-10 text-red-400 fill-red-400 animate-pulse group-hover:scale-110 transition-transform" />
              ) : (
                <Power
                  className={`w-10 h-10 transition-transform duration-300 ${
                    isConnected ? 'text-white' : 'text-gray-400'
                  }`}
                />
              )}
              <span className="text-sm font-bold uppercase tracking-wider">
                {isConnected ? '断开' : isConnecting ? '终止连接' : '连接'}
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
          {isConnecting && tunnelStage && (
            <p
              className={`text-[11px] leading-relaxed break-all px-4 ${
                tunnelStage.includes('失败') || tunnelStage.includes('拒绝') || tunnelStage.includes('授权')
                  ? 'text-amber-300'
                  : 'text-blue-200/70'
              }`}
            >
              {tunnelStage}
            </p>
          )}
          {(displayCountry || displayNodeName) && (
            <p className="text-xs text-gray-500">
              {displayCountry && <span>{displayCountry}</span>}
              {displayCountry && displayNodeName && <span> · </span>}
              {displayNodeName && <span className="truncate">{displayNodeName}</span>}
            </p>
          )}
          {isConnecting && (
            <div className="mt-3 flex justify-center">
              <button
                onClick={() => disconnect()}
                className="px-4 py-1.5 rounded-full bg-red-500/15 hover:bg-red-500/30 border border-red-500/40 text-red-300 hover:text-red-100 text-xs font-semibold flex items-center gap-1.5 transition-all shadow-sm shadow-red-500/10 active:scale-95"
              >
                <Square className="w-3 h-3 fill-red-400 text-red-400" />
                <span>终止当前连接</span>
              </button>
            </div>
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
