export type ProtocolType =
  | 'vless'
  | 'vmess'
  | 'trojan'
  | 'shadowsocks'
  | 'socks5'
  | 'http'
  | 'wireguard'
  | 'hysteria2'
  | 'masque'
  | 'openvpn'
  | 'psiphon';

export type NodeStatus = 'unknown' | 'alive' | 'dead';

export interface UnifiedNode {
  id: string;
  name: string;
  protocol: ProtocolType;
  address: string;
  port: number;
  country_code: string;
  country_name: string;
  city: string;
  group: string;
  tags: string[];
  favorite: boolean;
  latency_ms?: number | null;
  speed_bps?: number | null;
  last_checked?: number | null;
  status: NodeStatus;
  config: Record<string, any>;
}

export type ConnectionStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'disconnecting'
  | 'error';

export type ActiveTab =
  | 'dashboard'
  | 'servers'
  | 'chains'
  | 'psiphon'
  | 'vpngate'
  | 'residential'
  | 'megav'
  | 'subscriptions'
  | 'import'
  | 'settings';

export interface ProxyChain {
  id: string;
  name: string;
  remarks?: string;
  node_ids: string[];
  latency_ms?: number | null;
  created_at?: number;
}

export interface TrafficStats {
  upload_bytes: number;
  download_bytes: number;
  upload_speed: number;
  download_speed: number;
  uptime_seconds: number;
}

export type ProxyMode = 'system_proxy' | 'tun_mode' | 'proxy_only';

export interface RoutingRuleSet {
  id: string;
  name: string;
  direct_rules: string[];
  proxy_rules: string[];
  block_rules: string[];
}

export interface AppSettings {
  proxy_mode: ProxyMode;
  mixed_port: number;
  clash_api_port: number;
  auto_connect: boolean;
  auto_start: boolean;
  kill_switch: boolean;
  dns_mode: string;
  bypass_china: boolean;
  enable_ipv6: boolean;
  routing_mode: string; // 'rule' | 'global' | 'direct'
  custom_direct_rules?: string[];
  custom_proxy_rules?: string[];
  direct_geo_rules?: string[];
  block_geo_rules?: string[];
  proxy_geo_rules?: string[];
  block_udp_443?: boolean;
  active_rule_set_id?: string;
  rule_sets?: RoutingRuleSet[];
  theme?: 'dark' | 'light';
}

export interface SubscriptionTraffic {
  upload: number;
  download: number;
  total: number;
  expire?: number;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  update_interval_minutes: number;
  last_updated?: number | null;
  node_count: number;
  status: 'idle' | 'updating' | 'success' | 'error';
  error_message?: string | null;
  traffic?: SubscriptionTraffic | null;
}

export interface RelaySettings {
  enabled: boolean;
  relayNodeId: string; // 'auto' | 'none' | specific node id
}

