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
  // 启动测速选出的首选中转节点 id;从未实测过时为 null
  preferred_relay_id?: string | null;
}

export interface RelayRanking {
  preferred_id: string | null;
  preferred_name: string | null;
  latency_ms: number | null;
  speed_bps: number | null;
  tested: number;
  aborted: boolean;
}

// 真连接测活的进度:每拨完一个节点推送一次
export interface LivenessProgress {
  // 名单名:'VPNGate' | 'Residential'
  group: string;
  // 本轮已拨完的节点数,不会倒退
  tested: number;
  total: number;
  alive: number;
  running: boolean;
  done: boolean;
  // 提前结束(用户已建立真实隧道,或核心起不来),不等于剩下的节点都不可用
  aborted: boolean;
  // 这批结论是否已写回节点库;只有 true(或 running=false)时才重拉整份列表
  persisted: boolean;
  // 这一轮的结论或"为什么一个都没测成"。任何一条终止路径都必须带上它:
  // 满屏"未测"却没有一个字解释,和按钮失灵没有区别。
  message?: string | null;
  // 前端本地盖的时间戳(后端不发这个)。一轮在跑时每个节点都有一拍,45 秒没有
  // 新拍就说明这一轮已经没了 —— 按钮不能因为这条就永远灰着。
  at?: number;
}

// 单个节点"测活"的结论。message 永远是一句人话:这个入口存在的全部意义,就是让
// 用户知道自己刚才到底测没测、测出了什么。
export interface ProbeOutcome {
  // alive=可用;exit-dead=出口节点不可用;relay-dead=中转不可用(节点未判定);
  // not-judged=核心拒绝了这条 OpenVPN 配置(节点未判定)
  verdict: 'alive' | 'exit-dead' | 'relay-dead' | 'not-judged';
  message: string;
  // 判定过才有值;没判定时卡片保持"未测",不留下假结论。
  node?: UnifiedNode | null;
  // 同上,前端本地盖的时间戳,用来说"几点测的"。
  at?: number;
}


export interface SubscriptionTraffic {  upload: number;
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

