import React, { useMemo, useState } from 'react';
import { ChevronDown, Globe, Star, AlertCircle, Zap, Signal } from 'lucide-react';
import { UnifiedNode } from '../../types';
import { NodePickerDialog } from './NodePickerDialog';

// Country name → Chinese translation map
const COUNTRY_ZH: Record<string, string> = {
  'United States': '美国',
  'USA': '美国',
  'US': '美国',
  'Hong Kong': '香港',
  'HK': '香港',
  'United Kingdom': '英国',
  'UK': '英国',
  'Germany': '德国',
  'DE': '德国',
  'Japan': '日本',
  'JP': '日本',
  'Singapore': '新加坡',
  'SG': '新加坡',
  'France': '法国',
  'FR': '法国',
  'Netherlands': '荷兰',
  'NL': '荷兰',
  'Canada': '加拿大',
  'CA': '加拿大',
  'Australia': '澳大利亚',
  'AU': '澳大利亚',
  'South Korea': '韩国',
  'Korea': '韩国',
  'KR': '韩国',
  'Taiwan': '台湾',
  'TW': '台湾',
  'India': '印度',
  'IN': '印度',
  'Brazil': '巴西',
  'BR': '巴西',
  'Russia': '俄罗斯',
  'RU': '俄罗斯',
  'Turkey': '土耳其',
  'TR': '土耳其',
  'Italy': '意大利',
  'IT': '意大利',
  'Spain': '西班牙',
  'ES': '西班牙',
  'Switzerland': '瑞士',
  'CH': '瑞士',
  'Sweden': '瑞典',
  'SE': '瑞典',
  'Poland': '波兰',
  'PL': '波兰',
  'Czech Republic': '捷克',
  'CZ': '捷克',
  'Finland': '芬兰',
  'FI': '芬兰',
  'Norway': '挪威',
  'NO': '挪威',
  'Denmark': '丹麦',
  'DK': '丹麦',
  'Argentina': '阿根廷',
  'AR': '阿根廷',
  'Mexico': '墨西哥',
  'MX': '墨西哥',
  'Thailand': '泰国',
  'TH': '泰国',
  'Indonesia': '印度尼西亚',
  'ID': '印度尼西亚',
  'Vietnam': '越南',
  'VN': '越南',
  'Philippines': '菲律宾',
  'PH': '菲律宾',
  'Malaysia': '马来西亚',
  'MY': '马来西亚',
  'Ukraine': '乌克兰',
  'UA': '乌克兰',
  'Romania': '罗马尼亚',
  'RO': '罗马尼亚',
  'Hungary': '匈牙利',
  'HU': '匈牙利',
  'Bulgaria': '保加利亚',
  'BG': '保加利亚',
  'Portugal': '葡萄牙',
  'PT': '葡萄牙',
  'Belgium': '比利时',
  'BE': '比利时',
  'Austria': '奥地利',
  'AT': '奥地利',
  'New Zealand': '新西兰',
  'NZ': '新西兰',
  'Israel': '以色列',
  'IL': '以色列',
  'South Africa': '南非',
  'ZA': '南非',
  'Egypt': '埃及',
  'EG': '埃及',
  'United Arab Emirates': '阿联酋',
  'UAE': '阿联酋',
  'AE': '阿联酋',
};

function getCountryZh(name: string): string {
  return COUNTRY_ZH[name] || name;
}

function formatSpeed(bps?: number | null): string {
  if (!bps || bps <= 0) return '';
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)}KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(1)}MB/s`;
}

function formatLatency(ms?: number | null): string {
  if (!ms || ms <= 0) return '';
  return `${ms}ms`;
}

/** Pick the best node from a list: speed_bps desc → latency_ms asc → first alive */
export function getBestNode(nodes: UnifiedNode[]): UnifiedNode | null {
  const alive = nodes.filter((n) => n.status !== 'dead');
  if (!alive.length) return nodes[0] || null; // all dead, return first for display

  const withSpeed = alive.filter((n) => n.speed_bps && n.speed_bps > 0);
  if (withSpeed.length) {
    return withSpeed.reduce((a, b) => (b.speed_bps! > a.speed_bps! ? b : a));
  }
  const withLatency = alive.filter((n) => n.latency_ms && n.latency_ms > 0);
  if (withLatency.length) {
    return withLatency.reduce((a, b) => (b.latency_ms! < a.latency_ms! ? b : a));
  }
  return alive[0];
}

export interface CountryGroup {
  country: string; // original country_name from node
  countryZh: string;
  nodes: UnifiedNode[];
  bestNode: UnifiedNode | null;
  hasAlive: boolean;
}

interface CountryNodeSelectorProps {
  nodes: UnifiedNode[];
  /** Currently selected value: 'smart' or a node id */
  value: string;
  onChange: (value: string) => void;
}

export const CountryNodeSelector: React.FC<CountryNodeSelectorProps> = ({
  nodes,
  value,
  onChange,
}) => {
  const [open, setOpen] = useState(false);
  const [pickerCountry, setPickerCountry] = useState<CountryGroup | null>(null);
  const [lastTap, setLastTap] = useState<{ country: string; time: number } | null>(null);

  // Compute global best node for "smart" option
  const globalBest = useMemo(() => getBestNode(nodes), [nodes]);

  // Build country groups
  const countryGroups = useMemo<CountryGroup[]>(() => {
    const map: Record<string, UnifiedNode[]> = {};
    for (const node of nodes) {
      const c = node.country_name || '未知';
      if (!map[c]) map[c] = [];
      map[c].push(node);
    }
    return Object.entries(map)
      .map(([country, ns]) => {
        const bestNode = getBestNode(ns);
        const hasAlive = ns.some((n) => n.status !== 'dead');
        return {
          country,
          countryZh: getCountryZh(country),
          nodes: ns,
          bestNode,
          hasAlive,
        };
      })
      .sort((a, b) => {
        // Sort: countries with alive nodes first, then alphabetically
        if (a.hasAlive !== b.hasAlive) return a.hasAlive ? -1 : 1;
        return a.countryZh.localeCompare(b.countryZh, 'zh');
      });
  }, [nodes]);

  // Derive display label for the current value
  const currentLabel = useMemo(() => {
    if (value === 'smart') {
      return { primary: '🌐 智能最优线路', secondary: globalBest ? `自动选择: ${globalBest.name}` : '无可用节点' };
    }
    // Find which country+node
    for (const cg of countryGroups) {
      const found = cg.nodes.find((n) => n.id === value);
      if (found) {
        return { primary: cg.countryZh, secondary: found.name };
      }
    }
    return { primary: '请选择节点', secondary: '' };
  }, [value, globalBest, countryGroups]);

  // Handle double-click / double-tap on country item to open node picker
  const handleCountryDoubleClick = (cg: CountryGroup) => {
    setOpen(false);
    setPickerCountry(cg);
  };

  const handleCountryClick = (cg: CountryGroup) => {
    if (!cg.hasAlive) return;

    // Detect double-tap (for touch/mouse)
    const now = Date.now();
    if (lastTap && lastTap.country === cg.country && now - lastTap.time < 400) {
      // Double tap/click
      handleCountryDoubleClick(cg);
      setLastTap(null);
      return;
    }
    setLastTap({ country: cg.country, time: now });

    // Single click: select best node of this country
    if (cg.bestNode) {
      onChange(cg.bestNode.id);
      setOpen(false);
    }
  };

  const selectedSpeed = useMemo(() => {
    if (value === 'smart') return globalBest?.speed_bps;
    return nodes.find((n) => n.id === value)?.speed_bps;
  }, [value, globalBest, nodes]);

  const selectedLatency = useMemo(() => {
    if (value === 'smart') return globalBest?.latency_ms;
    return nodes.find((n) => n.id === value)?.latency_ms;
  }, [value, globalBest, nodes]);

  return (
    <div className="relative w-full">
      {/* Trigger button */}
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between bg-[#12151f] border border-[#212637] hover:border-[#3a4258] rounded-2xl px-5 py-4 transition-all"
      >
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-10 h-10 rounded-xl bg-[#1a1e2d] border border-[#282f45] flex items-center justify-center text-blue-400 shrink-0">
            <Globe className="w-5 h-5" />
          </div>
          <div className="text-left min-w-0">
            <div className="text-sm font-semibold text-gray-100 truncate">{currentLabel.primary}</div>
            <div className="text-xs text-gray-500 mt-0.5 truncate">{currentLabel.secondary}</div>
          </div>
        </div>
        <div className="flex items-center gap-2 ml-3 shrink-0">
          {/* Speed / latency badge */}
          {selectedSpeed && selectedSpeed > 0 ? (
            <span className="flex items-center gap-1 text-xs text-emerald-400 font-mono">
              <Zap className="w-3 h-3" />
              {formatSpeed(selectedSpeed)}
            </span>
          ) : selectedLatency && selectedLatency > 0 ? (
            <span className="flex items-center gap-1 text-xs text-blue-400 font-mono">
              <Signal className="w-3 h-3" />
              {formatLatency(selectedLatency)}
            </span>
          ) : null}
          <ChevronDown
            className={`w-4 h-4 text-gray-500 transition-transform ${open ? 'rotate-180' : ''}`}
          />
        </div>
      </button>

      {/* Dropdown */}
      {open && (
        <div className="absolute bottom-full mb-2 left-0 right-0 bg-[#0f1219] border border-[#2a3050] rounded-2xl shadow-2xl shadow-black/60 overflow-hidden z-40 max-h-72 flex flex-col">
          {/* Smart option */}
          <button
            onClick={() => {
              onChange('smart');
              setOpen(false);
            }}
            className={`flex items-center gap-3 px-5 py-3.5 hover:bg-[#161b28] transition-colors text-left border-b border-[#1e2535] ${
              value === 'smart' ? 'bg-indigo-600/10' : ''
            }`}
          >
            <Star
              className={`w-4 h-4 shrink-0 ${value === 'smart' ? 'text-yellow-400' : 'text-gray-500'}`}
            />
            <div className="flex-1 min-w-0">
              <div className={`text-sm font-semibold ${value === 'smart' ? 'text-indigo-300' : 'text-gray-200'}`}>
                🌐 智能最优线路
              </div>
              <div className="text-xs text-gray-500 mt-0.5 truncate">
                {globalBest ? `自动选择速度最快节点` : '无可用节点'}
              </div>
            </div>
            {value === 'smart' && (
              <span className="text-[10px] text-indigo-400 bg-indigo-500/10 border border-indigo-500/30 px-2 py-0.5 rounded-full shrink-0">
                已选
              </span>
            )}
          </button>

          {/* Country list */}
          <div className="overflow-y-auto">
            {countryGroups.map((cg) => {
              const isSelected = cg.nodes.some((n) => n.id === value);
              const speed = cg.bestNode?.speed_bps;
              const latency = cg.bestNode?.latency_ms;
              return (
                <button
                  key={cg.country}
                  onClick={() => handleCountryClick(cg)}
                  onDoubleClick={() => cg.hasAlive && handleCountryDoubleClick(cg)}
                  disabled={!cg.hasAlive}
                  className={`w-full flex items-center gap-3 px-5 py-3.5 transition-colors text-left ${
                    isSelected
                      ? 'bg-indigo-600/10'
                      : !cg.hasAlive
                      ? 'opacity-40 cursor-not-allowed'
                      : 'hover:bg-[#161b28]'
                  }`}
                >
                  <div className="flex-1 min-w-0">
                    <div className={`text-sm font-medium ${isSelected ? 'text-indigo-300' : !cg.hasAlive ? 'text-gray-500' : 'text-gray-200'}`}>
                      {cg.countryZh}
                    </div>
                    <div className="text-xs text-gray-600 mt-0.5">
                      {cg.nodes.length} 个节点 · 双击选择具体节点
                    </div>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {!cg.hasAlive ? (
                      <AlertCircle className="w-3.5 h-3.5 text-gray-600" />
                    ) : speed && speed > 0 ? (
                      <span className="text-xs text-emerald-400 font-mono">{formatSpeed(speed)}</span>
                    ) : latency && latency > 0 ? (
                      <span className="text-xs text-blue-400 font-mono">{formatLatency(latency)}</span>
                    ) : (
                      <span className="text-xs text-gray-600">未测速</span>
                    )}
                    {isSelected && (
                      <span className="text-[10px] text-indigo-400 bg-indigo-500/10 border border-indigo-500/30 px-2 py-0.5 rounded-full">
                        已选
                      </span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      )}

      {/* Country Node Picker Dialog */}
      {pickerCountry && (
        <NodePickerDialog
          countryName={pickerCountry.countryZh}
          nodes={pickerCountry.nodes}
          selectedNodeId={value === 'smart' ? null : value}
          onSelect={(nodeId) => onChange(nodeId)}
          onClose={() => setPickerCountry(null)}
        />
      )}
    </div>
  );
};
