import React, { useEffect, useRef } from 'react';
import { LayoutDashboard, Server, FolderSync, PlusCircle, Settings as SettingsIcon, Globe, Shield, Zap, Link2, Home } from 'lucide-react';
import { useAppStore } from '../../../stores/appStore';
import { ActiveTab } from '../../../types';

// Android 专家模式顶栏:桌面版一行摆 10 个 tab 在 ~390px 视口里必然溢出
// (后 8 个看不见也摸不着),这里改为横向可滚动的触控形态:
// 图标+文字常显、触控高度 ≥40px、激活项自动滚入视野。
export const Navigation: React.FC = () => {
  const { activeTab, setActiveTab, residentialSubUrl } = useAppStore();
  const barRef = useRef<HTMLDivElement>(null);

  const allTabs: { id: ActiveTab; label: string; icon: React.ComponentType<{ className?: string }>; condition?: boolean }[] = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'servers', label: 'Servers', icon: Server },
    { id: 'chains', label: 'Chained Proxy', icon: Link2 },
    { id: 'psiphon', label: 'Psiphon', icon: Globe },
    { id: 'vpngate', label: 'VPNGate', icon: Shield },
    { id: 'residential', label: 'Residential IP', icon: Home, condition: Boolean(residentialSubUrl && residentialSubUrl.trim()) },
    { id: 'megav', label: 'MegaV', icon: Zap },
    { id: 'subscriptions', label: 'Subscriptions', icon: FolderSync },
    { id: 'import', label: 'Import', icon: PlusCircle },
    { id: 'settings', label: 'Settings', icon: SettingsIcon },
  ];

  const visibleTabs = allTabs.filter(tab => tab.condition === undefined || tab.condition);

  useEffect(() => {
    const el = barRef.current?.querySelector<HTMLElement>(`[data-tab="${activeTab}"]`);
    el?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
  }, [activeTab]);

  return (
    <div
      ref={barRef}
      className="w-full bg-[#0c0e14] border-b border-[#1c1f2b] px-2 py-1.5 flex items-center gap-1.5 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      {visibleTabs.map((tab) => {
        const Icon = tab.icon;
        const isActive = activeTab === tab.id;
        return (
          <button
            key={tab.id}
            data-tab={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`shrink-0 flex items-center gap-1.5 px-3.5 h-10 rounded-lg text-[13px] font-medium transition-all ${
              isActive
                ? 'bg-blue-600 text-white shadow-sm shadow-blue-500/25'
                : 'text-gray-400 active:text-gray-200 active:bg-[#1a1e2b]'
            }`}
          >
            <Icon className="w-[18px] h-[18px]" />
            <span>{tab.label}</span>
          </button>
        );
      })}
    </div>
  );
};
