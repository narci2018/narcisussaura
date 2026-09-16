import React, { useMemo, useState } from 'react';
import { ChevronDown, Globe, Star, AlertCircle, Zap, Signal, ListFilter } from 'lucide-react';
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
function parseMetrics(node: UnifiedNode) {
  let speed = node.speed_bps || 0;
  let latency = node.latency_ms || 99999;
  
  if (speed === 0 || latency === 99999) {
    const match = node.name.match(/(\d+)ms-(\d+)Mbps/);
    if (match) {
      if (latency === 99999) latency = parseInt(match[1], 10);
      if (speed === 0) speed = parseInt(match[2], 10) * 1024 * 1024 / 8;
    }
  }
  return { speed, latency };
}

export function getBestNode(nodes: UnifiedNode[]): UnifiedNode | null {
  const alive = nodes.filter((n) => n.status !== 'dead');
  if (!alive.length) return nodes[0] || null; // all dead, return first for display

  // Sort alive nodes by parsed speed (descending) and then latency (ascending)
  const sorted = [...alive].sort((a, b) => {
    const aMetrics = parseMetrics(a);
    const bMetrics = parseMetrics(b);
    
    if (bMetrics.speed !== aMetrics.speed) {
      return bMetrics.speed - aMetrics.speed;
    }
    return aMetrics.latency - bMetrics.latency;
  });

  return sorted[0];
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

  // Compute global best node for "smart" option (exclude special groups)
  const SPECIAL_GROUPS = ['Psiphon', 'VPNGate', 'MegaV', 'Cloudflare WARP (MASQUE)', 'Cloudflare WARP (WireGuard)'];
  const regularNodes = useMemo(() => nodes.filter(n => !SPECIAL_GROUPS.includes(n.group)), [nodes]);
  const globalBest = useMemo(() => getBestNode(regularNodes), [regularNodes]);

  // Build country groups (exclude special protocol nodes)
  const countryGroups = useMemo<CountryGroup[]>(() => {
    // Extensive list of canonical country codes to always show in the dropdown
    const FIXED_COUNTRIES = [
      { code: 'US', zh: '美国', en: 'United States' },
      { code: 'HK', zh: '香港', en: 'Hong Kong' },
      { code: 'JP', zh: '日本', en: 'Japan' },
      { code: 'SG', zh: '新加坡', en: 'Singapore' },
      { code: 'GB', zh: '英国', en: 'United Kingdom' },
      { code: 'TW', zh: '台湾', en: 'Taiwan' },
      { code: 'KR', zh: '韩国', en: 'South Korea' },
      { code: 'DE', zh: '德国', en: 'Germany' },
      { code: 'FR', zh: '法国', en: 'France' },
      { code: 'AU', zh: '澳大利亚', en: 'Australia' },
      { code: 'CA', zh: '加拿大', en: 'Canada' },
      { code: 'NL', zh: '荷兰', en: 'Netherlands' },
      { code: 'IN', zh: '印度', en: 'India' },
      { code: 'BR', zh: '巴西', en: 'Brazil' },
      { code: 'RU', zh: '俄罗斯', en: 'Russia' },
      { code: 'TR', zh: '土耳其', en: 'Turkey' },
      { code: 'IT', zh: '意大利', en: 'Italy' },
      { code: 'ES', zh: '西班牙', en: 'Spain' },
      { code: 'CH', zh: '瑞士', en: 'Switzerland' },
      { code: 'SE', zh: '瑞典', en: 'Sweden' },
    ];
    
    // Create buckets for each fixed country
    const map: Record<string, { code: string, label: string, nodes: UnifiedNode[] }> = {};
    for (const fc of FIXED_COUNTRIES) {
      map[fc.code] = {
        code: fc.code,
        label: `${fc.zh} (${fc.code})`,
        nodes: []
      };
    }
    
    for (const node of nodes) {
      const isSpecial = 
        SPECIAL_GROUPS.includes(node.group) || 
        ['psiphon', 'vpngate'].includes(node.protocol) ||
        node.name.includes('VPNGate') || node.name.includes('Psiphon') || node.name.includes('MegaV') || node.name.includes('WARP');
        
      if (isSpecial) continue;
      
      // Node's country_code from backend parser (e.g. "US", "HK")
      let code = node.country_code;
      
      // Fallback for nodes that were saved to local disk BEFORE the v0.2.7 backend fix
      if (!code) {
        const upper = node.name.toUpperCase();
        const words = upper.split(/[^A-Z]+/);
        const hasCode = (c: string) => words.includes(c);
        
        if (node.name.includes("香港") || upper.includes("HONG KONG") || hasCode('HK')) code = 'HK';
        else if (node.name.includes("台湾") || node.name.includes("台灣") || upper.includes("TAIWAN") || hasCode('TW')) code = 'TW';
        else if (node.name.includes("日本") || upper.includes("JAPAN") || upper.includes("TOKYO") || hasCode('JP')) code = 'JP';
        else if (node.name.includes("美国") || node.name.includes("美國") || upper.includes("UNITED STATES") || hasCode('US') || hasCode('USA')) code = 'US';
        else if (node.name.includes("新加坡") || node.name.includes("狮城") || upper.includes("SINGAPORE") || hasCode('SG')) code = 'SG';
        else if (node.name.includes("韩国") || upper.includes("KOREA") || upper.includes("SOUTH KOREA") || hasCode('KR')) code = 'KR';
        else if (node.name.includes("英国") || upper.includes("BRITAIN") || upper.includes("UNITED KINGDOM") || hasCode('GB') || hasCode('UK')) code = 'GB';
        else if (node.name.includes("德国") || upper.includes("GERMANY") || hasCode('DE')) code = 'DE';
        else if (node.name.includes("法国") || upper.includes("FRANCE") || hasCode('FR')) code = 'FR';
        else if (node.name.includes("澳洲") || node.name.includes("澳大利亚") || upper.includes("AUSTRALIA") || hasCode('AU')) code = 'AU';
        else if (node.name.includes("加拿大") || upper.includes("CANADA") || hasCode('CA')) code = 'CA';
        else if (node.name.includes("荷兰") || upper.includes("NETHERLANDS") || hasCode('NL')) code = 'NL';
        else if (node.name.includes("印度") || upper.includes("INDIA") || hasCode('IN')) code = 'IN';
        else if (node.name.includes("巴西") || upper.includes("BRAZIL") || hasCode('BR')) code = 'BR';
        else if (node.name.includes("俄罗斯") || upper.includes("RUSSIA") || hasCode('RU')) code = 'RU';
        else if (node.name.includes("土耳其") || upper.includes("TURKEY") || hasCode('TR')) code = 'TR';
        else if (node.name.includes("意大利") || upper.includes("ITALY") || hasCode('IT')) code = 'IT';
        else if (node.name.includes("西班牙") || upper.includes("SPAIN") || hasCode('ES')) code = 'ES';
        else if (node.name.includes("瑞士") || upper.includes("SWITZERLAND") || hasCode('CH')) code = 'CH';
        else if (node.name.includes("瑞典") || upper.includes("SWEDEN") || hasCode('SE')) code = 'SE';
        else code = 'OTHER';
      }
      
      // If it's a known country code, place it in the bucket
      if (map[code]) {
        map[code].nodes.push(node);
      } else {
        // Unrecognized country or empty country code goes to a dynamically created bucket
        if (!map[code]) {
           const label = code === 'OTHER' ? '其他地区 (Other)' : `${getCountryZh(node.country_name || code)} (${code})`;
           map[code] = { code, label, nodes: [] };
        }
        map[code].nodes.push(node);
      }
    }

    return Object.values(map)
      .map(({ code, label, nodes: ns }) => {
        const bestNode = getBestNode(ns);
        const hasAlive = ns.some((n) => n.status !== 'dead');
        return {
          country: code,      // We use code as the unique key instead of English name
          countryZh: label,   // E.g., "美国 (US)"
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

  const handleCountryClick = (cg: CountryGroup) => {
    if (!cg.hasAlive) return;
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

  // Find the currently selected country group (for manual picker button)
  const currentCountryGroup = useMemo(() => {
    if (value === 'smart') return null;
    return countryGroups.find((cg) => cg.nodes.some((n) => n.id === value)) ?? null;
  }, [value, countryGroups]);

  // Nodes to show in manual picker: current country's nodes, or all nodes for smart
  const pickerNodes = useMemo(() => {
    if (value === 'smart') return nodes;
    return currentCountryGroup?.nodes ?? nodes;
  }, [value, currentCountryGroup, nodes]);

  const pickerLabel = value === 'smart' ? '全部节点' : (currentCountryGroup?.countryZh ?? '全部节点');

  return (
    <div className="relative w-full">
      {/* Trigger row: dropdown selector + manual picker button */}
      <div className="flex gap-2">
        {/* Dropdown trigger */}
        <button
          onClick={() => setOpen(!open)}
          className="flex-1 flex items-center justify-between bg-[#12151f] border border-[#212637] hover:border-[#3a4258] rounded-2xl px-4 py-3.5 transition-all min-w-0"
        >
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-9 h-9 rounded-xl bg-[#1a1e2d] border border-[#282f45] flex items-center justify-center text-blue-400 shrink-0">
              <Globe className="w-4.5 h-4.5" />
            </div>
            <div className="text-left min-w-0">
              <div className="text-sm font-semibold text-gray-100 truncate">{currentLabel.primary}</div>
              <div className="text-xs text-gray-500 mt-0.5 truncate">{currentLabel.secondary}</div>
            </div>
          </div>
          <div className="flex items-center gap-2 ml-2 shrink-0">
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

        {/* Manual node picker button */}
        <button
          onClick={() => {
            setOpen(false);
            setPickerCountry({ 
              country: currentCountryGroup?.country ?? '__all__',
              countryZh: pickerLabel,
              nodes: pickerNodes,
              bestNode: getBestNode(pickerNodes),
              hasAlive: pickerNodes.some(n => n.status !== 'dead'),
            });
          }}
          disabled={pickerNodes.length === 0}
          className="flex flex-col items-center justify-center gap-1 bg-[#12151f] border border-[#212637] hover:border-indigo-500/50 hover:bg-indigo-500/5 rounded-2xl px-3 py-2 transition-all shrink-0 disabled:opacity-40 disabled:cursor-not-allowed"
          title="手动选择具体节点"
        >
          <ListFilter className="w-4 h-4 text-indigo-400" />
          <span className="text-[10px] text-gray-500 leading-none whitespace-nowrap">手动选线路</span>
        </button>
      </div>

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
                {globalBest ? `自动选择速度最快节点` : '无可用节点，错峰上网'}
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
                      {cg.nodes.length} 个节点 · 点击自动择优
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
