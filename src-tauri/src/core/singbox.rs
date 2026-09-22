use crate::core::adapter::CoreAdapter;
use crate::models::{AppSettings, ProtocolType, ProxyMode, UnifiedNode};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct SingBoxAdapter;

impl SingBoxAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn build_outbound(&self, node: &UnifiedNode) -> Result<Value> {
        let conf = &node.config;
        match node.protocol {
            ProtocolType::Vless => {
                let uuid = conf.get("uuid").and_then(|v| v.as_str()).unwrap_or_default();
                if uuid.is_empty() {
                    bail!("VLESS node requires 'uuid'");
                }

                let mut outbound = json!({
                    "type": "vless",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port,
                    "uuid": uuid
                });

                if let Some(flow) = conf.get("flow").and_then(|v| v.as_str()) {
                    if !flow.is_empty() {
                        outbound["flow"] = json!(flow);
                    }
                }

                let security = conf.get("security").and_then(|v| v.as_str()).unwrap_or("none");
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or("");
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");

                if security == "reality" {
                    let public_key = conf.get("public_key").and_then(|v| v.as_str()).unwrap_or_default();
                    let short_id = conf.get("short_id").and_then(|v| v.as_str()).unwrap_or_default();

                    outbound["tls"] = json!({
                        "enabled": true,
                        "server_name": sni,
                        "reality": {
                            "enabled": true,
                            "public_key": public_key,
                            "short_id": short_id
                        },
                        "utls": {
                            "enabled": true,
                            "fingerprint": conf.get("fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome")
                        }
                    });
                } else if security == "tls" || conf.get("tls").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let server_name = if !sni.is_empty() { sni } else { &node.address };
                    let mut tls_obj = json!({
                        "enabled": true,
                        "server_name": server_name,
                        "insecure": conf.get("insecure").and_then(|v| v.as_bool()).unwrap_or(false)
                    });
                    // For WebSocket, ALPN must be http/1.1
                    if network == "ws" {
                        tls_obj["alpn"] = json!(["http/1.1"]);
                    } else {
                        tls_obj["alpn"] = json!(["h2", "http/1.1"]);
                    }
                    outbound["tls"] = tls_obj;
                }

                // Transport
                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or("");
                    let host_header = if !host.is_empty() {
                        host
                    } else if !sni.is_empty() {
                        sni
                    } else {
                        &node.address
                    };

                    outbound["transport"] = json!({
                        "type": "ws",
                        "path": if path.is_empty() { "/" } else { path },
                        "headers": {
                            "Host": host_header
                        }
                    });
                } else if network == "grpc" {
                    let service_name = conf.get("service_name").and_then(|v| v.as_str()).unwrap_or("");
                    outbound["transport"] = json!({
                        "type": "grpc",
                        "service_name": service_name
                    });
                }

                Ok(outbound)
            }
            ProtocolType::Shadowsocks => {
                let method = conf.get("method").and_then(|v| v.as_str()).unwrap_or("2022-blake3-aes-128-gcm");
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or("");
                if password.is_empty() {
                    bail!("Shadowsocks node requires 'password'");
                }

                Ok(json!({
                    "type": "shadowsocks",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port,
                    "method": method,
                    "password": password
                }))
            }
            ProtocolType::Trojan => {
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or("");
                if password.is_empty() {
                    bail!("Trojan node requires 'password'");
                }
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&node.address);
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");

                let mut outbound = json!({
                    "type": "trojan",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port,
                    "password": password,
                    "tls": {
                        "enabled": true,
                        "server_name": sni,
                        "insecure": conf.get("insecure").and_then(|v| v.as_bool()).unwrap_or(false)
                    }
                });

                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or(sni);
                    outbound["transport"] = json!({
                        "type": "ws",
                        "path": if path.is_empty() { "/" } else { path },
                        "headers": {
                            "Host": host
                        }
                    });
                }

                Ok(outbound)
            }
            ProtocolType::Vmess => {
                let uuid = conf.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
                let alter_id = conf.get("alter_id").and_then(|v| v.as_u64()).unwrap_or(0);
                let security = conf.get("cipher").and_then(|v| v.as_str()).unwrap_or("auto");
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");

                let mut outbound = json!({
                    "type": "vmess",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port,
                    "uuid": uuid,
                    "alter_id": alter_id,
                    "security": security
                });

                if conf.get("tls").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&node.address);
                    outbound["tls"] = json!({
                        "enabled": true,
                        "server_name": sni,
                        "insecure": conf.get("insecure").and_then(|v| v.as_bool()).unwrap_or(false)
                    });
                }

                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or(&node.address);
                    outbound["transport"] = json!({
                        "type": "ws",
                        "path": if path.is_empty() { "/" } else { path },
                        "headers": {
                            "Host": host
                        }
                    });
                }

                Ok(outbound)
            }
            ProtocolType::Socks5 => {
                let mut outbound = json!({
                    "type": "socks",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port
                });
                if let (Some(u), Some(p)) = (conf.get("username").and_then(|v| v.as_str()), conf.get("password").and_then(|v| v.as_str())) {
                    if !u.is_empty() {
                        outbound["username"] = json!(u);
                        outbound["password"] = json!(p);
                    }
                }
                Ok(outbound)
            }
            ProtocolType::Http => {
                let mut outbound = json!({
                    "type": "http",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port
                });
                if let (Some(u), Some(p)) = (conf.get("username").and_then(|v| v.as_str()), conf.get("password").and_then(|v| v.as_str())) {
                    if !u.is_empty() {
                        outbound["username"] = json!(u);
                        outbound["password"] = json!(p);
                    }
                }
                Ok(outbound)
            }
            ProtocolType::Hysteria2 => {
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or("");
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&node.address);
                Ok(json!({
                    "type": "hysteria2",
                    "tag": "proxy",
                    "server": node.address,
                    "server_port": node.port,
                    "password": password,
                    "tls": {
                        "enabled": true,
                        "server_name": sni
                    }
                }))
            }
            ProtocolType::Wireguard => {
                // Handled as native endpoint in sing-box 1.14+
                Ok(json!({}))
            }
            ProtocolType::Masque => {
                Ok(json!({
                    "type": "socks",
                    "tag": "proxy",
                    "server": "127.0.0.1",
                    "server_port": 1819
                }))
            }
            ProtocolType::Openvpn => {
                bail!("OpenVPN protocol is handled natively via Mihomo core");
            }
            ProtocolType::Psiphon => {
                // Psiphon routes through local SOCKS5 on 127.0.0.1:1820
                // (provided by psiphon-tunnel-core process launched by connection_manager)
                Ok(json!({
                    "type": "socks",
                    "tag": "proxy",
                    "server": "127.0.0.1",
                    "server_port": 1820,
                    "version": "5"
                }))
            }
        }
    }

    pub fn generate_config_for_chain(
        &self,
        nodes: &[UnifiedNode],
        settings: &AppSettings,
        work_dir: &Path,
    ) -> Result<String> {
        if nodes.is_empty() {
            bail!("Cannot generate config for empty node chain");
        }
        if nodes.len() == 1 {
            return self.generate_config_with_relay(&nodes[0], None, settings, work_dir);
        }

        let mut outbounds = Vec::new();
        let n = nodes.len();

        for (i, node) in nodes.iter().enumerate() {
            let mut ob = self.build_outbound(node)?;
            if i == 0 {
                ob["tag"] = json!("chain-0");
            } else if i == n - 1 {
                ob["tag"] = json!("proxy");
                ob["detour"] = json!(format!("chain-{}", i - 1));
                ob["domain_resolver"] = json!("dns-remote");
            } else {
                ob["tag"] = json!(format!("chain-{}", i));
                ob["detour"] = json!(format!("chain-{}", i - 1));
                ob["domain_resolver"] = json!("dns-remote");
            }
            outbounds.push(ob);
        }

        outbounds.push(json!({
            "type": "direct",
            "tag": "direct"
        }));
        outbounds.push(json!({
            "type": "block",
            "tag": "block"
        }));

        self.generate_config_common(outbounds, Vec::new(), "chain-0", "chain-0", settings, work_dir)
    }

    pub fn generate_config_for_urltest(
        &self,
        nodes: &[UnifiedNode],
        settings: &AppSettings,
        work_dir: &Path,
    ) -> Result<String> {
        if nodes.is_empty() {
            bail!("Cannot generate urltest config for empty node list");
        }
        if nodes.len() == 1 {
            return self.generate_config_with_relay(&nodes[0], None, settings, work_dir);
        }

        let mut outbounds = Vec::new();
        let mut tags = Vec::new();

        for node in nodes.iter() {
            if let Ok(mut ob) = self.build_outbound(node) {
                let tag = format!("node-{}", tags.len());
                ob["tag"] = json!(tag);
                tags.push(tag);
                outbounds.push(ob);
            }
        }

        if outbounds.is_empty() {
            bail!("No compatible outbound nodes available for urltest");
        }

        // Add the urltest outbound
        outbounds.insert(0, json!({
            "type": "urltest",
            "tag": "smart-urltest",
            "outbounds": tags,
            "url": "http://cp.cloudflare.com/generate_204",
            "interval": "3m",
            "tolerance": 50
        }));

        outbounds.push(json!({
            "type": "direct",
            "tag": "direct"
        }));
        outbounds.push(json!({
            "type": "block",
            "tag": "block"
        }));


        self.generate_config_common(outbounds, Vec::new(), "smart-urltest", "smart-urltest", settings, work_dir)
    }

    pub fn generate_config_common(
        &self,
        outbounds: Vec<Value>,
        endpoints: Vec<Value>,
        proxy_outbound: &str,
        dns_remote_detour: &str,
        settings: &AppSettings,
        _work_dir: &Path,
    ) -> Result<String> {
        let mut inbounds = vec![
            json!({
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "127.0.0.1",
                "listen_port": settings.mixed_port
            })
        ];

        if settings.proxy_mode == ProxyMode::TunMode {
            let mut tun_addresses = vec![json!("172.19.0.1/30")];
            if settings.enable_ipv6 {
                tun_addresses.push(json!("fdfe:dcba:9876::1/126"));
            }
            let tun_inbound = json!({
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "AuraWintun",
                "address": tun_addresses,
                "auto_route": true,
                "strict_route": true,
                "stack": "system"
            });
            inbounds.push(tun_inbound);
        }

        // Helper to locate compiled binary rule sets (.srs)
        let resolve_rule = |name: &str| -> Option<String> {
            let cand1 = _work_dir.join("rules").join(name);
            if cand1.exists() {
                return Some(cand1.to_string_lossy().replace('\\', "/"));
            }
            let cand2 = _work_dir.join(name);
            if cand2.exists() {
                return Some(cand2.to_string_lossy().replace('\\', "/"));
            }
            if let Ok(exe_path) = std::env::current_exe() {
                let exe_dir = exe_path.parent().unwrap_or(Path::new(""));
                let cand3 = exe_dir.join("binaries").join("rules").join(name);
                if cand3.exists() {
                    return Some(cand3.to_string_lossy().replace('\\', "/"));
                }
                let cand4 = exe_dir.join("rules").join(name);
                if cand4.exists() {
                    return Some(cand4.to_string_lossy().replace('\\', "/"));
                }
                let cand5 = exe_dir.join("binaries").join(name);
                if cand5.exists() {
                    return Some(cand5.to_string_lossy().replace('\\', "/"));
                }
                let cand6 = exe_dir.join("resources").join("binaries").join("rules").join(name);
                if cand6.exists() {
                    return Some(cand6.to_string_lossy().replace('\\', "/"));
                }
            }
            let cand7 = PathBuf::from("src-tauri/binaries/rules").join(name);
            if cand7.exists() {
                if let Ok(c) = cand7.canonicalize() {
                    return Some(c.to_string_lossy().replace('\\', "/"));
                }
                return Some(cand7.to_string_lossy().replace('\\', "/"));
            }
            let cand8 = PathBuf::from("binaries/rules").join(name);
            if cand8.exists() {
                if let Ok(c) = cand8.canonicalize() {
                    return Some(c.to_string_lossy().replace('\\', "/"));
                }
                return Some(cand8.to_string_lossy().replace('\\', "/"));
            }
            None
        };

        let mut rule_sets = Vec::new();
        let mut loaded_tags = std::collections::HashSet::new();

        let mut register_rule_set = |rule_tag: &str, file_name: &str| -> bool {
            if loaded_tags.contains(rule_tag) {
                return true;
            }
            if let Some(path) = resolve_rule(file_name) {
                rule_sets.push(json!({
                    "tag": rule_tag,
                    "type": "local",
                    "format": "binary",
                    "path": path
                }));
                loaded_tags.insert(rule_tag.to_string());
                true
            } else {
                false
            }
        };

        let parse_geo_rule = |rule_str: &str| -> Option<(String, String)> {
            let s = rule_str.trim();
            if s.is_empty() {
                return None;
            }
            if let Some(rest) = s.strip_prefix("geosite:") {
                let tag = format!("geosite-{}", rest.replace('@', "-"));
                let filename = format!("geosite-{}.srs", rest);
                Some((tag, filename))
            } else if let Some(rest) = s.strip_prefix("geoip:") {
                let tag = format!("geoip-{}", rest.replace('@', "-"));
                let filename = format!("geoip-{}.srs", rest);
                Some((tag, filename))
            } else {
                None
            }
        };

        let active_set = settings.get_active_rule_set();

        // 1. Process Block Rules
        let mut block_tags = Vec::new();
        let mut block_dns_tags = Vec::new();
        let mut block_domains = Vec::new();
        let mut block_ips = Vec::new();
        for r in &active_set.block_rules {
            let item = r.trim();
            if item.is_empty() {
                continue;
            }
            if let Some((tag, file)) = parse_geo_rule(item) {
                if register_rule_set(&tag, &file) {
                    block_tags.push(tag.clone());
                    if tag.starts_with("geosite-") {
                        block_dns_tags.push(tag);
                    }
                }
            } else if item.contains('/') || item.parse::<std::net::IpAddr>().is_ok() {
                block_ips.push(item.to_string());
            } else {
                block_domains.push(item.to_string());
            }
        }

        // 2. Process Direct Rules (China, Private LAN, custom domains/IPs)
        let mut direct_tags = Vec::new();
        let mut direct_dns_tags = Vec::new();
        let mut direct_domains = Vec::new();
        let mut direct_ips = Vec::new();
        for r in &active_set.direct_rules {
            let item = r.trim();
            if item.is_empty() {
                continue;
            }
            if let Some((tag, file)) = parse_geo_rule(item) {
                if register_rule_set(&tag, &file) {
                    direct_tags.push(tag.clone());
                    if tag.starts_with("geosite-") {
                        direct_dns_tags.push(tag);
                    }
                }
            } else if item.contains('/') || item.parse::<std::net::IpAddr>().is_ok() {
                direct_ips.push(item.to_string());
            } else {
                direct_domains.push(item.to_string());
            }
        }
        for r in &settings.custom_direct_rules {
            let item = r.trim();
            if item.is_empty() {
                continue;
            }
            if item.contains('/') || item.parse::<std::net::IpAddr>().is_ok() {
                direct_ips.push(item.to_string());
            } else {
                direct_domains.push(item.to_string());
            }
        }

        // 3. Process Proxy Rules
        let mut proxy_tags = Vec::new();
        let mut proxy_domains = Vec::new();
        let mut proxy_ips = Vec::new();
        for r in &active_set.proxy_rules {
            let item = r.trim();
            if item.is_empty() {
                continue;
            }
            if let Some((tag, file)) = parse_geo_rule(item) {
                if register_rule_set(&tag, &file) {
                    proxy_tags.push(tag);
                }
            } else if item.contains('/') || item.parse::<std::net::IpAddr>().is_ok() {
                proxy_ips.push(item.to_string());
            } else {
                proxy_domains.push(item.to_string());
            }
        }
        for r in &settings.custom_proxy_rules {
            let item = r.trim();
            if item.is_empty() {
                continue;
            }
            if item.contains('/') || item.parse::<std::net::IpAddr>().is_ok() {
                proxy_ips.push(item.to_string());
            } else {
                proxy_domains.push(item.to_string());
            }
        }

        // Assemble routing rules
        let mut rules = vec![
            json!({
                "protocol": "dns",
                "action": "hijack-dns"
            }),
        ];

        // 1. Ad / Block rules
        if !block_tags.is_empty() {
            rules.push(json!({
                "rule_set": block_tags,
                "outbound": "block"
            }));
        }
        if !block_domains.is_empty() {
            rules.push(json!({
                "domain_suffix": block_domains,
                "outbound": "block"
            }));
        }
        if !block_ips.is_empty() {
            rules.push(json!({
                "ip_cidr": block_ips,
                "outbound": "block"
            }));
        }

        // 1b. Block UDP 443 (QUIC/HTTP3) if enabled
        if settings.block_udp_443 {
            rules.push(json!({
                "network": "udp",
                "port": [443],
                "outbound": "block"
            }));
        }

        // 2. Private LAN IPs (always direct)
        rules.push(json!({
            "ip_is_private": true,
            "outbound": "direct"
        }));

        // 3. IPv6 blocking if disabled
        if !settings.enable_ipv6 {
            rules.push(json!({
                "ip_version": 6,
                "outbound": "block"
            }));
        }

        // 4. Custom & RuleSet Direct Rules (Domain suffixes and IPs)
        if !direct_domains.is_empty() {
            rules.push(json!({
                "domain_suffix": direct_domains,
                "outbound": "direct"
            }));
        }
        if !direct_ips.is_empty() {
            rules.push(json!({
                "ip_cidr": direct_ips,
                "outbound": "direct"
            }));
        }

        // 5. Custom & RuleSet Proxy Rules (Domain suffixes and IPs)
        if !proxy_domains.is_empty() {
            rules.push(json!({
                "domain_suffix": proxy_domains,
                "outbound": proxy_outbound
            }));
        }
        if !proxy_ips.is_empty() {
            rules.push(json!({
                "ip_cidr": proxy_ips,
                "outbound": proxy_outbound
            }));
        }

        // 6. Direct Geo Rules (Domestic & Private LAN - 100% GeoSite/GeoIP based)
        if settings.routing_mode != "global" && !direct_tags.is_empty() {
            rules.push(json!({
                "rule_set": direct_tags,
                "outbound": "direct"
            }));
        }

        // 7. Proxy Geo Rules
        if !proxy_tags.is_empty() {
            rules.push(json!({
                "rule_set": proxy_tags,
                "outbound": proxy_outbound
            }));
        }

        // 8. Default outbound based on routing_mode
        let default_outbound = if settings.routing_mode == "direct" {
            "direct"
        } else {
            proxy_outbound
        };
        rules.push(json!({
            "outbound": default_outbound
        }));

        // Assemble DNS rules for split resolution
        let mut dns_rules = Vec::new();
        if !block_dns_tags.is_empty() {
            dns_rules.push(json!({
                "rule_set": block_dns_tags,
                "action": "reject"
            }));
        }
        if !direct_dns_tags.is_empty() {
            dns_rules.push(json!({
                "rule_set": direct_dns_tags,
                "server": "dns-direct"
            }));
        }

        let mut route_obj = json!({
            "auto_detect_interface": true,
            "default_domain_resolver": "dns-direct",
            "rules": rules
        });
        if !rule_sets.is_empty() {
            route_obj["rule_set"] = json!(rule_sets);
        }

        let mut full_config = json!({
            "log": {
                "level": "info",
                "timestamp": true
            },
            "experimental": {
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", settings.clash_api_port),
                    "external_ui": "",
                    "secret": ""
                }
            },
            "dns": {
                "strategy": if settings.enable_ipv6 { "prefer_ipv4" } else { "ipv4_only" },
                "servers": [
                    {
                        "tag": "dns-remote",
                        "type": "tls",
                        "server": "8.8.8.8",
                        "detour": dns_remote_detour
                    },
                    {
                        "tag": "dns-direct",
                        "type": "udp",
                        "server": "223.5.5.5"
                    }
                ],
                "rules": dns_rules,
                "final": "dns-remote"
            },
            "inbounds": inbounds,
            "outbounds": outbounds,
            "route": route_obj
        });

        if !endpoints.is_empty() {
            full_config["endpoints"] = json!(endpoints);
        }

        Ok(serde_json::to_string_pretty(&full_config)?)
    }
}

impl CoreAdapter for SingBoxAdapter {
    fn generate_config_with_relay(
        &self,
        node: &UnifiedNode,
        relay_node: Option<&UnifiedNode>,
        settings: &AppSettings,
        _work_dir: &Path,
    ) -> Result<String> {
        let is_wireguard = node.protocol == ProtocolType::Wireguard;
        let mut endpoints = Vec::new();
        let mut outbounds = Vec::new();

        if is_wireguard {
            let conf = &node.config;
            let priv_key = conf.get("private_key")
                .and_then(|v| v.as_str())
                .unwrap_or("uIGE36QwoZQ96je0LBtVXv2rE8sCSGCd/lwKPpZU4Gw=");
            let pub_key = conf.get("public_key")
                .and_then(|v| v.as_str())
                .unwrap_or("bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=");
            let ip = conf.get("ip")
                .and_then(|v| v.as_str())
                .unwrap_or("172.16.0.2/32");
            let local_ip = if ip.contains('/') { ip.to_string() } else { format!("{}/32", ip) };

            let mut addresses = vec![json!(local_ip)];
            if settings.enable_ipv6 {
                if let Some(ipv6) = conf.get("ipv6").and_then(|v| v.as_str()) {
                    let local_ipv6 = if ipv6.contains('/') { ipv6.to_string() } else { format!("{}/128", ipv6) };
                    addresses.push(json!(local_ipv6));
                } else {
                    addresses.push(json!("2606:4700:110:854a:44df:78af:2d4a:aade/128"));
                }
            }

            let mut peer_obj = json!({
                "address": node.address,
                "port": node.port,
                "public_key": pub_key,
                "allowed_ips": ["0.0.0.0/0", "::/0"]
            });

            if let Some(res) = conf.get("reserved") {
                peer_obj["reserved"] = res.clone();
            } else {
                peer_obj["reserved"] = json!([170, 168, 7]);
            }

            let wireguard_ep = json!({
                "type": "wireguard",
                "tag": "proxy",
                "private_key": priv_key,
                "address": addresses,
                "peers": [peer_obj],
                "mtu": conf.get("mtu").and_then(|v| v.as_u64()).unwrap_or(1280)
            });

            endpoints.push(wireguard_ep);
        } else if let Some(relay) = relay_node {
            // For local proxy daemons (Psiphon, Masque), the relay was already utilized by the daemon itself.
            // Do NOT detour the local loopback proxy through the remote relay!
            if node.protocol == ProtocolType::Psiphon || node.protocol == ProtocolType::Masque {
                let proxy_outbound = self.build_outbound(node)?;
                outbounds.push(proxy_outbound);
            } else {
                // Chain proxy: Client -> proxy (tag: "proxy") -> detour via relay (tag: "relay") -> Target
                let mut relay_outbound = self.build_outbound(relay)?;
                relay_outbound["tag"] = json!("relay");
                outbounds.push(relay_outbound);

                let mut proxy_outbound = self.build_outbound(node)?;
                proxy_outbound["detour"] = json!("relay");
                proxy_outbound["domain_resolver"] = json!("dns-remote");
                outbounds.push(proxy_outbound);
            }
        } else {
            let proxy_outbound = self.build_outbound(node)?;
            outbounds.push(proxy_outbound);
        }

        outbounds.push(json!({
            "type": "direct",
            "tag": "direct"
        }));
        outbounds.push(json!({
            "type": "block",
            "tag": "block"
        }));

        let has_active_relay = relay_node.is_some()
            && node.protocol != ProtocolType::Psiphon
            && node.protocol != ProtocolType::Masque;
        let dns_remote_detour = if has_active_relay { "relay" } else { "proxy" };

        self.generate_config_common(outbounds, endpoints, "proxy", dns_remote_detour, settings, _work_dir)
    }
}
