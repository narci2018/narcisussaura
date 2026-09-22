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
    assert_eq!(rules[1]["action"], "resolve");
    // The selector outbound and every referenced tag must exist.
    let outbounds = v["outbounds"].as_array().unwrap();
    let tags: Vec<&str> = outbounds.iter().filter_map(|o| o["tag"].as_str()).collect();
    assert!(tags.contains(&"smart-select"));
    let selector = outbounds.iter().find(|o| o["tag"] == "smart-select").unwrap();
    assert_eq!(selector["type"], "selector");
    for ob in selector["outbounds"].as_array().unwrap() {
        assert!(tags.contains(&ob.as_str().unwrap()), "selector references missing tag {ob}");
    }
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
