import { create } from 'zustand';
import { api } from '../services/api';
import { ActiveTab, AppSettings, ConnectionStatus, ProxyChain, Subscription, TrafficStats, UnifiedNode } from '../types';

interface AppStore {
  status: ConnectionStatus;
  connectedNode: UnifiedNode | null;
  traffic: TrafficStats;
  nodes: UnifiedNode[];
  subscriptions: Subscription[];
  chains: ProxyChain[];
  connectedChainId: string | null;
  selectedNodeId: string | null;
  settings: AppSettings;
  activeTab: ActiveTab;
  isTestingAll: boolean;
  searchQuery: string;
  selectedProtocol: string;
  selectedGroup: string;
  filterFavorite: boolean;
  sortBySpeed: boolean;
  sortMode: 'speed' | 'latency' | 'default';
  isTestingAllSpeed: boolean;
  testingLatencyIds: string[];
  testingSpeedIds: string[];
  testingChainIds: string[];
  errorMessage: string | null;

  // Actions
  init: () => Promise<void>;
  setActiveTab: (tab: ActiveTab) => void;
  setSearchQuery: (q: string) => void;
  setSelectedProtocol: (p: string) => void;
  setSelectedGroup: (g: string) => void;
  setFilterFavorite: (f: boolean) => void;
  setSortBySpeed: (s: boolean) => void;
  setSortMode: (mode: 'speed' | 'latency' | 'default') => void;
  setSelectedNodeId: (id: string | null) => void;
  setErrorMessage: (msg: string | null) => void;
  relayEnabled: boolean;
  selectedRelayNodeId: string;
  relayCandidates: UnifiedNode[];
  fetchRelayCandidates: () => Promise<void>;
  setRelayEnabled: (enabled: boolean) => void;
  setSelectedRelayNodeId: (id: string) => void;

  connect: (nodeId?: string, relayIdOverride?: string | null) => Promise<void>;
  disconnect: () => Promise<void>;
  refreshNodes: () => Promise<void>;
  refreshSubscriptions: () => Promise<void>;
  restoreDefaultSubscriptions: () => Promise<void>;
  updateAllSubscriptions: () => Promise<void>;
  addSubscription: (name: string, url: string) => Promise<boolean>;
  editSubscription: (id: string, name: string, url: string) => Promise<boolean>;
  deleteSubscription: (id: string) => Promise<void>;
  updateSubViaProxy: boolean;
  setUpdateSubViaProxy: (val: boolean) => void;
  updateSubscription: (id: string) => Promise<void>;
  testLatency: (id: string) => Promise<void>;
  testSpeed: (id: string) => Promise<void>;
  testAllNodes: () => Promise<void>;
  testAllSpeeds: () => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  deleteNode: (id: string) => Promise<void>;
  importLink: (link: string) => Promise<boolean>;
  saveSettings: (settings: AppSettings) => Promise<void>;

  // Chained Proxy Actions
  refreshChains: () => Promise<void>;
  addChain: (chain: { name: string; remarks?: string; node_ids: string[] }) => Promise<boolean>;
  updateChain: (chain: ProxyChain) => Promise<boolean>;
  deleteChain: (id: string) => Promise<void>;
  connectChain: (chainId: string) => Promise<void>;
  testChainLatency: (chainId: string) => Promise<void>;
  testAllChains: () => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  status: 'disconnected',
  connectedNode: null,
  traffic: {
    upload_bytes: 0,
    download_bytes: 0,
    upload_speed: 0,
    download_speed: 0,
    uptime_seconds: 0,
  },
  nodes: [],
  subscriptions: [],
  chains: [],
  connectedChainId: null,
  selectedNodeId: null,
  settings: {
    proxy_mode: 'system_proxy',
    mixed_port: 2080,
    clash_api_port: 9090,
    auto_connect: false,
    auto_start: false,
    kill_switch: false,
    dns_mode: 'auto',
    bypass_china: true,
    enable_ipv6: false,
    routing_mode: 'rule',
    custom_direct_rules: [],
    custom_proxy_rules: [],
  },
  activeTab: 'dashboard',
  isTestingAll: false,
  isTestingAllSpeed: false,
  searchQuery: '',
  selectedProtocol: 'all',
  selectedGroup: 'all',
  filterFavorite: false,
  sortBySpeed: false,
  sortMode: 'speed',
  testingLatencyIds: [],
  testingSpeedIds: [],
  testingChainIds: [],
  errorMessage: null,
  updateSubViaProxy: true,
  setUpdateSubViaProxy: (val) => set({ updateSubViaProxy: val }),
  relayEnabled: true,
  selectedRelayNodeId: 'auto',
  relayCandidates: [],
  setRelayEnabled: (enabled) => set({ relayEnabled: enabled }),
  setSelectedRelayNodeId: (id) => set({ selectedRelayNodeId: id }),
  fetchRelayCandidates: async () => {
    try {
      const relayCandidates = await api.getRelayCandidates();
      set({ relayCandidates });
    } catch (e) {
      console.error('Failed to fetch relay candidates:', e);
    }
  },

  init: async () => {
    try {
      const [status, connectedNode, nodes, subscriptions, settings, relayCandidates, chains, connectedChainId] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
        api.getNodes(),
        api.getSubscriptions(),
        api.getSettings(),
        api.getRelayCandidates().catch(() => []),
        api.getChains().catch(() => []),
        api.getConnectedChain().catch(() => null),
      ]);

      const selectedId = connectedNode ? connectedNode.id : (nodes.length > 0 ? nodes[0].id : null);

      set({
        status,
        connectedNode,
        nodes,
        subscriptions,
        settings,
        relayCandidates,
        chains,
        connectedChainId,
        selectedNodeId: selectedId,
      });

      // Automatically fetch nodes for Default sub on very first run
      if (subscriptions.length === 1 && subscriptions[0].name === 'Default' && subscriptions[0].node_count === 0) {
        get().updateAllSubscriptions().catch(console.error);
      }

      // Listen for background state events
      api.onStatusChanged((newStatus) => {
        set({ status: newStatus });
        if (newStatus === 'disconnected') {
          set({
            connectedNode: null,
            connectedChainId: null,
            traffic: {
              upload_bytes: 0,
              download_bytes: 0,
              upload_speed: 0,
              download_speed: 0,
              uptime_seconds: 0,
            },
          });
        }
      });

      api.onTrafficTick((stats) => {
        set({ traffic: stats });
      });
    } catch (e: any) {
      console.error('Init failed:', e);
      set({ errorMessage: String(e) });
    }
  },

  setActiveTab: (tab) => set({ activeTab: tab, errorMessage: null }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setSelectedProtocol: (selectedProtocol) => set({ selectedProtocol }),
  setSelectedGroup: (selectedGroup) => set({ selectedGroup }),
  setFilterFavorite: (filterFavorite) => set({ filterFavorite }),
  setSortBySpeed: (sortBySpeed) => set({ sortBySpeed, sortMode: sortBySpeed ? 'speed' : 'default' }),
  setSortMode: (sortMode) => set({ sortMode, sortBySpeed: sortMode === 'speed' }),
  setSelectedNodeId: (selectedNodeId) => set({ selectedNodeId }),
  setErrorMessage: (errorMessage) => set({ errorMessage }),

  connect: async (overrideId, relayIdOverride) => {
    const state = get();
    const targetId = overrideId || state.selectedNodeId;
    if (!targetId) {
      set({ errorMessage: 'Please select a server first.' });
      return;
    }

    const connectingNode = state.nodes.find((n) => n.id === targetId) || null;
    set({ status: 'connecting', selectedNodeId: targetId, connectedNode: connectingNode, connectedChainId: null, errorMessage: null });

    let relayParam: string | null = null;
    if (relayIdOverride !== undefined) {
      relayParam = relayIdOverride;
    } else if (state.relayEnabled) {
      relayParam = state.selectedRelayNodeId || 'auto';
    } else {
      relayParam = 'none';
    }

    try {
      await api.connect(targetId, relayParam);
      const connected = state.nodes.find((n) => n.id === targetId) || connectingNode;
      set({ status: 'connected', connectedNode: connected, selectedNodeId: targetId, connectedChainId: null });
    } catch (e: any) {
      console.error('Connect failed:', e);
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ status: 'error', errorMessage: errStr });
    }
  },

  disconnect: async () => {
    set({ status: 'disconnecting', errorMessage: null });
    try {
      await api.disconnect();
      set({ status: 'disconnected', connectedNode: null, connectedChainId: null });
    } catch (e: any) {
      console.error('Disconnect failed:', e);
      set({ status: 'disconnected', errorMessage: `Disconnect failed: ${e}`, connectedChainId: null });
    }
  },

  refreshNodes: async () => {
    try {
      const nodes = await api.getNodes();
      set({ nodes });
    } catch (e: any) {
      console.error('Refresh nodes failed:', e);
    }
  },

  refreshSubscriptions: async () => {
    try {
      const subscriptions = await api.getSubscriptions();
      set({ subscriptions });
    } catch (e: any) {
      console.error('Refresh subscriptions failed:', e);
    }
  },

  restoreDefaultSubscriptions: async () => {
    try {
      const subs = await api.restoreDefaultSubscriptions();
      set({ subscriptions: subs });
      // Trigger background update for any new idle subscriptions
      for (const sub of subs) {
        if (!sub.last_updated) {
          get().updateSubscription(sub.id);
        }
      }
    } catch (e: any) {
      set({ errorMessage: `Failed to load default subscriptions: ${e}` });
    }
  },

  updateAllSubscriptions: async () => {
    const subs = get().subscriptions;
    const useProxy = get().updateSubViaProxy;
    set((state) => ({
      subscriptions: state.subscriptions.map((s) => ({ ...s, status: 'updating', error_message: null })),
    }));
    try {
      const updated = await api.updateAllSubscriptions(useProxy);
      set({ subscriptions: updated });
      await get().refreshNodes();
    } catch (e: any) {
      console.error('Update all subscriptions batch failed, falling back:', e);
      for (const sub of subs) {
        get().updateSubscription(sub.id);
      }
    }
  },

  addSubscription: async (name, url) => {
    try {
      const sub = await api.addSubscription(name, url);
      set((state) => ({ subscriptions: [...state.subscriptions, sub] }));
      // Automatically trigger first update
      get().updateSubscription(sub.id);
      return true;
    } catch (e: any) {
      set({ errorMessage: `Failed to add subscription: ${e}` });
      return false;
    }
  },

  editSubscription: async (id, name, url) => {
    try {
      const updated = await api.editSubscription(id, name, url);
      set((state) => ({
        subscriptions: state.subscriptions.map((s) => (s.id === id ? { ...s, name: updated.name, url: updated.url } : s)),
      }));
      // Refresh nodes to reflect possible group rename
      await get().refreshNodes();
      return true;
    } catch (e: any) {
      set({ errorMessage: `Failed to edit subscription: ${e}` });
      return false;
    }
  },

  deleteSubscription: async (id) => {
    try {
      await api.deleteSubscription(id);
      set((state) => ({
        subscriptions: state.subscriptions.filter((s) => s.id !== id),
      }));
      // Refresh nodes since this subscription's nodes were removed
      await get().refreshNodes();
    } catch (e: any) {
      console.error('Delete subscription failed:', e);
    }
  },

  updateSubscription: async (id) => {
    set((state) => ({
      subscriptions: state.subscriptions.map((s) =>
        s.id === id ? { ...s, status: 'updating', error_message: null } : s
      ),
    }));

    try {
      const updatedSub = await api.updateSubscription(id, get().updateSubViaProxy);
      set((state) => ({
        subscriptions: state.subscriptions.map((s) => (s.id === id ? updatedSub : s)),
      }));
      // Refresh nodes list to reflect newly fetched nodes
      await get().refreshNodes();
    } catch (e: any) {
      set((state) => ({
        subscriptions: state.subscriptions.map((s) =>
          s.id === id ? { ...s, status: 'error', error_message: String(e) } : s
        ),
      }));
    }
  },

  testLatency: async (id) => {
    set((state) => ({
      testingLatencyIds: [...state.testingLatencyIds, id],
    }));

    try {
      const latency = await api.testNodeLatency(id);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                latency_ms: latency,
                status: latency > 0 ? 'alive' : 'dead',
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } catch (e: any) {
      console.error('Test latency failed:', e);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                latency_ms: -1,
                status: 'dead',
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } finally {
      set((state) => ({
        testingLatencyIds: state.testingLatencyIds.filter((tid) => tid !== id),
      }));
    }
  },

  testSpeed: async (id) => {
    set((state) => ({
      testingSpeedIds: [...state.testingSpeedIds, id],
    }));

    try {
      const speed = await api.testNodeSpeed(id);
      set((state) => ({
        nodes: state.nodes.map((n) =>
          n.id === id
            ? {
                ...n,
                speed_bps: speed,
                last_checked: Date.now() / 1000,
              }
            : n
        ),
      }));
    } catch (e: any) {
      console.error('Test speed failed:', e);
    } finally {
      set((state) => ({
        testingSpeedIds: state.testingSpeedIds.filter((sid) => sid !== id),
      }));
    }
  },

  testAllNodes: async () => {
    set({ isTestingAll: true });
    try {
      const updatedNodes = await api.testAllNodes();
      set({ nodes: updatedNodes });
    } catch (e: any) {
      console.error('Test all nodes failed:', e);
    } finally {
      set({ isTestingAll: false });
    }
  },

  testAllSpeeds: async () => {
    set({ isTestingAllSpeed: true });
    try {
      const updatedNodes = await api.testAllSpeeds();
      set({ nodes: updatedNodes });
    } catch (e: any) {
      console.error('Test all speeds failed:', e);
    } finally {
      set({ isTestingAllSpeed: false });
    }
  },

  toggleFavorite: async (id) => {
    try {
      const fav = await api.toggleFavorite(id);
      set((state) => ({
        nodes: state.nodes.map((n) => (n.id === id ? { ...n, favorite: fav } : n)),
      }));
    } catch (e: any) {
      console.error('Toggle favorite failed:', e);
    }
  },

  deleteNode: async (id) => {
    try {
      await api.deleteNode(id);
      set((state) => {
        const newNodes = state.nodes.filter((n) => n.id !== id);
        return {
          nodes: newNodes,
          selectedNodeId: state.selectedNodeId === id ? (newNodes[0]?.id || null) : state.selectedNodeId,
        };
      });
    } catch (e: any) {
      console.error('Delete node failed:', e);
    }
  },

  importLink: async (link) => {
    try {
      const node = await api.importShareLink(link);
      set((state) => ({
        nodes: [...state.nodes, node],
        selectedNodeId: node.id,
        activeTab: 'servers',
        errorMessage: null,
      }));
      return true;
    } catch (e: any) {
      set({ errorMessage: `Import failed: ${e}` });
      return false;
    }
  },

  saveSettings: async (settings) => {
    try {
      await api.saveSettings(settings);
      set({ settings });
    } catch (e: any) {
      set({ errorMessage: `Failed to save settings: ${e}` });
    }
  },

  refreshChains: async () => {
    try {
      const chains = await api.getChains();
      set({ chains });
    } catch (e: any) {
      console.error('Refresh chains failed:', e);
    }
  },

  addChain: async (chainData) => {
    try {
      const newChain = await api.addChain({
        id: '',
        name: chainData.name,
        remarks: chainData.remarks || '',
        node_ids: chainData.node_ids,
      });
      set((state) => ({ chains: [...state.chains, newChain] }));
      return true;
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `创建链式代理失败: ${errStr}` });
      return false;
    }
  },

  updateChain: async (chain) => {
    try {
      await api.updateChain(chain);
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chain.id ? chain : c)),
      }));
      return true;
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `更新链式代理失败: ${errStr}` });
      return false;
    }
  },

  deleteChain: async (id) => {
    try {
      await api.deleteChain(id);
      set((state) => ({
        chains: state.chains.filter((c) => c.id !== id),
        connectedChainId: state.connectedChainId === id ? null : state.connectedChainId,
      }));
    } catch (e: any) {
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({ errorMessage: `删除链式代理失败: ${errStr}` });
    }
  },

  connectChain: async (chainId) => {
    const state = get();
    if (state.connectedChainId === chainId && state.status === 'connected') {
      await state.disconnect();
      return;
    }

    set({ status: 'connecting', connectedChainId: chainId, errorMessage: null });
    try {
      await api.connectChain(chainId);
      const [status, connectedNode] = await Promise.all([
        api.getConnectionStatus(),
        api.getConnectedNode(),
      ]);
      set({ status, connectedNode, connectedChainId: chainId });
    } catch (e: any) {
      console.error('Chain connect failed:', e);
      const errStr = typeof e === 'string' ? e : (e?.message || JSON.stringify(e));
      set({
        status: 'error',
        connectedNode: null,
        connectedChainId: null,
        errorMessage: `链式代理连接失败: ${errStr}`,
      });
    }
  },

  testChainLatency: async (chainId) => {
    set((state) => ({
      testingChainIds: [...state.testingChainIds, chainId],
    }));
    try {
      const latency = await api.testChainLatency(chainId);
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chainId ? { ...c, latency_ms: latency } : c)),
        testingChainIds: state.testingChainIds.filter((id) => id !== chainId),
      }));
    } catch (e) {
      set((state) => ({
        chains: state.chains.map((c) => (c.id === chainId ? { ...c, latency_ms: -1 } : c)),
        testingChainIds: state.testingChainIds.filter((id) => id !== chainId),
      }));
    }
  },

  testAllChains: async () => {
    const chains = get().chains;
    for (const chain of chains) {
      get().testChainLatency(chain.id);
    }
  },
}));
