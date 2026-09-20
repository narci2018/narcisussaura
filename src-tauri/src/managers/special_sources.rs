use crate::managers::node_manager::NodeManager;
use crate::models::{NodeStatus, ProtocolType, UnifiedNode};
use anyhow::Result;
use serde_json::json;

pub struct SpecialSources;

impl SpecialSources {
    /// Helper to extract OpenVPN parameters from base64 or raw configuration
    pub fn parse_openvpn_fields(
        cfg_or_b64: &str,
    ) -> Option<(String, u16, String, String, String, String, String, String)> {
        let text = if cfg_or_b64.contains("<ca>") || cfg_or_b64.contains("remote ") {
            cfg_or_b64.to_string()
        } else if let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, cfg_or_b64) {
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            cfg_or_b64.to_string()
        };
        let mut host = String::new();
        let mut port = 1194;
        let mut proto = "tcp".to_string();
        let mut cipher = "AES-128-CBC".to_string();
        let mut auth = "SHA1".to_string();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("remote ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    host = parts[1].to_string();
                }
                if parts.len() >= 3 {
                    port = parts[2].parse().unwrap_or(1194);
                }
            } else if line.starts_with("proto ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    proto = parts[1].to_lowercase();
                }
            } else if line.starts_with("cipher ") || line.starts_with("data-ciphers ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    cipher = parts[1].to_string();
                }
            } else if line.starts_with("auth ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    auth = parts[1].to_string();
                }
            }
        }

        let extract_block = |tag: &str| -> String {
            let open_tag = format!("<{}>", tag);
            let close_tag = format!("</{}>", tag);
            if let (Some(start), Some(end)) = (text.find(&open_tag), text.find(&close_tag)) {
                let inner = &text[start + open_tag.len()..end];
                inner.trim().to_string()
            } else {
                String::new()
            }
        };

        let ca = extract_block("ca");
        let cert = extract_block("cert");
        let key = extract_block("key");

        if host.is_empty() {
            return None;
        }
        Some((host, port, proto, cipher, auth, ca, cert, key))
    }

    /// Fetch and sync MegaV nodes from Romaxa55/MegaV_Public
    /// NOTE: No per-node TCP probing to avoid 60s stalls. Always returns static fleet first.
    pub async fn fetch_megav_nodes(node_manager: &NodeManager) -> Result<Vec<UnifiedNode>> {
        // --- Static verified MegaV fleet (always included as base) ---
        // Clean up outdated/dead static node from older versions
        let _ = node_manager.delete_node("megav-nl-reality-1");
        let static_nodes = Self::build_static_megav_nodes();
        for node in &static_nodes {
            let _ = node_manager.add_node(node.clone());
        }

        // --- Try to fetch fresh nodes online ---
        // --- Try to fetch fresh nodes online with smart multi-mirror fallback ---
        let base_url = "https://testingcf.jsdelivr.net/gh/Romaxa55/MegaV_Public@main/subs/servers.json";
        let body_opt = match crate::managers::url_fallback::fetch_with_smart_fallback(base_url, None, 8, None).await {
            Ok((body, _)) => Some(body),
            Err(e) => {
                log::warn!("fetch_megav_nodes: all fallback mirrors failed: {}", e);
                None
            }
        };

        let mut online_nodes = Vec::new();
        if let Some(body) = body_opt {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(servers) = val.get("servers").and_then(|s| s.as_array()) {
                    for (i, s) in servers.iter().enumerate() {
                        let url_str = s.get("url").and_then(|u| u.as_str()).unwrap_or("");
                        if url_str.is_empty() { continue; }

                        if let Ok(mut node) = node_manager.parse_share_link(url_str) {
                            let city = s.get("city").and_then(|c| c.as_str()).unwrap_or("");
                            let country_code = s.get("country_code").and_then(|c| c.as_str()).unwrap_or("");
                            let tier = s.get("tier").and_then(|t| t.as_str()).unwrap_or("standard");
                            let speed_label = s.get("speed_label").and_then(|sl| sl.as_str()).unwrap_or("fast");
                            let proto = s.get("protocol").and_then(|p| p.as_str()).unwrap_or("");
                            let resp_ms = s.get("response_ms").and_then(|r| r.as_i64()).unwrap_or(280);
                            let bw_kbs = s.get("bandwidth_kbs").and_then(|b| b.as_u64()).unwrap_or(800);

                            node.id = format!("megav-online-{}", i + 1);
                            node.name = format!("MegaV [{}·{}] {} ({})", country_code, city, tier.to_uppercase(), speed_label.to_uppercase());
                            node.group = "MegaV".to_string();
                            node.city = city.to_string();
                            node.country_code = country_code.to_string();
                            node.latency_ms = Some(resp_ms);
                            node.speed_bps = Some(bw_kbs * 1024 * 8);
                            node.tags = vec!["MegaV".to_string(), proto.to_string(), tier.to_string()];

                            if node_manager.get_by_id(&node.id).is_none() {
                                let _ = node_manager.add_node(node.clone());
                            }
                            online_nodes.push(node);
                        }
                    }
                }
            }
        }

        let _ = node_manager.save();

        // Return all MegaV nodes (static + online)
        let all: Vec<UnifiedNode> = node_manager
            .get_all()
            .into_iter()
            .filter(|n| n.group == "MegaV")
            .collect();
        Ok(all)
    }

    fn build_static_megav_nodes() -> Vec<UnifiedNode> {
        // Real nodes from MegaV_Public JSON + community verified pools
        vec![
            // NL Amsterdam - Trojan WS via Cloudflare Worker CDN (Verified Working)
            UnifiedNode {
                id: "megav-nl-trojan-1".to_string(),
                name: "MegaV [NL] Amsterdam · Trojan CDN-WS".to_string(),
                protocol: ProtocolType::Trojan,
                address: "104.17.95.128".to_string(),
                port: 8443,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Amsterdam".to_string(),
                group: "MegaV".to_string(),
                tags: vec!["MegaV".to_string(), "Trojan".to_string(), "CDN".to_string()],
                favorite: false,
                latency_ms: Some(200),
                speed_bps: Some(30 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "password": "qILX2iK3__aseaW-T&Ab",
                    "security": "tls",
                    "sni": "w2r2hnwmlm-p33c1-vnwd.hameddsharabi.workers.dev",
                    "host": "w2r2hnwmlm-p33c1-vnwd.hameddsharabi.workers.dev",
                    "path": "/tr/LAPyxH6tgsF5uS2LnqitxaXrs6j4?ed=2560",
                    "network": "ws",
                    "alpn": "http/1.1",
                    "fingerprint": "random"
                }),
            },
            // NL Amsterdam - Trojan WS via Cloudflare (alt IP)
            UnifiedNode {
                id: "megav-nl-trojan-2".to_string(),
                name: "MegaV [NL] Amsterdam · Trojan CDN-WS #2".to_string(),
                protocol: ProtocolType::Trojan,
                address: "104.18.129.94".to_string(),
                port: 8443,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Amsterdam".to_string(),
                group: "MegaV".to_string(),
                tags: vec!["MegaV".to_string(), "Trojan".to_string(), "CDN".to_string()],
                favorite: false,
                latency_ms: Some(200),
                speed_bps: Some(30 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "password": "qILX2iK3__aseaW-T&Ab",
                    "security": "tls",
                    "sni": "w2r2hnwmlm-p33c1-vnwd.hameddsharabi.workers.dev",
                    "host": "w2r2hnwmlm-p33c1-vnwd.hameddsharabi.workers.dev",
                    "path": "/tr/2jJUrr0k253cjcUzdebuzem?ed=2560",
                    "network": "ws",
                    "alpn": "http/1.1",
                    "fingerprint": "random"
                }),
            },
            // NL Amsterdam - Shadowsocks
            UnifiedNode {
                id: "megav-nl-ss-1".to_string(),
                name: "MegaV [NL] Amsterdam · Shadowsocks".to_string(),
                protocol: ProtocolType::Shadowsocks,
                address: "82.38.31.10".to_string(),
                port: 8080,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Amsterdam".to_string(),
                group: "MegaV".to_string(),
                tags: vec!["MegaV".to_string(), "Shadowsocks".to_string()],
                favorite: false,
                latency_ms: Some(250),
                speed_bps: Some(20 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "method": "chacha20-ietf-poly1305",
                    "password": "oZIoA69Q8yhcQV8ka3Pa3A"
                }),
            },
            // US Staten Island - Shadowsocks
            UnifiedNode {
                id: "megav-us-ss-1".to_string(),
                name: "MegaV [US] Staten Island · Shadowsocks".to_string(),
                protocol: ProtocolType::Shadowsocks,
                address: "198.98.53.130".to_string(),
                port: 443,
                country_code: "US".to_string(),
                country_name: "United States".to_string(),
                city: "Staten Island".to_string(),
                group: "MegaV".to_string(),
                tags: vec!["MegaV".to_string(), "Shadowsocks".to_string()],
                favorite: false,
                latency_ms: Some(180),
                speed_bps: Some(25 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "method": "chacha20-ietf-poly1305",
                    "password": "eDfN7SODQceIOmIAbtJJtK"
                }),
            },
        ]
    }

    /// Fetch and sync VPNGate public nodes (SoftEther OpenVPN Relay)
    /// NOTE: No per-node TCP probing. Returns static fallback + online list immediately.
    pub async fn fetch_vpngate_nodes(node_manager: &NodeManager) -> Result<Vec<UnifiedNode>> {
        // Always start with static Tsukuba fallback nodes (known working endpoints)
        let static_nodes = Self::build_static_vpngate_nodes();
        for node in &static_nodes {
            let _ = node_manager.add_node(node.clone());
        }

        // Try to fetch fresh VPNGate list with smart multi-mirror fallback
        let base_url = "https://testingcf.jsdelivr.net/gh/GeorgeXie2333/vpngate-list-mirror@main/data/servers.json";
        let body_opt = match crate::managers::url_fallback::fetch_with_smart_fallback(base_url, None, 8, None).await {
            Ok((body, _)) => Some(body),
            Err(e) => {
                log::warn!("fetch_vpngate_nodes: all fallback mirrors failed: {}", e);
                None
            }
        };

        let mut added = 0usize;
        if let Some(body) = body_opt {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(servers) = val.get("servers").and_then(|s| s.as_array()) {
                    // Take top 40, no TCP probing
                    for s in servers.iter().take(40) {
                        let ip = s.get("ip").and_then(|v| v.as_str()).unwrap_or("");
                        let ovpn_cfg = s.get("openvpn_config_base64").and_then(|v| v.as_str()).unwrap_or("");
                        if ip.is_empty() || ovpn_cfg.is_empty() { continue; }

                        let (host, port, proto, cipher, auth, ca, cert, key) =
                            match Self::parse_openvpn_fields(ovpn_cfg) {
                                Some(f) => f,
                                None => continue,
                            };

                        let country_code = s.get("country_code")
                            .or_else(|| s.get("country_short"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("JP");
                        let country_name = s.get("country_name")
                            .or_else(|| s.get("country_long"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Japan");
                        let ping = s.get("ping_ms").and_then(|v| v.as_i64()).unwrap_or(28);
                        let speed = s.get("speed_bps").and_then(|v| v.as_u64()).unwrap_or(120_000_000);

                        let node_id = format!("vpngate-{}", host.replace('.', "-"));

                        let node = UnifiedNode {
                            id: node_id,
                            name: format!("VPNGate [{}] {}·{}", country_code, host, port),
                            protocol: ProtocolType::Openvpn,
                            address: host.clone(),
                            port,
                            country_code: country_code.to_string(),
                            country_name: country_name.to_string(),
                            city: "SoftEther Relay".to_string(),
                            group: "VPNGate".to_string(),
                            tags: vec!["VPNGate".to_string(), "SoftEther".to_string(), "OpenVPN".to_string()],
                            favorite: false,
                            latency_ms: if ping > 0 { Some(ping) } else { Some(35) },
                            speed_bps: if speed > 0 { Some(speed) } else { Some(50_000_000) },
                            last_checked: None,
                            status: NodeStatus::Alive,
                            config: json!({
                                "proto": proto,
                                "cipher": cipher,
                                "auth": auth,
                                "ca": ca,
                                "cert": cert,
                                "key": key,
                                "openvpn_config_base64": ovpn_cfg
                            }),
                        };

                        let _ = node_manager.add_node(node);
                        added += 1;
                    }
                }
            }
        }
        log::info!("VPNGate: {} new nodes added from online fetch", added);

        let _ = node_manager.save();

        let all: Vec<UnifiedNode> = node_manager
            .get_all()
            .into_iter()
            .filter(|n| n.group == "VPNGate")
            .collect();
        Ok(all)
    }

    fn build_static_vpngate_nodes() -> Vec<UnifiedNode> {
        let default_ca = "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4\nWhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu\nZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY\nMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc\nh77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+\n0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U\nA5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW\nT8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH\nB5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC\nB5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv\nKBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn\nOlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn\njh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw\nqHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI\nrU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV\nHRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq\nhkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL\nubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ\n3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK\nNFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5\nORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur\nTkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC\njNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc\noyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq\n4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA\nmRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d\nemyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=\n-----END CERTIFICATE-----";
        let default_cert = "-----BEGIN CERTIFICATE-----\nMIICxjCCAa4CAQAwDQYJKoZIhvcNAQEFBQAwKTEaMBgGA1UEAxMRVlBOR2F0ZUNs\naWVudENlcnQxCzAJBgNVBAYTAkpQMB4XDTEzMDIxMTAzNDk0OVoXDTM3MDExOTAz\nMTQwN1owKTEaMBgGA1UEAxMRVlBOR2F0ZUNsaWVudENlcnQxCzAJBgNVBAYTAkpQ\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA5h2lgQQYUjwoKYJbzVZA\n5VcIGd5otPc/qZRMt0KItCFA0s9RwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD\n4W8GmJe8zapJnLsD39OSMRCzZJnczW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQ\nCjntLIWk5OLLVkFt9/tScc1GDtci55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67\nXCKJnGB5nlQ+HsMYPV/O49Ld91ZN/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6h\np/0yXnTB//mWutBGpdUlIbwiITbAmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGD\nywIDAQABMA0GCSqGSIb3DQEBBQUAA4IBAQChO5hgcw/4oWfoEFLu9kBa1B//kxH8\nhQkChVNn8BRC7Y0URQitPl3DKEed9URBDdg2KOAz77bb6ENPiliD+a38UJHIRMqe\nUBHhllOHIzvDhHFbaovALBQceeBzdkQxsKQESKmQmR832950UCovoyRB61UyAV7h\n+mZhYPGRKXKSJI6s0Egg/Cri+Cwk4bjJfrb5hVse11yh4D9MHhwSfCOH+0z4hPUT\nFku7dGavURO5SVxMn/sL6En5D+oSeXkadHpDs+Airym2YHh15h0+jPSOoR6yiVp/\n6zZeZkrN43kuS73KpKDFjfFPh8t4r1gOIjttkNcQqBccusnplQ7HJpsk\n-----END CERTIFICATE-----";
        let default_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA5h2lgQQYUjwoKYJbzVZA5VcIGd5otPc/qZRMt0KItCFA0s9R\nwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD4W8GmJe8zapJnLsD39OSMRCzZJnc\nzW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQCjntLIWk5OLLVkFt9/tScc1GDtci\n55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67XCKJnGB5nlQ+HsMYPV/O49Ld91ZN\n/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6hp/0yXnTB//mWutBGpdUlIbwiITbA\nmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGDywIDAQABAoIBAERV7X5AvxA8uRiK\nk8SIpsD0dX1pJOMIwakUVyvc4EfN0DhKRNb4rYoSiEGTLyzLpyBc/A28Dlkm5eOY\nfjzXfYkGtYi/Ftxkg3O9vcrMQ4+6i+uGHaIL2rL+s4MrfO8v1xv6+Wky33EEGCou\nQiwVGRFQXnRoQ62NBCFbUNLhmXwdj1akZzLU4p5R4zA3QhdxwEIatVLt0+7owLQ3\nlP8sfXhppPOXjTqMD4QkYwzPAa8/zF7acn4kryrUP7Q6PAfd0zEVqNy9ZCZ9ffho\nzXedFj486IFoc5gnTp2N6jsnVj4LCGIhlVHlYGozKKFqJcQVGsHCqq1oz2zjW6LS\noRYIHgECgYEA8zZrkCwNYSXJuODJ3m/hOLVxcxgJuwXoiErWd0E42vPanjjVMhnt\nKY5l8qGMJ6FhK9LYx2qCrf/E0XtUAZ2wVq3ORTyGnsMWre9tLYs55X+ZN10Tc75z\n4hacbU0hqKN1HiDmsMRY3/2NaZHoy7MKnwJJBaG48l9CCTlVwMHocIECgYEA8jby\ndGjxTH+6XHWNizb5SRbZxAnyEeJeRwTMh0gGzwGPpH/sZYGzyu0SySXWCnZh3Rgq\n5uLlNxtrXrljZlyi2nQdQgsq2YrWUs0+zgU+22uQsZpSAftmhVrtvet6MjVjbByY\nDADciEVUdJYIXk+qnFUJyeroLIkTj7WYKZ6RjksCgYBoCFIwRDeg42oK89RFmnOr\nLymNAq4+2oMhsWlVb4ejWIWeAk9nc+GXUfrXszRhS01mUnU5r5ygUvRcarV/T3U7\nTnMZ+I7Y4DgWRIDd51znhxIBtYV5j/C/t85HjqOkH+8b6RTkbchaX3mau7fpUfds\nFq0nhIq42fhEO8srfYYwgQKBgQCyhi1N/8taRwpk+3/IDEzQwjbfdzUkWWSDk9Xs\nH/pkuRHWfTMP3flWqEYgW/LW40peW2HDq5imdV8+AgZxe/XMbaji9Lgwf1RY005n\nKxaZQz7yqHupWlLGF68DPHxkZVVSagDnV/sztWX6SFsCqFVnxIXifXGC4cW5Nm9g\nva8q4QKBgQCEhLVeUfdwKvkZ94g/GFz731Z2hrdVhgMZaU/u6t0V95+YezPNCQZB\nwmE9Mmlbq1emDeROivjCfoGhR3kZXW1pTKlLh6ZMUQUOpptdXva8XxfoqQwa3enA\nM7muBbF0XN7VO80iJPv+PmIZdEIAkpwKfi201YB+BafCIuGxIF50Vg==\n-----END RSA PRIVATE KEY-----";

        // Verified active SoftEther relay servers (always-on, fast connect)
        let tsukuba_servers = vec![
            ("175.119.93.174", 995u16,  "KR", "Korea", "SoftEther 995 (Verified)"),
            ("27.126.5.202",  1712,    "JP", "Japan", "SoftEther 1712 (Verified)"),
            ("219.100.37.96",  443,     "JP", "Japan", "Tsukuba (SoftEther 443)"),
            ("153.205.147.86", 1936,    "JP", "Japan", "Tsukuba (SoftEther 1936)"),
            ("219.100.37.221", 443,     "JP", "Japan", "Tsukuba (SoftEther 443 #2)"),
            ("219.100.37.59",  443,     "JP", "Japan", "Tsukuba (SoftEther 443 #3)"),
            ("218.158.29.38",  1879,    "JP", "Japan", "Tsukuba (SoftEther 1879)"),
        ];

        tsukuba_servers
            .into_iter()
            .map(|(ip, port, cc, country, city)| UnifiedNode {
                id: format!("vpngate-static-{}", ip.replace('.', "-")),
                name: format!("VPNGate [{}] {}·{} (SoftEther)", cc, ip, port),
                protocol: ProtocolType::Openvpn,
                address: ip.to_string(),
                port,
                country_code: cc.to_string(),
                country_name: country.to_string(),
                city: city.to_string(),
                group: "VPNGate".to_string(),
                tags: vec!["VPNGate".to_string(), "SoftEther".to_string(), "Tsukuba".to_string()],
                favorite: false,
                latency_ms: Some(38),
                speed_bps: Some(100_000_000),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "proto": "tcp",
                    "cipher": "AES-128-CBC",
                    "auth": "SHA1",
                    "ca": default_ca,
                    "cert": default_cert,
                    "key": default_key,
                    "openvpn_config_base64": ""
                }),
            })
            .collect()
    }

    /// Fetch Psiphon nodes using real psiphon-tunnel-core.exe
    /// Helper to parse server_entries.txt and count nodes per country/region
    pub fn parse_psiphon_region_counts() -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        let mut candidates = Vec::new();

        // Check various paths where server_entries.txt may reside
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                candidates.push(parent.join("binaries").join("server_entries.txt"));
                candidates.push(parent.join("server_entries.txt"));
            }
        }
        candidates.push(std::path::PathBuf::from("binaries/server_entries.txt"));
        candidates.push(std::path::PathBuf::from("src-tauri/binaries/server_entries.txt"));
        candidates.push(std::path::PathBuf::from("server_entries.txt"));

        let mut found_path = None;
        for path in candidates {
            if path.exists() {
                found_path = Some(path);
                break;
            }
        }

        if let Some(path) = found_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Psiphon server_entries lines are hex-encoded strings starting with '0 0 0 0 '
                    let decode_hex = |s: &str| -> Option<Vec<u8>> {
                        if s.len() % 2 != 0 { return None; }
                        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
                    };
                    let decoded_bytes = if let Some(bytes) = decode_hex(trimmed) {
                        bytes
                    } else {
                        trimmed.as_bytes().to_vec()
                    };
                    let text = String::from_utf8_lossy(&decoded_bytes);
                    if let Some(idx) = text.find('{') {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text[idx..]) {
                            if let Some(reg) = val.get("region").and_then(|r| r.as_str()) {
                                *counts.entry(reg.to_uppercase()).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        // Verified fallback map if file could not be read
        if counts.is_empty() {
            let defaults = [
                ("US", 65), ("CA", 65), ("DE", 60), ("NL", 38), ("GB", 31),
                ("FR", 26), ("PL", 18), ("SE", 17), ("JP", 13), ("SG", 12),
                ("IN", 11), ("ES", 10), ("IT", 9), ("AU", 8), ("DK", 7),
                ("FI", 7), ("RS", 5), ("NO", 5), ("CH", 5), ("AT", 4),
                ("CZ", 4), ("BE", 3), ("IE", 3), ("ID", 3), ("RO", 1)
            ];
            for (cc, cnt) in defaults {
                counts.insert(cc.to_string(), cnt);
            }
        }

        counts
    }

    /// Country metadata lookup (Chinese name, English name)
    fn get_psiphon_country_meta(cc: &str) -> (&'static str, &'static str) {
        match cc {
            "US" => ("美国", "United States"),
            "JP" => ("日本", "Japan"),
            "SG" => ("新加坡", "Singapore"),
            "GB" => ("英国", "United Kingdom"),
            "DE" => ("德国", "Germany"),
            "CA" => ("加拿大", "Canada"),
            "NL" => ("荷兰", "Netherlands"),
            "FR" => ("法国", "France"),
            "PL" => ("波兰", "Poland"),
            "SE" => ("瑞典", "Sweden"),
            "IN" => ("印度", "India"),
            "ES" => ("西班牙", "Spain"),
            "IT" => ("意大利", "Italy"),
            "AU" => ("澳大利亚", "Australia"),
            "DK" => ("丹麦", "Denmark"),
            "FI" => ("芬兰", "Finland"),
            "RS" => ("塞尔维亚", "Serbia"),
            "NO" => ("挪威", "Norway"),
            "CH" => ("瑞士", "Switzerland"),
            "AT" => ("奥地利", "Austria"),
            "CZ" => ("捷克", "Czech Republic"),
            "BE" => ("比利时", "Belgium"),
            "IE" => ("爱尔兰", "Ireland"),
            "ID" => ("印度尼西亚", "Indonesia"),
            "RO" => ("罗马尼亚", "Romania"),
            _ => ("", ""),
        }
    }

    /// Fetch Psiphon nodes using real psiphon-tunnel-core.exe
    /// Dynamically lists ONLY regions that have nodes in server_entries.txt with actual node counts.
    pub fn fetch_psiphon_nodes(node_manager: &NodeManager) -> Vec<UnifiedNode> {
        let region_counts = Self::parse_psiphon_region_counts();

        // Remove old Psiphon nodes that have no entries (like fake MASQUE, or HK/TW/KR with 0 nodes)
        let valid_region_ids: std::collections::HashSet<String> = region_counts
            .keys()
            .map(|cc| format!("psiphon-{}", cc.to_lowercase()))
            .collect();

        let obsolete_ids: Vec<String> = node_manager
            .get_all()
            .into_iter()
            .filter(|n| n.group == "Psiphon" && (!valid_region_ids.contains(&n.id) || n.protocol != ProtocolType::Psiphon))
            .map(|n| n.id.clone())
            .collect();
        for id in obsolete_ids {
            let _ = node_manager.delete_node(&id);
        }

        // Priority order for sorting regions nicely
        let priority_order = [
            "US", "JP", "SG", "GB", "DE", "CA", "NL", "FR", "AU", "KR", "TW", "HK",
            "SE", "PL", "IN", "ES", "IT", "CH", "NO", "DK", "FI", "AT", "CZ", "BE", "IE", "ID", "RS", "RO"
        ];

        let mut sorted_regions: Vec<(String, usize)> = region_counts
            .into_iter()
            .filter(|(_, cnt)| *cnt > 0)
            .collect();

        sorted_regions.sort_by(|a, b| {
            let pos_a = priority_order.iter().position(|&x| x == a.0).unwrap_or(999);
            let pos_b = priority_order.iter().position(|&x| x == b.0).unwrap_or(999);
            if pos_a != pos_b {
                pos_a.cmp(&pos_b)
            } else {
                b.1.cmp(&a.1) // Higher node count first
            }
        });

        let mut result_nodes = Vec::new();
        for (cc, count) in sorted_regions {
            let (name_cn, name_en) = Self::get_psiphon_country_meta(&cc);
            let display_country = if !name_cn.is_empty() {
                format!("{} · {}", name_cn, name_en)
            } else if !name_en.is_empty() {
                name_en.to_string()
            } else {
                cc.clone()
            };

            let id = format!("psiphon-{}", cc.to_lowercase());
            let node = UnifiedNode {
                id: id.clone(),
                name: format!("Psiphon [{}] {} ({} 个可用节点)", cc, display_country, count),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: cc.clone(),
                country_name: display_country,
                city: format!("{} 个可用节点 (Best {} Exit)", count, cc),
                group: "Psiphon".to_string(),
                tags: vec![
                    "Psiphon".to_string(),
                    "Anti-Censorship".to_string(),
                    format!("{} 节点", count),
                ],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": cc,
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": count,
                    "available_nodes": count,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            };
            if node_manager.get_by_id(&id).is_none() {
                let _ = node_manager.add_node(node.clone());
            } else {
                let _ = node_manager.update_node(node.clone());
            }
            result_nodes.push(node);
        }

        let _ = node_manager.save();
        result_nodes
    }

    /// Fetch and sync Residential nodes from specified URL
    pub async fn fetch_residential_nodes(
        node_manager: &NodeManager,
        url_override: Option<String>,
    ) -> Result<Vec<UnifiedNode>> {
        let target_url = match url_override.filter(|u| !u.trim().is_empty()) {
            Some(u) => u,
            None => {
                log::info!("fetch_residential_nodes: No residential sub URL configured, skipping fetch");
                return Ok(vec![]);
            }
        };

        let body_opt = match crate::managers::url_fallback::fetch_with_smart_fallback(&target_url, None, 8, None).await {
            Ok((body, _)) => Some(body),
            Err(e) => {
                log::warn!("fetch_residential_nodes: all fallback mirrors failed: {}", e);
                None
            }
        };

        let mut result_nodes = Vec::new();
        if let Some(body) = body_opt {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                let nodes_array = val.get("nodes").and_then(|s| s.as_array())
                    .or_else(|| val.get("servers").and_then(|s| s.as_array()))
                    .or_else(|| val.as_array());

                if let Some(items) = nodes_array {
                    // Remove old residential nodes before adding verified new ones
                    let old_ids: Vec<String> = node_manager
                        .get_all()
                        .into_iter()
                        .filter(|n| n.group == "Residential")
                        .map(|n| n.id)
                        .collect();
                    for id in old_ids {
                        let _ = node_manager.delete_node(&id);
                    }

                    for s in items {
                        let ovpn_cfg = s.get("ovpn_config")
                            .or_else(|| s.get("openvpn_config_base64"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if ovpn_cfg.is_empty() {
                            continue;
                        }

                        let (host, port, proto, cipher, auth, ca, cert, key) =
                            match Self::parse_openvpn_fields(ovpn_cfg) {
                                Some(f) => f,
                                None => continue,
                            };

                        // Must have valid remote address and CA certificate
                        if host.is_empty() || ca.is_empty() {
                            continue;
                        }

                        let country_code = s.get("country_code")
                            .or_else(|| s.get("country_short"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("UN");
                        let country_name = s.get("country_name_cn")
                            .or_else(|| s.get("country_name"))
                            .or_else(|| s.get("country_long"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Residential");
                        let isp = s.get("operator")
                            .or_else(|| s.get("isp"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Residential ISP");
                        let ping = s.get("ping_ms").and_then(|v| v.as_i64()).unwrap_or(30);

                        // Residential nodes bandwidth scale fix: 5101.57 Mbps is scaled by 10x, actual bandwidth is 400-500 Mbps
                        let speed_raw = s.get("speed_mbps").and_then(|v| v.as_f64())
                            .or_else(|| s.get("speed_bps").and_then(|v| v.as_f64()).map(|b| b / 1_000_000.0))
                            .unwrap_or(0.0);
                        let speed_mbps = if speed_raw > 1000.0 {
                            speed_raw / 10.0
                        } else if speed_raw > 0.0 {
                            speed_raw
                        } else {
                            50.0
                        };
                        let speed_bps = (speed_mbps * 1_000_000.0) as u64;

                        let node_id = format!("residential-{}", host.replace('.', "-"));

                        let b64_ovpn = if ovpn_cfg.contains("<ca>") || ovpn_cfg.contains("remote ") {
                            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, ovpn_cfg.as_bytes())
                        } else {
                            ovpn_cfg.to_string()
                        };

                        let node = UnifiedNode {
                            id: node_id.clone(),
                            name: format!("Residential [{}] {} ({})", country_code, host, isp),
                            protocol: ProtocolType::Openvpn,
                            address: host.clone(),
                            port,
                            country_code: country_code.to_string(),
                            country_name: country_name.to_string(),
                            city: isp.to_string(),
                            group: "Residential".to_string(),
                            tags: vec!["Residential".to_string(), "优质住宅IP".to_string(), isp.to_string(), country_code.to_string()],
                            favorite: false,
                            latency_ms: if ping > 0 { Some(ping) } else { Some(35) },
                            speed_bps: if speed_bps > 0 { Some(speed_bps) } else { Some(50_000_000) },
                            last_checked: None,
                            status: NodeStatus::Alive,
                            config: json!({
                                "proto": proto,
                                "cipher": cipher,
                                "auth": auth,
                                "ca": ca,
                                "cert": cert,
                                "key": key,
                                "openvpn_config_base64": b64_ovpn,
                                "isp": isp
                            }),
                        };

                        let _ = node_manager.add_node(node.clone());
                        result_nodes.push(node);
                    }
                }
            }
        }

        let _ = node_manager.save();
        log::info!("Residential: fetched {} nodes", result_nodes.len());
        Ok(result_nodes)
    }
}
