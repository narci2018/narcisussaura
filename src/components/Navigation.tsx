import { LayoutDashboard, Server, FolderSync, PlusCircle, Settings as SettingsIcon, Globe, Shield, Zap, Link2 } from 'lucide-react';
import { useAppStore } from '../stores/appStore';

export const Navigation: React.FC = () => {
  const { activeTab, setActiveTab } = useAppStore();

  const tabs = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'servers', label: 'Servers', icon: Server },
    { id: 'chains', label: 'Chained Proxy', icon: Link2 },
    { id: 'psiphon', label: 'Psiphon', icon: Globe },
    { id: 'vpngate', label: 'VPNGate', icon: Shield },
    { id: 'megav', label: 'MegaV', icon: Zap },
    { id: 'subscriptions', label: 'Subscriptions', icon: FolderSync },
    { id: 'import', label: 'Import', icon: PlusCircle },
    { id: 'settings', label: 'Settings', icon: SettingsIcon },
  ] as const;

  return (
    <div className="w-full bg-[#0c0e14] border-b border-[#1c1f2b] px-6 py-2 flex items-center justify-between">
      <div className="flex items-center gap-1.5 bg-[#12151e] p-1 rounded-xl border border-[#222736]">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex items-center gap-2 px-4 py-1.5 rounded-lg text-xs font-medium transition-all ${
                isActive
                  ? 'bg-blue-600 text-white shadow-sm shadow-blue-500/25'
                  : 'text-gray-400 hover:text-gray-200 hover:bg-[#1a1e2b]'
              }`}
            >
              <Icon className="w-4 h-4" />
              <span>{tab.label}</span>
            </button>
          );
        })}
      </div>

      <div className="text-[11px] text-gray-500 font-mono">
        sing-box v1.14.0 Core
      </div>
    </div>
  );
};
