use crate::managers::node_manager::NodeManager;
use crate::models::{NodeStatus, ProtocolType, Subscription, SubscriptionTraffic, UnifiedNode};
use anyhow::{bail, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL_SAFE;
use base64::Engine;
use parking_lot::RwLock;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SubscriptionManager {
    subscriptions: Arc<RwLock<Vec<Subscription>>>,
    data_path: PathBuf,
    node_manager: NodeManager,
}

impl SubscriptionManager {
    pub fn default_subscriptions() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Default", "https://cdn.jsdelivr.net/gh/narci2018/freesubplus@main/output/v2ray.txt"),
        ]
    }

    pub fn new(app_data_dir: &Path, node_manager: NodeManager) -> Self {
        let data_path = app_data_dir.join("subscriptions.json");
        let mut initial_subs = Vec::new();

        if data_path.exists() {
            if let Ok(content) = fs::read_to_string(&data_path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<Subscription>>(&content) {
                    initial_subs = loaded;
                }
            }
        }

        // If no subscriptions exist, pre-populate default free subscription sources
        if initial_subs.is_empty() {
            for (name, url) in Self::default_subscriptions() {
                initial_subs.push(Subscription {
                    id: Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    url: url.to_string(),
                    update_interval_minutes: 1440,
                    last_updated: None,
                    node_count: 0,
                    status: "idle".to_string(),
                    error_message: None,
                    traffic: None,
                });
            }
        } else {
            // Ensure WARP subscription exists
            for (name, url) in Self::default_subscriptions() {
                if !initial_subs.iter().any(|s| s.url == url) {
                    initial_subs.push(Subscription {
                        id: Uuid::new_v4().to_string(),
                        name: name.to_string(),
                        url: url.to_string(),
                        update_interval_minutes: 1440,
                        last_updated: None,
                        node_count: 0,
                        status: "idle".to_string(),
                        error_message: None,
                        traffic: None,
                    });
                }
            }
        }

        let mgr = Self {
            subscriptions: Arc::new(RwLock::new(initial_subs)),
            data_path,
            node_manager,
        };

        // Clean up old default subscriptions from earlier versions
        let old_default_urls = vec![
            "https://raw.githubusercontent.com/byJoey/warp-masque-actions/main/configs/opera-masque.yaml",
            "https://raw.githubusercontent.com/morpheusadam/v2ray-config/main/subs/bundles/best.txt",
            "https://raw.githubusercontent.com/Au1rxx/free-vpn-subscriptions/main/output/clash.yaml",
            "https://raw.githubusercontent.com/ssrsub/ssr/master/clash.yaml",
            "https://raw.githubusercontent.com/sunmiao4458/free-proxy-airport/main/output/clash.yaml",
            "https://raw.githubusercontent.com/snakem982/proxypool/main/source/clash-meta-2.yaml",
            "https://raw.githubusercontent.com/ermaozi/get_subscribe/main/subscribe/clash.yml",
            "https://raw.githubusercontent.com/ts-sf/fly/main/clash",
            "https://raw.githubusercontent.com/zhuhaiuk/free-nodes/main/clash_config.yaml",
            "https://raw.githubusercontent.com/xyfqzy/free-nodes/main/docs/subscriptions/base64.txt",
        ];
        
        let to_delete: Vec<String> = {
            let lock = mgr.subscriptions.read();
            lock.iter().filter(|s| old_default_urls.contains(&s.url.as_str())).map(|s| s.id.clone()).collect()
        };
        for id in to_delete {
            let _ = mgr.delete_subscription(&id);
        }

        let _ = mgr.save();
        mgr
    }

    pub fn restore_default_subscriptions(&self) -> Result<Vec<Subscription>, String> {
        // First delete all existing subscriptions to clear old free sources
        let all_ids: Vec<String> = {
            let lock = self.subscriptions.read();
            lock.iter().map(|s| s.id.clone()).collect()
        };
        for id in all_ids {
            let _ = self.delete_subscription(&id);
        }

        let mut lock = self.subscriptions.write();
        let defaults = Self::default_subscriptions();
        
        for (name, url) in defaults {
            if !lock.iter().any(|s| s.url == url) {
                lock.push(Subscription {
                    id: Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    url: url.to_string(),
                    update_interval_minutes: 1440,
                    last_updated: None,
                    node_count: 0,
                    status: "idle".to_string(),
                    error_message: None,
                    traffic: None,
                });
            }
        }
        let list = lock.clone();
        drop(lock);
        self.save()?;
        Ok(list)
    }

    pub fn get_all(&self) -> Vec<Subscription> {
        self.subscriptions.read().clone()
    }

    pub fn add_subscription(&self, name: String, url: String) -> Result<Subscription, String> {
        let sub = Subscription {
            id: Uuid::new_v4().to_string(),
            name,
            url,
            update_interval_minutes: 1440,
            last_updated: None,
            node_count: 0,
            status: "idle".to_string(),
            error_message: None,
            traffic: None,
        };

        let mut lock = self.subscriptions.write();
        lock.push(sub.clone());
        drop(lock);
        self.save()?;
        Ok(sub)
    }

    pub fn edit_subscription(&self, id: &str, name: String, url: String) -> Result<Subscription, String> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Subscription URL must start with http:// or https://".to_string());
        }
        let old_name;
        {
            let lock = self.subscriptions.read();
            let sub = lock.iter().find(|s| s.id == id).ok_or_else(|| "Subscription not found".to_string())?;
            old_name = sub.name.clone();
        }
        // Rename nodes that belong to the old group name
        if old_name != name {
            let all_nodes = self.node_manager.get_all();
            for mut node in all_nodes {
                if node.group == old_name {
                    node.group = name.clone();
                    let _ = self.node_manager.update_node(node);
                }
            }
        }
        let updated;
        {
            let mut lock = self.subscriptions.write();
            let sub = lock.iter_mut().find(|s| s.id == id).ok_or_else(|| "Subscription not found".to_string())?;
            sub.name = name;
            sub.url = url;
            updated = sub.clone();
        }
        self.save()?;
        Ok(updated)
    }

    pub fn delete_subscription(&self, id: &str) -> Result<(), String> {
        let mut lock = self.subscriptions.write();
        let sub_name = lock.iter().find(|s| s.id == id).map(|s| s.name.clone());
        lock.retain(|s| s.id != id);
        drop(lock);
        self.save()?;

        // Clean up nodes associated with this subscription group
        if let Some(group) = sub_name {
            let all_nodes = self.node_manager.get_all();
            for node in all_nodes {
                if node.group == group {
                    let _ = self.node_manager.delete_node(&node.id);
                }
            }
        }

        Ok(())
    }

    pub async fn update_subscription(&self, id: &str, proxy_url: Option<&str>) -> Result<Subscription, String> {
        let sub = {
            let lock = self.subscriptions.read();
            lock.iter().find(|s| s.id == id).cloned()
        };

        let mut sub = sub.ok_or_else(|| "Subscription not found".to_string())?;
        sub.status = "updating".to_string();
        self.update_sub_in_list(&sub);

        match self.fetch_and_parse(&sub, proxy_url).await {
            Ok((nodes, traffic)) => {
                sub.node_count = nodes.len();
                sub.last_updated = Some(chrono::Utc::now().timestamp());
                sub.status = "success".to_string();
                sub.error_message = None;
                sub.traffic = traffic;

                // Sync nodes into NodeManager: remove previous nodes with this subscription group, and insert new ones
                let existing = self.node_manager.get_all();
                for ex in existing {
                    if ex.group == sub.name {
                        let _ = self.node_manager.delete_node(&ex.id);
                    }
                }
                for node in nodes {
                    let _ = self.node_manager.add_node(node);
                }

                self.update_sub_in_list(&sub);
                self.save()?;
                Ok(sub)
            }
            Err(e) => {
                let err_str = e.to_string();
                sub.status = "error".to_string();
                sub.error_message = Some(err_str.clone());
                self.update_sub_in_list(&sub);
                self.save()?;
                Err(err_str)
            }
        }
    }

    pub async fn update_all_subscriptions(&self, proxy_url: Option<&str>) -> Result<Vec<Subscription>, String> {
        let ids: Vec<String> = self.subscriptions.read().iter().map(|s| s.id.clone()).collect();
        for id in ids {
            let _ = self.update_subscription(&id, proxy_url).await;
        }
        Ok(self.get_all())
    }

    fn update_sub_in_list(&self, sub: &Subscription) {
        let mut lock = self.subscriptions.write();
        if let Some(item) = lock.iter_mut().find(|s| s.id == sub.id) {
            *item = sub.clone();
        }
    }

    async fn fetch_and_parse(&self, sub: &Subscription, proxy_url: Option<&str>) -> Result<(Vec<UnifiedNode>, Option<SubscriptionTraffic>)> {
        // Collect URLs to attempt: primary URL first, and if from GitHub, fallback mirrors to bypass GFW blocks
        let mut urls_to_try = vec![sub.url.clone()];
        if sub.url.contains("raw.githubusercontent.com") || sub.url.contains("github.com") {
            urls_to_try.push(format!("https://ghproxy.net/{}", sub.url));
            urls_to_try.push(format!("https://ghfast.top/{}", sub.url));
        }

        let mut last_err = anyhow::anyhow!("No candidate URL attempted");

        for target_url in &urls_to_try {
            let mut client_builder = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12));

            if let Some(p) = proxy_url {
                if let Ok(proxy) = reqwest::Proxy::all(p) {
                    client_builder = client_builder.proxy(proxy);
                }
            }

            let client = match client_builder.build() {
                Ok(c) => c,
                Err(e) => {
                    last_err = e.into();
                    continue;
                }
            };

            let resp = match client
                .get(target_url)
                .header("User-Agent", "clash.meta/1.18.0 NarcissusAura/1.0.0")
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_err = e.into();
                    continue;
                }
            };

            if !resp.status().is_success() {
                last_err = anyhow::anyhow!("HTTP request failed with status: {}", resp.status());
                continue;
            }

            let traffic = if let Some(header_val) = resp.headers().get("subscription-userinfo") {
                if let Ok(val_str) = header_val.to_str() {
                    Self::parse_userinfo(val_str)
                } else {
                    None
                }
            } else {
                None
            };

            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    last_err = e.into();
                    continue;
                }
            };

            let nodes = self.parse_subscription_body(&body, &sub.name)?;
            return Ok((nodes, traffic));
        }

        Err(last_err)
    }

    fn parse_userinfo(info: &str) -> Option<SubscriptionTraffic> {
        let mut upload = 0;
        let mut download = 0;
        let mut total = 0;
        let mut expire = None;

        for part in info.split(';') {
            let mut kv = part.split('=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                let k = k.trim();
                let v = v.trim();
                match k {
                    "upload" => upload = v.parse().unwrap_or(0),
                    "download" => download = v.parse().unwrap_or(0),
                    "total" => total = v.parse().unwrap_or(0),
                    "expire" => expire = v.parse().ok(),
                    _ => {}
                }
            }
        }

        Some(SubscriptionTraffic {
            upload,
            download,
            total,
            expire,
        })
    }

    fn parse_subscription_body(&self, body: &str, group_name: &str) -> Result<Vec<UnifiedNode>> {
        let body = body.trim();
        if body.is_empty() {
            bail!("Subscription response body is empty");
        }

        // 1. Try Clash YAML
        if let Ok(nodes) = self.parse_clash_yaml(body, group_name) {
            if !nodes.is_empty() {
                return Ok(nodes);
            }
        }

        // 2. Try Base64 decode
        if let Ok(decoded) = BASE64_STANDARD.decode(body.as_bytes())
            .or_else(|_| BASE64_URL_SAFE.decode(body.as_bytes()))
        {
            if let Ok(decoded_str) = String::from_utf8(decoded) {
                let nodes = self.parse_links_text(&decoded_str, group_name);
                if !nodes.is_empty() {
                    return Ok(nodes);
                }
            }
        }

        // 3. Try plaintext multi-line share links
        let nodes = self.parse_links_text(body, group_name);
        if !nodes.is_empty() {
            return Ok(nodes);
        }

        bail!("Could not detect recognized subscription format (Clash YAML or Base64 links)")
    }

    fn parse_clash_yaml(&self, body: &str, group_name: &str) -> Result<Vec<UnifiedNode>> {
        let val: serde_yaml::Value = serde_yaml::from_str(body)?;
        let proxies = match val.get("proxies").and_then(|p| p.as_sequence()) {
            Some(seq) => seq,
            None => return Ok(vec![]),
        };

        let mut nodes = Vec::new();
        for p in proxies {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("Proxy");
            let ptype = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let server = p.get("server").and_then(|v| v.as_str()).unwrap_or("");
            let port = p.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;

            if server.is_empty() || port == 0 {
                continue;
            }

            let sni = p.get("servername")
                .and_then(|v| v.as_str())
                .or_else(|| p.get("sni").and_then(|v| v.as_str()));
            let insecure = p.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let network = p.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");

            let (ws_path, ws_host) = if let Some(opts) = p.get("ws-opts") {
                let path = opts.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                let host = opts.get("headers")
                    .and_then(|h| h.get("Host").or_else(|| h.get("host")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                (path, host)
            } else {
                ("/", "")
            };

            let (protocol, config) = match ptype.to_lowercase().as_str() {
                "vless" => {
                    let uuid = p.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
                    let flow = p.get("flow").and_then(|v| v.as_str());
                    let reality_opts = p.get("reality-opts");
                    let mut security = "none";
                    let mut public_key = None;
                    let mut short_id = None;

                    if let Some(r) = reality_opts {
                        security = "reality";
                        public_key = r.get("public-key").and_then(|v| v.as_str());
                        short_id = r.get("short-id").and_then(|v| v.as_str());
                    } else if p.get("tls").and_then(|v| v.as_bool()).unwrap_or(false) || sni.is_some() {
                        security = "tls";
                    }

                    (
                        ProtocolType::Vless,
                        json!({
                            "uuid": uuid,
                            "flow": flow,
                            "security": security,
                            "sni": sni,
                            "public_key": public_key,
                            "short_id": short_id,
                            "network": network,
                            "path": ws_path,
                            "host": ws_host,
                            "insecure": insecure
                        }),
                    )
                }
                "ss" | "shadowsocks" => {
                    let cipher = p.get("cipher").and_then(|v| v.as_str()).unwrap_or("");
                    let password = p.get("password").and_then(|v| v.as_str()).unwrap_or("");
                    (
                        ProtocolType::Shadowsocks,
                        json!({
                            "method": cipher,
                            "password": password
                        }),
                    )
                }
                "trojan" => {
                    let password = p.get("password").and_then(|v| v.as_str()).unwrap_or("");
                    (
                        ProtocolType::Trojan,
                        json!({
                            "password": password,
                            "sni": sni,
                            "network": network,
                            "path": ws_path,
                            "host": ws_host,
                            "insecure": insecure
                        }),
                    )
                }
                "vmess" => {
                    let uuid = p.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
                    let alter_id = p.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cipher = p.get("cipher").and_then(|v| v.as_str()).unwrap_or("auto");
                    let tls = p.get("tls").and_then(|v| v.as_bool()).unwrap_or(false);
                    (
                        ProtocolType::Vmess,
                        json!({
                            "uuid": uuid,
                            "alter_id": alter_id,
                            "cipher": cipher,
                            "tls": tls,
                            "sni": sni,
                            "network": network,
                            "path": ws_path,
                            "host": ws_host,
                            "insecure": insecure
                        }),
                    )
                }
                "socks5" | "socks" => (
                    ProtocolType::Socks5,
                    json!({
                        "username": p.get("username").and_then(|v| v.as_str()),
                        "password": p.get("password").and_then(|v| v.as_str())
                    }),
                ),
                "wireguard" => {
                    let private_key = p.get("private-key").and_then(|v| v.as_str()).unwrap_or("aGVsbG93b3JsZGhlbGxvd29ybGRoZWxsb3dvcmxkMTE=");
                    let public_key = p.get("public-key").and_then(|v| v.as_str())
                        .unwrap_or("bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=");
                    let ip = p.get("ip").and_then(|v| v.as_str()).unwrap_or("172.16.0.2");
                    let ipv6 = p.get("ipv6").and_then(|v| v.as_str());
                    let reserved = p.get("reserved").and_then(|v| v.as_sequence()).map(|seq| {
                        seq.iter().filter_map(|x| x.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
                    }).unwrap_or_else(|| vec![0, 0, 0]);
                    let mtu = p.get("mtu").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;

                    (
                        ProtocolType::Wireguard,
                        json!({
                            "private_key": private_key,
                            "public_key": public_key,
                            "ip": ip,
                            "ipv6": ipv6,
                            "reserved": reserved,
                            "mtu": mtu,
                        }),
                    )
                }
                "masque" | "warp-masque" | "h3" => {
                    (
                        ProtocolType::Masque,
                        json!({
                            "sni": sni.unwrap_or("zt-masque.cloudflareclient.com"),
                            "network": "quic",
                            "mtu": 1280
                        }),
                    )
                }
                _ => continue,
            };

            // Detect Country Code from Node Name
            let name_upper = name.to_uppercase();
            let country_code = if name.contains("香港") || name_upper.contains("HK") || name_upper.contains("HONG KONG") {
                "HK".to_string()
            } else if name.contains("台湾") || name.contains("台灣") || name_upper.contains("TW") || name_upper.contains("TAIWAN") {
                "TW".to_string()
            } else if name.contains("日本") || name_upper.contains("JP") || name_upper.contains("JAPAN") || name_upper.contains("TOKYO") {
                "JP".to_string()
            } else if name.contains("美国") || name.contains("美國") || name_upper.contains("US") || name_upper.contains("UNITED STATES") || name_upper.contains("USA") {
                "US".to_string()
            } else if name.contains("新加坡") || name.contains("狮城") || name_upper.contains("SG") || name_upper.contains("SINGAPORE") {
                "SG".to_string()
            } else if name.contains("韩国") || name_upper.contains("KR") || name_upper.contains("KOREA") {
                "KR".to_string()
            } else if name.contains("英国") || name_upper.contains("UK") || name_upper.contains("GB") || name_upper.contains("BRITAIN") {
                "GB".to_string()
            } else if name.contains("德国") || name_upper.contains("DE") || name_upper.contains("GERMANY") {
                "DE".to_string()
            } else {
                "".to_string()
            };

            nodes.push(UnifiedNode {
                id: Uuid::new_v4().to_string(),
                name: name.to_string(),
                protocol,
                address: server.to_string(),
                port,
                country_code,
                country_name: "".to_string(),
                city: "".to_string(),
                group: group_name.to_string(),
                tags: vec![group_name.to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config,
            });
        }

        Ok(nodes)
    }

    fn parse_links_text(&self, text: &str, group_name: &str) -> Vec<UnifiedNode> {
        let mut nodes = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(mut node) = self.node_manager.parse_share_link(line) {
                node.group = group_name.to_string();
                nodes.push(node);
            }
        }
        nodes
    }

    pub fn save(&self) -> Result<(), String> {
        let list = self.subscriptions.read().clone();
        let json_data = serde_json::to_string_pretty(&list)
            .map_err(|e| format!("Failed to serialize subscriptions: {}", e))?;
        if let Some(parent) = self.data_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.data_path, json_data)
            .map_err(|e| format!("Failed to write subscriptions.json: {}", e))?;
        Ok(())
    }
}
