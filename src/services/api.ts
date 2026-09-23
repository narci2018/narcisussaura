import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { AppSettings, ConnectionStatus, ProxyChain, Subscription, TrafficStats, UnifiedNode } from '../types';

export const api = {
  getConnectionStatus: () => invoke<ConnectionStatus>('get_connection_status'),
  getConnectedNode: () => invoke<UnifiedNode | null>('get_connected_node'),
  connect: (nodeId: string, relayNodeId?: string | null) => invoke<void>('connect', { nodeId, relayNodeId: relayNodeId ?? null }),
  getRelayCandidates: () => invoke<UnifiedNode[]>('get_relay_candidates'),
  disconnect: () => invoke<void>('disconnect'),
  
  getNodes: () => invoke<UnifiedNode[]>('get_nodes'),
  addNode: (node: UnifiedNode) => invoke<UnifiedNode>('add_node', { node }),
  updateNode: (node: UnifiedNode) => invoke<void>('update_node', { node }),
  deleteNode: (id: string) => invoke<void>('delete_node', { id }),
  toggleFavorite: (id: string) => invoke<boolean>('toggle_favorite', { id }),
  importShareLink: (link: string) => invoke<UnifiedNode>('import_share_link', { link }),
  
  testNodeLatency: (id: string) => invoke<number>('test_node_latency', { id }),
  testNodeSpeed: (id: string) => invoke<number | null>('test_node_speed', { id }),
  testAllNodes: () => invoke<UnifiedNode[]>('test_all_nodes'),
  testAllSpeeds: () => invoke<UnifiedNode[]>('test_all_speeds'),

  // Chained Proxy
  getChains: () => invoke<ProxyChain[]>('get_chains'),
  addChain: (chain: ProxyChain) => invoke<ProxyChain>('add_chain', { chain }),
  updateChain: (chain: ProxyChain) => invoke<void>('update_chain', { chain }),
  deleteChain: (id: string) => invoke<void>('delete_chain', { id }),
  getConnectedChain: () => invoke<string | null>('get_connected_chain'),
  connectChain: (chainId: string) => invoke<void>('connect_chain', { chainId }),
  connectSmartGroup: (nodeIds: string[]) => invoke<void>('connect_smart_group', { nodeIds }),
  testChainLatency: (chainId: string) => invoke<number>('test_chain_latency', { chainId }),
  
  getSubscriptions: () => invoke<Subscription[]>('get_subscriptions'),
  restoreDefaultSubscriptions: () => invoke<Subscription[]>('restore_default_subscriptions'),
  addSubscription: (name: string, url: string) => invoke<Subscription>('add_subscription', { name, url }),
  editSubscription: (id: string, name: string, url: string) => invoke<Subscription>('edit_subscription', { id, name, url }),
  deleteSubscription: (id: string) => invoke<void>('delete_subscription', { id }),
  updateSubscription: (id: string, useProxy?: boolean) => invoke<Subscription>('update_subscription', { id, useProxy }),
  updateAllSubscriptions: (useProxy?: boolean) => invoke<Subscription[]>('update_all_subscriptions', { useProxy }),

  fetchMegaVNodes: () => invoke<UnifiedNode[]>('fetch_megav_nodes'),
  fetchVPNGateNodes: () => invoke<UnifiedNode[]>('fetch_vpngate_nodes'),
  fetchPsiphonNodes: () => invoke<UnifiedNode[]>('fetch_psiphon_nodes'),
  fetchResidentialNodes: (url?: string) => invoke<UnifiedNode[]>('fetch_residential_nodes', { url: url || null }),
  
  getMachineId: () => invoke<string>('get_machine_id'),
  requestAuth: (machineId: string) => invoke<string>('request_auth', { machineId }),

  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) => invoke<void>('save_settings', { newSettings: settings }),
  
  minimizeWindow: () => invoke<void>('minimize_window'),
  toggleMaximize: () => invoke<boolean>('toggle_maximize'),
  isWindowMaximized: () => invoke<boolean>('is_window_maximized'),
  closeWindow: () => invoke<void>('close_window'),
  
  onStatusChanged: (callback: (status: ConnectionStatus) => void): Promise<UnlistenFn> => {
    return listen<ConnectionStatus>('core:status-changed', (event) => callback(event.payload));
  },
  
  onTrafficTick: (callback: (stats: TrafficStats) => void): Promise<UnlistenFn> => {
    return listen<TrafficStats>('core:traffic-tick', (event) => callback(event.payload));
  },

  onVpnStage: (callback: (stage: string) => void): Promise<UnlistenFn> => {
    return listen<string>('core:vpn-stage', (event) => callback(event.payload));
  },

  getCrashReport: () => invoke<string | null>('get_crash_report'),
  getFullLogs: () => invoke<string>('get_full_logs'),
};
