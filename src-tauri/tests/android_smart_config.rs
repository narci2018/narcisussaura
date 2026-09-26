//! Reproduces the exact sing-box config the Android smart-group connect
//! path writes on the phone (selector-group config + Android relay patch) so it can
//! be validated with `sing-box check` on the host. CI never type-checks the
//! android cfg branches, and the phone gives us no logs — a config that
//! sing-box rejects would otherwise look like "connecting forever".

use serde_json::json;
use std::path::PathBuf;
use tauri_app_lib::core::adapter::CoreAdapter;
use tauri_app_lib::core::singbox::SingBoxAdapter;
use tauri_app_lib::managers::connection_manager::ConnectionManager;
use tauri_app_lib::models::{AppSettings, NodeStatus, ProtocolType, ProxyMode, UnifiedNode};

fn node(name: &str, protocol: ProtocolType, address: &str, port: u16, config: serde_json::Value) -> UnifiedNode {
    UnifiedNode {
        id: format!("stable-{name}"),
        name: name.to_string(),
        protocol,
        address: address.to_string(),
        port,
        country_code: "HK".to_string(),
        country_name: "Hong Kong".to_string(),
        city: String::new(),
        group: "Default".to_string(),
        tags: vec!["Default".to_string()],
        favorite: false,
        latency_ms: Some(120),
        speed_bps: Some(4_000_000),
        last_checked: None,
        status: NodeStatus::Unknown,
        config,
    }
}

fn sample_nodes() -> Vec<UnifiedNode> {
    vec![
        node(
            "vless-ws-tls",
            ProtocolType::Vless,
            "hk1.example.com",
            443,
            json!({"uuid": "b83e2f1a-2f1c-4f5b-9a1d-1c2d3e4f5a6b", "security": "tls", "sni": "hk1.example.com", "network": "ws", "path": "/vless"}),
        ),
        node(
            "vmess-http",
            ProtocolType::Vmess,
            "104.18.1.2",
            80,
            json!({"uuid": "aaaa1111-2222-3333-4444-555566667777", "alter_id": 0, "cipher": "auto", "network": "ws", "path": "/vmess", "host": "hk2.example.com", "security": "none"}),
        ),
        node(
            "trojan-tcp",
            ProtocolType::Trojan,
            "hk3.example.com",
            443,
            json!({"password": "troj4n-pass", "sni": "hk3.example.com", "network": "tcp"}),
        ),
        node(
            "ss-plain",
            ProtocolType::Shadowsocks,
            "104.18.9.9",
            8388,
            json!({"method": "chacha20-ietf-poly1305", "password": "ss-pass"}),
        ),
    ]
}

fn write_checked(config: &str, out_name: &str) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push(out_name);
    std::fs::write(&path, config).expect("write config");
    println!("wrote {} -> {}", out_name, path.display());
}

#[test]
fn android_smart_group_config_is_valid_json_structure() {
    let settings = AppSettings::default();
    let work_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let adapter = SingBoxAdapter::new();

    // TunMode first: the phone config pipeline strips the tun inbound via the
    // relay patch, so the patch must work on a config that HAS one.
    let mut tun_settings = settings.clone();
    tun_settings.proxy_mode = ProxyMode::TunMode;
    let (raw, members) = adapter
        .generate_config_for_urltest(&sample_nodes(), &tun_settings, &work_dir)
        .expect("smart group config generation failed");
    assert_eq!(members, vec![0usize, 1, 2, 3], "all sample nodes must map to node-k tags");
    let patched = ConnectionManager::patch_android_relay_config(&raw).expect("android patch failed");

    let v: serde_json::Value = serde_json::from_str(&patched).expect("patched config is not valid JSON");
    let inbounds = v["inbounds"].as_array().unwrap();
    assert!(
        !inbounds.iter().any(|ib| ib["type"] == "tun"),
        "tun inbound must be stripped on Android"
    );
    assert_eq!(v["route"]["auto_detect_interface"], false);
    let rules = v["route"]["rules"].as_array().unwrap();
    assert_eq!(rules[0]["action"], "sniff");
    assert!(
        rules[0]["protocol"].is_null()
            || rules[0]["protocol"]
                .as_array()
                .map(|p| p.iter().any(|x| x == "dns"))
                .unwrap_or(false),
        "sniff must cover dns (either unrestricted or with dns listed)"
    );
    assert_eq!(rules[1]["action"], "resolve");
    assert!(
        rules.iter().any(|r| {
            r["action"] == "hijack-dns"
                && r["port"].as_array().map(|p| p.iter().any(|x| x == 53)).unwrap_or(false)
        }),
        "tunrelay DNS (plain UDP to port 53) must be hijacked by port, not only by sniffed protocol"
    );
    // The selector outbound and every referenced tag must exist.
    let outbounds = v["outbounds"].as_array().unwrap();
    let tags: Vec<&str> = outbounds.iter().filter_map(|o| o["tag"].as_str()).collect();
    assert!(tags.contains(&"smart-select"));
    let selector = outbounds.iter().find(|o| o["tag"] == "smart-select").unwrap();
    assert_eq!(selector["type"], "selector");
    for ob in selector["outbounds"].as_array().unwrap() {
        assert!(tags.contains(&ob.as_str().unwrap()), "selector references missing tag {ob}");
    }
    // Probe hosts must resolve via dns-direct: if their resolution rode the
    // pinned member's tunnel, a node that RSTs DNS would make every member
    // fail the probe regardless of its real health.
    let dns_rules = v["dns"]["rules"].as_array().unwrap();
    let probe_rule = dns_rules
        .iter()
        .find(|r| r["domain"].as_array().map(|d| d.iter().any(|x| x == "cp.cloudflare.com")).unwrap_or(false))
        .expect("probe domains must resolve outside the tunnel");
    assert_eq!(probe_rule["server"], "dns-direct");
    // The same rule is what the user's own browsing gets resolved by, and dns-direct
    // is 223.5.5.5 — which lies about google (measured: www.google.com →
    // 69.171.235.22, a Facebook-range address, and Chrome then reports
    // ERR_SSL_VERSION_OR_CIPHER_MISMATCH). Anything added to this list must first be
    // confirmed to resolve honestly from a Chinese public resolver.
    let off_tunnel: Vec<&str> = probe_rule["domain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d.as_str().unwrap())
        .collect();
    assert_eq!(off_tunnel, vec!["cp.cloudflare.com", "ip-api.com"]);
    // dns-remote must be DoH on 443, not DoT 853: 443-only nodes RST 853.
    let dns_servers = v["dns"]["servers"].as_array().unwrap();
    let remote = dns_servers.iter().find(|s| s["tag"] == "dns-remote").unwrap();
    assert_eq!(remote["type"], "https");
    assert_eq!(remote["path"], "/dns-query");
    write_checked(&patched, "android_smart_config.json");

    // Default (SystemProxy) settings path too — what the phone actually uses.
    let (raw2, _) = adapter
        .generate_config_for_urltest(&sample_nodes(), &settings, &work_dir)
        .expect("smart group config generation failed");
    let patched2 = ConnectionManager::patch_android_relay_config(&raw2).expect("android patch failed");
    write_checked(&patched2, "android_smart_config_sysproxy.json");

    // Single-node connect path (manual selection) with the same patch.
    let one = sample_nodes();
    let raw3 = adapter
        .generate_config_with_relay(&one[0], None, &settings, &work_dir)
        .expect("single-node config failed");
    let patched3 = ConnectionManager::patch_android_relay_config(&raw3).expect("android patch failed");
    write_checked(&patched3, "android_single_config.json");
}

#[test]
fn vless_ws_ech_node_config_carries_every_handshake_param() {
    // Mirrors what NodeManager::parse_vless_link now produces for the live
    // freesubplus links (France 122): path/host/fp/ech all survived parsing.
    // If any of them is missing from the sing-box outbound, the node either
    // 404s the WS upgrade, gets RST by the GFW (no uTLS) or hangs at the
    // Cloudflare edge (no ECH) — the exact France failure of v0.2.96-98.
    let settings = AppSettings::default();
    let work_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let fr = node(
        "fr122",
        ProtocolType::Vless,
        "104.252.111.38",
        8443,
        json!({
            "uuid": "08ea5abf-fbe9-4c40-983d-099076942b93",
            "security": "tls",
            "sni": "forfreesub.tclucky.eu.cc",
            "network": "ws",
            "path": "/events?ed=2560",
            "host": "forfreesub.tclucky.eu.cc",
            "fingerprint": "chrome",
            "ech": "cloudflare-ech.com",
        }),
    );
    let adapter = SingBoxAdapter::new();
    let raw = adapter
        .generate_config_with_relay(&fr, None, &settings, &work_dir)
        .expect("single-node config failed");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let ob = v["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["type"] == "vless")
        .expect("vless outbound");
    assert_eq!(ob["transport"]["path"], "/events?ed=2560");
    assert_eq!(ob["transport"]["headers"]["Host"], "forfreesub.tclucky.eu.cc");
    assert_eq!(ob["tls"]["utls"]["enabled"], true);
    assert_eq!(ob["tls"]["utls"]["fingerprint"], "chrome");
    assert_eq!(ob["tls"]["ech"]["enabled"], true);
    assert_eq!(ob["tls"]["ech"]["query_server_name"], "cloudflare-ech.com");
    // The HTTPS record fetch must NOT ride dns-remote through the node whose
    // handshake is waiting for the answer.
    let dns_rules = v["dns"]["rules"].as_array().unwrap();
    let ech_rule = dns_rules
        .iter()
        .find(|r| {
            r["domain"]
                .as_array()
                .map(|d| d.iter().any(|x| x == "cloudflare-ech.com"))
                .unwrap_or(false)
        })
        .expect("ECH public name must resolve outside the tunnel");
    assert_eq!(ech_rule["server"], "dns-direct");
    write_checked(&raw, "android_fr122_ech.json");

    // And the phone-patched variant must stay structurally valid too.
    let patched = ConnectionManager::patch_android_relay_config(&raw).expect("android patch");
    write_checked(&patched, "android_fr122_ech_patched.json");
}
