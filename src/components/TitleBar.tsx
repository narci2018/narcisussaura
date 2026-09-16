import React, { useState, useEffect } from 'react';
import { Minus, Square, Copy, X, Shield, Smartphone, Sun, Moon } from 'lucide-react';
import { getVersion } from '@tauri-apps/api/app';
import { api } from '../services/api';
import { useAppStore } from '../stores/appStore';

interface TitleBarProps {
  /** If provided, shows a "小白模式" back button in expert mode */
  onSwitchToSimple?: () => void;
}

export const TitleBar: React.FC<TitleBarProps> = ({ onSwitchToSimple }) => {
  const { status, connectedNode, settings, saveSettings } = useAppStore();
  const [isMaximized, setIsMaximized] = useState(false);
  const [appVersion, setAppVersion] = useState('');

  useEffect(() => {
    api.isWindowMaximized().then(setIsMaximized).catch(() => {});
    getVersion().then(v => setAppVersion(`v${v}`)).catch(() => setAppVersion('v0.2.4'));
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
      data-tauri-drag-region
      onDoubleClick={handleToggleMaximize}
      className="h-10 bg-[#090a0f] border-b border-[#1c1f2b] flex items-center justify-between px-3 select-none z-50 text-xs font-medium text-gray-400 cursor-default"
    >
      {/* Left: Brand & Status pill */}
      <div className="flex items-center gap-2.5 pointer-events-none" data-tauri-drag-region>
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
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/20 text-[11px]">
            <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-ping" />
            <span>Connecting...</span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-gray-800/40 text-gray-400 border border-gray-700/30 text-[11px]">
            <span className="w-1.5 h-1.5 rounded-full bg-gray-500" />
            <span>Disconnected</span>
          </div>
        )}
      </div>

      {/* Right: Mode switch + Window Controls */}
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
      </div>
    </div>
  );
};
