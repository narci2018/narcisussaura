import React, { useState, useEffect } from 'react';
import { Minus, Square, Copy, X, Shield, Smartphone, Sun, Moon } from 'lucide-react';
import { getVersion } from '@tauri-apps/api/app';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from '../services/api';
import { useAppStore } from '../stores/appStore';
// 平台差异一律经 src/platform 分发（规则见仓库根 AGENTS.md）——
// 组件内不允许自行嗅探 UA / 判断平台。
import { currentProfile } from '../platform';
// Build-time version: package.json is tagged together with the release, so it
// is a truthful fallback when the runtime getVersion() IPC is unavailable.
import pkg from '../../package.json';

interface TitleBarProps {
  /** If provided, shows a "小白模式" back button in expert mode */
  onSwitchToSimple?: () => void;
}

// UA is static for the process lifetime; resolve once.
const profile = currentProfile();

export const TitleBar: React.FC<TitleBarProps> = ({ onSwitchToSimple }) => {
  const { status, connectedNode, settings, saveSettings, disconnect } = useAppStore();
  const [isMaximized, setIsMaximized] = useState(false);
  const [appVersion, setAppVersion] = useState('');

  useEffect(() => {
    if (!profile.windowChrome) return;
    api.isWindowMaximized().then(setIsMaximized).catch(() => {});
    // 双击标题栏的最大化是 Tauri 注入脚本自己 invoke internal_toggle_maximize 完成的,
    // 组件拿不到回调,只能跟着窗口尺寸事件回读真实状态(否则按钮图标会停留在旧值)。
    // async 包装:纯浏览器里没有 __TAURI_INTERNALS__,getCurrentWindow() 会同步抛错。
    let stop: (() => void) | undefined;
    const watchResize = async () => {
      stop = await getCurrentWindow().onResized(() => {
        api.isWindowMaximized().then(setIsMaximized).catch(() => {});
      });
    };
    watchResize().catch(() => {});
    return () => stop?.();
  }, []);

  useEffect(() => {
    getVersion().then(v => setAppVersion(`v${v}`)).catch(() => setAppVersion(`v${pkg.version}`));
  }, []);

  const handleMinimize = () => {
    api.minimizeWindow().catch(console.error);
  };

  const handleToggleMaximize = async () => {
    try {
      const max = await api.toggleMaximize();
      setIsMaximized(max);
    } catch (e) {
      console.error(e);
    }
  };

  const handleClose = () => {
    api.closeWindow().catch(console.error);
  };

  const handleThemeToggle = () => {
    const newTheme = settings.theme === 'light' ? 'dark' : 'light';
    saveSettings({ ...settings, theme: newTheme });
  };

  return (
    <div
      {...(profile.windowChrome && { 'data-tauri-drag-region': 'deep' })}
      className="h-10 bg-[#090a0f] border-b border-[#1c1f2b] flex items-center justify-between px-3 select-none z-50 text-xs font-medium text-gray-400 cursor-default"
    >
      {/* Left: Brand & Status pill */}
      <div className="flex items-center gap-2.5 pointer-events-none">
        <div className="w-5 h-5 rounded-md bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-white shadow-sm shadow-blue-500/20">
          <Shield className="w-3.5 h-3.5" />
        </div>
        <span className="font-semibold tracking-wide text-gray-200">Narci'ssus Aura</span>
        <span className="text-[10px] px-1.5 py-0.2 rounded bg-indigo-500/20 text-indigo-300 font-mono border border-indigo-500/30">{appVersion || 'v...'}</span>

        <div className="h-3.5 w-px bg-[#262a3b] mx-1" />

        {status === 'connected' ? (
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-[11px]">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
            <span>Connected: {connectedNode?.name || 'Proxy Active'}</span>
          </div>
        ) : status === 'connecting' ? (
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/20 text-[11px] pointer-events-auto">
            <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-ping" />
            <span>Connecting...</span>
            <button
              onClick={() => disconnect()}
              className="ml-1 px-1.5 py-0.5 rounded bg-red-500/20 hover:bg-red-500/40 text-red-300 hover:text-red-100 text-[10px] font-semibold border border-red-500/30 transition-all flex items-center gap-1 shadow-sm active:scale-95"
              title="终止连接"
            >
              <Square className="w-2.5 h-2.5 fill-red-400 text-red-400" />
              <span>终止</span>
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-gray-800/40 text-gray-400 border border-gray-700/30 text-[11px]">
            <span className="w-1.5 h-1.5 rounded-full bg-gray-500" />
            <span>Disconnected</span>
          </div>
        )}
      </div>

      {/* Right: Mode switch + Window Controls (hidden on mobile) */}
      <div className="flex items-center gap-1 no-drag">
        {onSwitchToSimple && (
          <button
            onClick={onSwitchToSimple}
            className="flex items-center gap-1 mr-1 text-[11px] px-2 py-1 rounded-md hover:bg-indigo-500/20 text-gray-500 hover:text-indigo-300 transition-colors"
            title="切换回小白模式"
          >
            <Smartphone className="w-3 h-3" />
            <span>小白模式</span>
          </button>
        )}
        <button
          onClick={handleThemeToggle}
          className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-gray-800/60 text-gray-400 hover:text-amber-400 transition-colors mr-1"
          title="切换主题 / Toggle Theme"
        >
          {settings.theme === 'light' ? <Moon className="w-3.5 h-3.5" /> : <Sun className="w-3.5 h-3.5" />}
        </button>
        {profile.windowChrome && (
          <>
            <button
              onClick={handleMinimize}
              className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-gray-800/60 text-gray-400 hover:text-gray-200 transition-colors"
              title="Minimize"
            >
              <Minus className="w-3.5 h-3.5" />
            </button>
            <button
              onClick={handleToggleMaximize}
              className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-gray-800/60 text-gray-400 hover:text-gray-200 transition-colors"
              title={isMaximized ? 'Restore' : 'Maximize'}
            >
              {isMaximized ? <Copy className="w-3 h-3" /> : <Square className="w-3 h-3" />}
            </button>
            <button
              onClick={handleClose}
              className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-red-500/80 hover:text-white text-gray-400 transition-colors"
              title="Close"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </>
        )}
      </div>
    </div>
  );
};
