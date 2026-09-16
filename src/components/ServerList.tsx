import React, { useMemo, useRef } from 'react';
import {
  Search,
  Star,
  Zap,
  Trash2,
  Check,
  Plus,
  RefreshCw,
  Globe,
  Radio,
  Gauge,
  Sparkles,
  FolderSync,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';
import { useAppStore } from '../stores/appStore';
import { UnifiedNode } from '../types';

function formatSpeed(bytesPerSec?: number | null): string {
  if (!bytesPerSec || bytesPerSec <= 0) return '';
  const mb = bytesPerSec / (1024 * 1024);
  if (mb >= 1) {
    return `${mb.toFixed(1)} MB/s`;
  }
  const kb = bytesPerSec / 1024;
  return `${kb.toFixed(0)} KB/s`;
}

export const ServerList: React.FC = () => {
  const {
    nodes,
    subscriptions,
    selectedNodeId,
    connectedNode,
    status,
    searchQuery,
    setSearchQuery,
    selectedProtocol,
    setSelectedProtocol,
    selectedGroup,
    setSelectedGroup,
    filterFavorite,
    setFilterFavorite,
    sortMode,
    setSortMode,
    setSelectedNodeId,
    connect,
    testLatency,
    testSpeed,
    testAllNodes,
    testAllSpeeds,
    isTestingAll,
    isTestingAllSpeed,
    testingLatencyIds,
    testingSpeedIds,
    toggleFavorite,
    deleteNode,
    setActiveTab,
  } = useAppStore();

  const groupContainerRef = useRef<HTMLDivElement>(null);
  const protocols = ['all', 'masque', 'wireguard', 'vless', 'shadowsocks', 'trojan', 'socks5', 'http'];

  // Calculate group node counts
  const groupCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const node of nodes) {
      const g = node.group?.trim() || 'Default';
      counts[g] = (counts[g] || 0) + 1;
    }
    return counts;
  }, [nodes]);

  // Groups to exclude from Servers view (these have dedicated tabs)
  const SPECIAL_GROUPS = ['Psiphon', 'VPNGate', 'MegaV'];

  // Extract unique groups from nodes and subscriptions, with WARP groups prioritized
  const availableGroups = useMemo(() => {
    const groupSet = new Set<string>();
    const warpPriorities = ['Cloudflare WARP (MASQUE)', 'Cloudflare WARP (WireGuard)'];
    for (const wp of warpPriorities) {
      if (subscriptions.some((s) => s.name === wp) || nodes.some((n) => n.group === wp)) {
        groupSet.add(wp);
      }
    }
    for (const sub of subscriptions) {
      if (sub.name) groupSet.add(sub.name);
    }
    for (const node of nodes) {
      if (node.group && node.group.trim() && !SPECIAL_GROUPS.includes(node.group)) {
        groupSet.add(node.group);
      }
    }
    return ['all', ...Array.from(groupSet)];
  }, [subscriptions, nodes]);

  const handleGroupWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (groupContainerRef.current && e.deltaY !== 0) {
      e.preventDefault();
      groupContainerRef.current.scrollLeft += e.deltaY;
    }
  };

  const scrollGroups = (offset: number) => {
    if (groupContainerRef.current) {
      groupContainerRef.current.scrollBy({ left: offset, behavior: 'smooth' });
    }
  };

  // Filtering & Sorting

  const filteredAndSortedNodes = useMemo(() => {
    let result = nodes.filter((node) => {
      // Exclude Psiphon / VPNGate / MegaV nodes from the general server list
      const isSpecial = 
        SPECIAL_GROUPS.includes(node.group) || 
        ['psiphon', 'vpngate'].includes(node.protocol) ||
        node.name.includes('VPNGate') || node.name.includes('Psiphon') || node.name.includes('MegaV');
        
      if (isSpecial) return false;
      if (filterFavorite && !node.favorite) return false;
      if (selectedProtocol !== 'all' && node.protocol !== selectedProtocol) return false;
      if (selectedGroup !== 'all' && node.group !== selectedGroup) return false;
      if (searchQuery) {
        const q = searchQuery.toLowerCase();
        return (
          node.name.toLowerCase().includes(q) ||
          node.address.toLowerCase().includes(q) ||
          node.country_name.toLowerCase().includes(q) ||
          node.group.toLowerCase().includes(q)
        );
      }
      return true;
    });

    if (sortMode === 'speed') {
      result.sort((a, b) => {
        const aLatencyValid = a.latency_ms !== undefined && a.latency_ms !== null && a.latency_ms > 0 && a.latency_ms < 1000;
        const bLatencyValid = b.latency_ms !== undefined && b.latency_ms !== null && b.latency_ms > 0 && b.latency_ms < 1000;

        if (aLatencyValid && !bLatencyValid) return -1;
        if (!aLatencyValid && bLatencyValid) return 1;

        // Both eligible: sort by speed descending
        const aSpeed = a.speed_bps || 0;
        const bSpeed = b.speed_bps || 0;
        if (aSpeed !== bSpeed) {
          return bSpeed - aSpeed;
        }

        // Fallback to lowest latency
        const aLat = a.latency_ms ?? 9999;
        const bLat = b.latency_ms ?? 9999;
        return aLat - bLat;
      });
    } else if (sortMode === 'latency') {
      result.sort((a, b) => {
        const aValid = a.latency_ms !== undefined && a.latency_ms !== null && a.latency_ms > 0;
        const bValid = b.latency_ms !== undefined && b.latency_ms !== null && b.latency_ms > 0;

        if (aValid && !bValid) return -1;
        if (!aValid && bValid) return 1;
        if (!aValid && !bValid) return 0;

        return (a.latency_ms || 9999) - (b.latency_ms || 9999);
      });
    }

    return result;
  }, [nodes, filterFavorite, selectedProtocol, selectedGroup, searchQuery, sortMode]);

  const isRecommended = (node: UnifiedNode): boolean => {
    const lat = node.latency_ms;
    const spd = node.speed_bps;
    // Latency < 1000ms and Bandwidth >= 5 MB/s (5 * 1024 * 1024 bytes/sec)
    return Boolean(
      lat !== undefined &&
      lat !== null &&
      lat > 0 &&
      lat < 1000 &&
      spd !== undefined &&
      spd !== null &&
      spd >= 5 * 1024 * 1024
    );
  };

  const getLatencyBadge = (node: UnifiedNode) => {
    const isTesting = testingLatencyIds.includes(node.id);
    if (isTesting) {
      return (
        <span className="text-[11px] text-blue-400 font-mono px-2 py-0.5 rounded bg-blue-500/10 border border-blue-500/20 flex items-center gap-1 animate-pulse">
          <RefreshCw className="w-2.5 h-2.5 animate-spin" />
          <span>Ping...</span>
        </span>
      );
    }

    const latency = node.latency_ms;
    if (latency === undefined || latency === null) {
      return (
        <span className="text-[11px] text-gray-500 font-mono px-2 py-0.5 rounded bg-gray-800/40 border border-gray-700/30">
          Unchecked
        </span>
      );
    }
    if (latency <= 0) {
      return (
        <span className="text-[11px] text-red-400 font-mono px-2 py-0.5 rounded bg-red-500/10 border border-red-500/20 flex items-center gap-1">
          <span>Timeout</span>
        </span>
      );
    }
    if (latency < 100) {
      return (
        <span className="text-[11px] text-emerald-400 font-mono px-2 py-0.5 rounded bg-emerald-500/10 border border-emerald-500/20">
          {latency} ms
        </span>
      );
    }
    if (latency < 250) {
      return (
        <span className="text-[11px] text-yellow-400 font-mono px-2 py-0.5 rounded bg-yellow-500/10 border border-yellow-500/20">
          {latency} ms
        </span>
      );
    }
    return (
      <span className="text-[11px] text-orange-400 font-mono px-2 py-0.5 rounded bg-orange-500/10 border border-orange-500/20">
        {latency} ms
      </span>
    );
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden px-6 py-4 max-w-[1600px] mx-auto w-full">
      {/* Top Controls Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 mb-3">
        {/* Search Input */}
        <div className="relative flex-1 min-w-[240px] max-w-md">
          <Search className="w-4 h-4 text-gray-400 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
          <input
            type="text"
            placeholder="Search servers by name, group, country, IP..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-[#12151e] border border-[#222736] rounded-xl pl-9 pr-4 py-2 text-xs text-gray-200 placeholder-gray-500 focus:outline-none focus:border-blue-500/60 transition-colors"
          />
        </div>

        {/* Sorting Controls & Action Buttons */}
        <div className="flex items-center gap-2 flex-wrap">
          {/* Sorting Segment */}
          <div className="flex items-center bg-[#111420] border border-[#222738] rounded-xl p-0.5 shadow-sm">
            <button
              onClick={() => setSortMode(sortMode === 'latency' ? 'default' : 'latency')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                sortMode === 'latency'
                  ? 'bg-blue-600/30 text-blue-300 font-semibold border border-blue-500/40 shadow-sm'
                  : 'text-gray-400 hover:text-gray-200 border border-transparent'
              }`}
              title="按延迟从小到大排序 (Lowest Ping First)"
            >
              <Zap className="w-3.5 h-3.5 text-blue-400" />
              <span>按延迟排序</span>
            </button>

            <button
              onClick={() => setSortMode(sortMode === 'speed' ? 'default' : 'speed')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                sortMode === 'speed'
                  ? 'bg-indigo-600/30 text-indigo-300 font-semibold border border-indigo-500/40 shadow-sm'
                  : 'text-gray-400 hover:text-gray-200 border border-transparent'
              }`}
              title="按真实下载速度从高到低排序 (Fastest Download First)"
            >
              <Gauge className="w-3.5 h-3.5 text-indigo-400" />
              <span>按速度排序</span>
            </button>
          </div>

          <div className="h-4 w-px bg-[#262a3b] mx-0.5" />

          {/* Test All Ping */}
          <button
            onClick={() => testAllNodes()}
            disabled={isTestingAll}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-[#151824] hover:bg-[#1d2233] text-gray-300 hover:text-white text-xs font-medium border border-[#242a3d] transition-all disabled:opacity-50 shadow-sm"
            title="对所有节点批量测试延迟 (Ping Latency)"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isTestingAll ? 'animate-spin text-blue-400' : 'text-blue-400'}`} />
            <span>{isTestingAll ? 'Pinging...' : 'Test All Ping'}</span>
          </button>

          {/* Test All Speed */}
          <button
            onClick={() => testAllSpeeds()}
            disabled={isTestingAllSpeed}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-[#151824] hover:bg-[#1d2233] text-indigo-300 hover:text-indigo-200 text-xs font-medium border border-indigo-500/30 transition-all disabled:opacity-50 shadow-sm"
            title="对所有节点批量测真实带宽速度 (Test Speed)"
          >
            <Gauge className={`w-3.5 h-3.5 ${isTestingAllSpeed ? 'animate-spin text-indigo-400' : 'text-indigo-400'}`} />
            <span>{isTestingAllSpeed ? 'Testing Speed...' : 'Test All Speed'}</span>
          </button>

          {/* Add Server */}
          <button
            onClick={() => setActiveTab('import')}
            className="flex items-center gap-1.5 px-3.5 py-2 rounded-xl bg-blue-600 hover:bg-blue-500 text-white text-xs font-medium shadow-sm shadow-blue-500/20 transition-all"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Add Server</span>
          </button>
        </div>
      </div>

      {/* Subscription / Group Tabs Bar with Left/Right Buttons and Visible Horizontal Scrollbar */}
      <div className="relative flex items-center mb-3 bg-[#0d0f17] border border-[#1b1f2e] rounded-xl px-2 py-1.5 shadow-sm">
        <span className="text-[11px] text-gray-400 font-medium mr-2 flex items-center gap-1 shrink-0 select-none">
          <FolderSync className="w-3.5 h-3.5 text-blue-400" />
          <span>分组:</span>
        </span>

        {/* Left Scroll Button */}
        <button
          onClick={() => scrollGroups(-240)}
          className="w-6 h-6 rounded-md flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-800/60 shrink-0 transition-colors mr-1"
          title="向前滚动分组"
        >
          <ChevronLeft className="w-4 h-4" />
        </button>

        {/* Scrollable Container with Custom Draggable Scrollbar & Mouse Wheel Support */}
        <div
          ref={groupContainerRef}
          onWheel={handleGroupWheel}
          className="flex-1 flex items-center gap-1.5 overflow-x-auto py-1 custom-scrollbar-x"
        >
          {availableGroups.map((grp) => {
            const isSelected = selectedGroup === grp;
            const count = grp === 'all' ? nodes.length : (groupCounts[grp] ?? 0);
            return (
              <button
                key={grp}
                onClick={() => setSelectedGroup(grp)}
                className={`px-3 py-1 rounded-lg text-xs font-medium border shrink-0 transition-all flex items-center gap-1.5 ${
                  isSelected
                    ? 'bg-blue-600/25 text-blue-300 border-blue-500/50 font-semibold shadow-sm shadow-blue-500/10'
                    : 'bg-[#111420] text-gray-400 border-[#1f2538] hover:border-[#2f3854] hover:text-gray-200'
                }`}
              >
                <span>{grp === 'all' ? 'All Subscriptions' : grp}</span>
                <span className={`text-[10px] px-1.5 py-0.2 rounded-full font-mono ${
                  isSelected ? 'bg-blue-500/30 text-blue-200' : 'bg-gray-800/80 text-gray-500'
                }`}>
                  {count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Right Scroll Button */}
        <button
          onClick={() => scrollGroups(240)}
          className="w-6 h-6 rounded-md flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-800/60 shrink-0 transition-colors ml-1"
          title="向后滚动分组"
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>

      {/* Protocol & Favorites Bar */}
      <div className="flex items-center gap-2 mb-3.5 overflow-x-auto pb-1">
        <button
          onClick={() => setFilterFavorite(!filterFavorite)}
          className={`flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-medium border transition-all ${
            filterFavorite
              ? 'bg-amber-500/10 text-amber-300 border-amber-500/30'
              : 'bg-[#12151e] text-gray-400 border-[#222736] hover:text-gray-200'
          }`}
        >
          <Star className={`w-3.5 h-3.5 ${filterFavorite ? 'fill-amber-400 text-amber-400' : ''}`} />
          <span>Favorites</span>
        </button>

        <div className="h-4 w-px bg-[#222736] mx-1" />

        {protocols.map((proto) => (
          <button
            key={proto}
            onClick={() => setSelectedProtocol(proto)}
            className={`px-3 py-1 rounded-lg text-xs font-medium border transition-all ${
              selectedProtocol === proto
                ? proto === 'masque'
                  ? 'bg-purple-500/20 text-purple-300 border-purple-500/50 font-bold shadow-sm shadow-purple-500/20'
                  : proto === 'wireguard'
                  ? 'bg-cyan-500/20 text-cyan-300 border-cyan-500/50 font-bold shadow-sm shadow-cyan-500/20'
                  : 'bg-blue-500/15 text-blue-400 border-blue-500/40 font-semibold'
                : 'bg-[#12151e] text-gray-400 border-[#222736] hover:text-gray-200'
            }`}
          >
            {proto === 'all'
              ? 'ALL'
              : proto === 'masque'
              ? 'WARP / MASQUE'
              : proto === 'wireguard'
              ? 'WARP / WireGuard'
              : proto.toUpperCase()}
          </button>
        ))}
      </div>

      {/* Nodes List */}
      <div className="flex-1 overflow-y-auto pr-1 space-y-2.5">
        {filteredAndSortedNodes.length === 0 ? (
          <div className="h-48 flex flex-col items-center justify-center text-gray-500 text-xs">
            <Radio className="w-8 h-8 mb-2 opacity-40 text-gray-400" />
            <span>No matching servers found</span>
          </div>
        ) : (
          filteredAndSortedNodes.map((node) => {
            const isSelected = selectedNodeId === node.id;
            const isLiveConnected = connectedNode?.id === node.id && status === 'connected';
            const isTestingLat = testingLatencyIds.includes(node.id);
            const isTestingSpd = testingSpeedIds.includes(node.id);
            const recommended = isRecommended(node);

            return (
              <div
                key={node.id}
                onClick={() => setSelectedNodeId(node.id)}
                className={`w-full rounded-xl p-3.5 flex items-center justify-between border transition-all cursor-pointer ${
                  isLiveConnected
                    ? 'bg-emerald-500/5 border-emerald-500/40 shadow-sm shadow-emerald-500/10'
                    : isSelected
                    ? 'bg-[#151926] border-blue-500/50 shadow-sm shadow-blue-500/10'
                    : 'bg-[#11131c] border-[#1f2433] hover:border-[#2b3348] hover:bg-[#141722]'
                }`}
              >
                {/* Left: Info */}
                <div className="flex items-center gap-3.5">
                  {/* Country Flag or Globe Icon */}
                  <div
                    className={`w-9 h-9 rounded-xl flex items-center justify-center font-bold text-xs ${
                      isLiveConnected
                        ? 'bg-emerald-500/20 text-emerald-300'
                        : 'bg-[#1b202e] text-gray-300 border border-[#272e42]'
                    }`}
                  >
                    {node.country_code ? (
                      <span className="font-mono text-xs">{node.country_code}</span>
                    ) : (
                      <Globe className="w-4 h-4 text-gray-400" />
                    )}
                  </div>

                  <div>
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-xs font-semibold text-gray-200">{node.name}</span>

                      {/* Recommended Tag */}
                      {recommended && (
                        <span className="px-1.5 py-0.5 rounded text-[10px] bg-amber-500/15 text-amber-300 font-bold border border-amber-500/30 flex items-center gap-1 shadow-sm shadow-amber-500/10">
                          <Sparkles className="w-2.5 h-2.5 text-amber-400" />
                          <span>推荐</span>
                        </span>
                      )}

                      {isLiveConnected && (
                        <span className="px-1.5 py-0.5 rounded text-[10px] bg-emerald-500/20 text-emerald-400 font-bold border border-emerald-500/30">
                          Active
                        </span>
                      )}
                    </div>

                    <div className="flex items-center gap-2 text-[11px] text-gray-400 font-mono mt-0.5 flex-wrap">
                      <span>
                        {node.address}:{node.port}
                      </span>
                      {node.protocol === 'masque' ? (
                        <span className="text-[10px] font-bold text-purple-300 bg-purple-500/15 border border-purple-500/30 px-1.5 py-0.5 rounded shadow-sm shadow-purple-500/10">
                          WARP · MASQUE (H3)
                        </span>
                      ) : node.protocol === 'wireguard' ? (
                        <span className="text-[10px] font-bold text-cyan-300 bg-cyan-500/15 border border-cyan-500/30 px-1.5 py-0.5 rounded shadow-sm shadow-cyan-500/10">
                          WARP · WireGuard
                        </span>
                      ) : (
                        <span className="uppercase text-gray-500">{node.protocol}</span>
                      )}
                      {node.group && (
                        <>
                          <span>·</span>
                          <span className="text-blue-400/80 bg-blue-500/10 px-1.5 py-0.2 rounded border border-blue-500/20">
                            {node.group}
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                </div>

                {/* Right: Latency, Speed & Controls */}
                <div className="flex items-center gap-2.5" onClick={(e) => e.stopPropagation()}>
                  {/* Real Download Speed Badge */}
                  {node.speed_bps ? (
                    <span className="text-[11px] font-mono text-indigo-300 px-2 py-0.5 rounded bg-indigo-500/10 border border-indigo-500/20 flex items-center gap-1">
                      <Gauge className="w-3 h-3 text-indigo-400" />
                      <span>{formatSpeed(node.speed_bps)}</span>
                    </span>
                  ) : null}

                  {/* Latency badge */}
                  {getLatencyBadge(node)}

                  {/* Ping latency button */}
                  <button
                    onClick={() => testLatency(node.id)}
                    disabled={isTestingLat}
                    className="p-1.5 rounded-lg hover:bg-gray-800 text-gray-400 hover:text-blue-400 transition-colors disabled:opacity-50"
                    title="Ping latency (TCP Handshake)"
                  >
                    <Zap className={`w-3.5 h-3.5 ${isTestingLat ? 'animate-bounce text-blue-400' : ''}`} />
                  </button>

                  {/* Real speed test button */}
                  <button
                    onClick={() => testSpeed(node.id)}
                    disabled={isTestingSpd}
                    className="p-1.5 rounded-lg hover:bg-gray-800 text-gray-400 hover:text-indigo-400 transition-colors disabled:opacity-50"
                    title="Test real download bandwidth"
                  >
                    <Gauge className={`w-3.5 h-3.5 ${isTestingSpd ? 'animate-spin text-indigo-400' : ''}`} />
                  </button>

                  {/* Favorite toggle */}
                  <button
                    onClick={() => toggleFavorite(node.id)}
                    className="p-1.5 rounded-lg hover:bg-gray-800 transition-colors"
                    title={node.favorite ? 'Remove favorite' : 'Add favorite'}
                  >
                    <Star
                      className={`w-3.5 h-3.5 ${
                        node.favorite
                          ? 'fill-amber-400 text-amber-400'
                          : 'text-gray-500 hover:text-gray-300'
                      }`}
                    />
                  </button>

                  {/* Connect / Active Button */}
                  {isLiveConnected ? (
                    <div className="w-8 h-8 rounded-lg bg-emerald-500/20 text-emerald-400 flex items-center justify-center">
                      <Check className="w-4 h-4" />
                    </div>
                  ) : (
                    <button
                      onClick={() => {
                        setSelectedNodeId(node.id);
                        connect(node.id);
                      }}
                      className="px-3 py-1.5 rounded-lg bg-blue-600/80 hover:bg-blue-600 text-white text-xs font-medium transition-all"
                    >
                      Connect
                    </button>
                  )}

                  {/* Delete button */}
                  <button
                    onClick={() => deleteNode(node.id)}
                    className="p-1.5 rounded-lg hover:bg-red-500/20 text-gray-500 hover:text-red-400 transition-colors"
                    title="Delete node"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
