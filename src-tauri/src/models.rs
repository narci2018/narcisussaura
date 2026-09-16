use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProtocolType {
    Vless,
    Vmess,
    Trojan,
    Shadowsocks,
    Socks5,
    Http,
    Wireguard,
    Hysteria2,
    Masque,
    Openvpn,
    Psiphon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Unknown,
    Alive,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedNode {
    pub id: String,
    pub name: String,
    pub protocol: ProtocolType,
    pub address: String,
    pub port: u16,

    #[serde(default)]
    pub country_code: String,
    #[serde(default)]
    pub country_name: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,

    #[serde(default)]
    pub latency_ms: Option<i64>,
    #[serde(default)]
    pub speed_bps: Option<u64>,
    #[serde(default)]
    pub last_checked: Option<i64>,
    #[serde(default = "default_status")]
    pub status: NodeStatus,

    /// Protocol specific parameters (e.g. uuid, password, method, reality public key, sni, etc.)
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_status() -> NodeStatus {
    NodeStatus::Unknown
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStats {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub upload_speed: u64,   // bytes per second
    pub download_speed: u64, // bytes per second
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    SystemProxy,
    TunMode,
    ProxyOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub proxy_mode: ProxyMode,
    pub mixed_port: u16,
    pub clash_api_port: u16,
    pub auto_connect: bool,
    pub auto_start: bool,
    pub kill_switch: bool,
    pub dns_mode: String,
    #[serde(default = "default_true")]
    pub bypass_china: bool,
    #[serde(default)]
    pub enable_ipv6: bool,
    #[serde(default = "default_routing_mode")]
    pub routing_mode: String, // "rule" | "global" | "direct"
    #[serde(default)]
    pub custom_direct_rules: Vec<String>,
    #[serde(default)]
    pub custom_proxy_rules: Vec<String>,
    #[serde(default = "default_direct_geos")]
    pub direct_geo_rules: Vec<String>,
    #[serde(default = "default_block_geos")]
    pub block_geo_rules: Vec<String>,
    #[serde(default)]
    pub proxy_geo_rules: Vec<String>,
    #[serde(default)]
    pub block_udp_443: bool,
    #[serde(default = "default_active_rule_set_id")]
    pub active_rule_set_id: String,
    #[serde(default = "default_rule_sets")]
    pub rule_sets: Vec<RoutingRuleSet>,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "dark".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRuleSet {
    pub id: String,
    pub name: String,
    #[serde(default = "default_direct_geos")]
    pub direct_rules: Vec<String>,
    #[serde(default)]
    pub proxy_rules: Vec<String>,
    #[serde(default = "default_block_geos")]
    pub block_rules: Vec<String>,
}

fn default_rule_sets() -> Vec<RoutingRuleSet> {
    vec![RoutingRuleSet {
        id: "default".to_string(),
        name: "默认".to_string(),
        direct_rules: default_direct_geos(),
        proxy_rules: Vec::new(),
        block_rules: default_block_geos(),
    }]
}

fn default_active_rule_set_id() -> String {
    "default".to_string()
}

fn default_true() -> bool {
    true
}

fn default_routing_mode() -> String {
    "rule".to_string()
}

fn default_direct_geos() -> Vec<String> {
    vec![
        "geosite:cn".to_string(),
        "geoip:cn".to_string(),
        "geosite:private".to_string(),
        "geoip:private".to_string(),
    ]
}

fn default_block_geos() -> Vec<String> {
    vec!["geosite:category-ads-all".to_string()]
}

impl AppSettings {
    pub fn get_active_rule_set(&self) -> RoutingRuleSet {
        if let Some(set) = self.rule_sets.iter().find(|s| s.id == self.active_rule_set_id) {
            return set.clone();
        }
        if let Some(set) = self.rule_sets.first() {
            return set.clone();
        }
        RoutingRuleSet {
            id: "default".to_string(),
            name: "默认".to_string(),
            direct_rules: default_direct_geos(),
            proxy_rules: Vec::new(),
            block_rules: default_block_geos(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            proxy_mode: ProxyMode::SystemProxy,
            mixed_port: 2080,
            clash_api_port: 9090,
            auto_connect: false,
            auto_start: false,
            kill_switch: false,
            dns_mode: "auto".to_string(),
            bypass_china: true,
            enable_ipv6: false,
            routing_mode: "rule".to_string(),
            custom_direct_rules: Vec::new(),
            custom_proxy_rules: Vec::new(),
            direct_geo_rules: default_direct_geos(),
            block_geo_rules: default_block_geos(),
            proxy_geo_rules: Vec::new(),
            block_udp_443: false,
            active_rule_set_id: "default".to_string(),
            rule_sets: default_rule_sets(),
            theme: "dark".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionTraffic {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default = "default_interval")]
    pub update_interval_minutes: u32,
    #[serde(default)]
    pub last_updated: Option<i64>,
    #[serde(default)]
    pub node_count: usize,
    #[serde(default = "default_sub_status")]
    pub status: String,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub traffic: Option<SubscriptionTraffic>,
}

fn default_interval() -> u32 {
    1440
}

fn default_sub_status() -> String {
    "idle".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyChain {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub remarks: String,
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
    #[serde(default)]
    pub created_at: i64,
}
