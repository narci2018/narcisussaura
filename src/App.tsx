import React, { useEffect, useState } from 'react';
import { AlertCircle, X, Copy, Check } from 'lucide-react';
import { TitleBar } from './components/TitleBar';
import { Navigation } from './components/Navigation';
import { Dashboard } from './components/Dashboard';
import { ServerList } from './components/ServerList';
import { PsiphonView } from './components/PsiphonView';
import { VPNGateView } from './components/VPNGateView';
import { MegaVView } from './components/MegaVView';
import { SubscriptionsView } from './components/SubscriptionsView';
import { ImportModal } from './components/ImportModal';
import { SettingsView } from './components/SettingsView';
import { ChainedProxyView } from './components/ChainedProxyView';
import { SimpleDashboard } from './components/SimpleMode';
import { useAppStore } from './stores/appStore';
import './App.css';

const APP_MODE_KEY = 'app_mode';

export const App: React.FC = () => {
  const { activeTab, init, errorMessage, setErrorMessage } = useAppStore();
  const [copiedError, setCopiedError] = useState(false);

  // Mode: 'simple' (default) or 'expert'
  const [mode, setMode] = useState<'simple' | 'expert'>(() => {
    try {
      const saved = localStorage.getItem(APP_MODE_KEY);
      return saved === 'expert' ? 'expert' : 'simple';
    } catch {
      return 'simple';
    }
  });

  useEffect(() => {
    init();
  }, [init]);

  const switchToExpert = () => {
    setMode('expert');
    try {
      localStorage.setItem(APP_MODE_KEY, 'expert');
    } catch {}
  };

  const switchToSimple = () => {
    setMode('simple');
    try {
      localStorage.setItem(APP_MODE_KEY, 'simple');
    } catch {}
  };

  const handleCopyError = () => {
    if (!errorMessage) return;
    navigator.clipboard.writeText(errorMessage);
    setCopiedError(true);
    setTimeout(() => setCopiedError(false), 2000);
  };

  // ─── Simple Mode ─────────────────────────────────────────────────────────────
  if (mode === 'simple') {
    return (
      <div className="flex flex-col h-screen w-screen bg-[#090a0f] text-gray-100 overflow-hidden select-none">
        {/* Keep TitleBar for window dragging */}
        <TitleBar />
        <div className="flex-1 overflow-hidden">
          <SimpleDashboard onSwitchToExpert={switchToExpert} />
        </div>
      </div>
    );
  }

  // ─── Expert Mode ─────────────────────────────────────────────────────────────
  return (
    <div className="flex flex-col h-screen w-screen bg-[#090a0f] text-gray-100 overflow-hidden select-none relative">
      {/* Frameless Draggable TitleBar */}
      <TitleBar onSwitchToSimple={switchToSimple} />

      {/* Main Navigation */}
      <Navigation />

      {/* Global Error Floating Banner */}
      {errorMessage && (
        <div className="absolute top-12 left-1/2 -translate-x-1/2 z-50 max-w-xl w-[92%] bg-red-950/95 border border-red-500/70 shadow-2xl shadow-red-950/90 rounded-xl p-3.5 flex items-start gap-3 backdrop-blur-md animate-in fade-in slide-from-top-3 duration-200">
          <div className="w-6 h-6 rounded-lg bg-red-500/20 border border-red-500/30 flex items-center justify-center shrink-0 mt-0.5 text-red-400">
            <AlertCircle className="w-3.5 h-3.5" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center justify-between gap-2 mb-1">
              <span className="text-[11px] font-bold uppercase tracking-wider text-red-300">
                连接错误提示 / Connection Notice
              </span>
              <div className="flex items-center gap-1.5">
                <button
                  onClick={handleCopyError}
                  className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-lg bg-red-900/60 hover:bg-red-800/80 text-red-200 border border-red-500/40 transition-colors shadow-sm"
                  title="拷贝错误信息到剪贴板"
                >
                  {copiedError ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                  <span>{copiedError ? '已拷贝' : '拷贝错误'}</span>
                </button>
                <button
                  onClick={() => setErrorMessage(null)}
                  className="text-gray-400 hover:text-white p-1 rounded-lg hover:bg-white/10 transition-colors"
                  title="关闭"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>
            <p className="text-xs text-red-100 leading-relaxed font-mono whitespace-pre-wrap break-words max-h-32 overflow-y-auto pr-1 select-text">
              {errorMessage}
            </p>
          </div>
        </div>
      )}

      {/* View Container */}
      <main className="flex-1 flex overflow-hidden">
        {activeTab === 'dashboard' && <Dashboard />}
        {activeTab === 'servers' && <ServerList />}
        {activeTab === 'chains' && <ChainedProxyView />}
        {activeTab === 'psiphon' && <PsiphonView />}
        {activeTab === 'vpngate' && <VPNGateView />}
        {activeTab === 'megav' && <MegaVView />}
        {activeTab === 'subscriptions' && <SubscriptionsView />}
        {activeTab === 'import' && <ImportModal />}
        {activeTab === 'settings' && <SettingsView />}
      </main>
    </div>
  );
};

export default App;
