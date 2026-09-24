use crate::models::{NodeStatus, ProtocolType, UnifiedNode};
use parking_lot::RwLock;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
pub struct NodeManager {
    nodes: Arc<RwLock<Vec<UnifiedNode>>>,
    data_path: PathBuf,
}

/// Whether a group sync may forget the servers that are no longer listed.
///
/// `DropMissing` is the normal case: one authoritative dump covers the whole
/// group. `KeepMissing` is for a partial source — a mirror that publishes a
/// slice must not delete the rest of the list every time the tab is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPrune {
    DropMissing,
    KeepMissing,
}

impl NodeManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let data_path = app_data_dir.join("nodes.json");
        let mut initial_nodes = Vec::new();

        if data_path.exists() {
            if let Ok(content) = fs::read_to_string(&data_path) {
                if let Ok(loaded) = serde_json::from_str::<Vec<UnifiedNode>>(&content) {
                    let mut seen_ids = std::collections::HashSet::new();
                    for node in loaded {
                        if seen_ids.insert(node.id.clone()) {
                            initial_nodes.push(node);
                        }
                    }
                }
            }
        }

        // If completely empty or missing special sources, insert sensible template nodes for the user to try immediately
        // Clean up obsolete dummy sample servers and any WARP sample nodes
        initial_nodes.retain(|n| {
            n.group != "Sample Servers"
                && !n.id.starts_with("warp-")
                && !n.group.to_uppercase().contains("WARP")
        });
        for sample in Self::sample_nodes() {
            if !initial_nodes.iter().any(|n| n.id == sample.id) {
                initial_nodes.push(sample);
            }
        }

        let mgr = Self {
            nodes: Arc::new(RwLock::new(initial_nodes)),
            data_path,
        };
        let _ = mgr.save();
        mgr
    }

    fn sample_nodes() -> Vec<UnifiedNode> {
        vec![
            // ==========================================
            // Psiphon Obfuscated Tunnel Fleet
            // ==========================================
            UnifiedNode {
                id: "psiphon-us".to_string(),
                name: "Psiphon [US] 美国 · United States (65 个可用节点)".to_string(),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: "US".to_string(),
                country_name: "美国 · United States".to_string(),
                city: "65 个可用节点 (Best US Exit)".to_string(),
                group: "Psiphon".to_string(),
                tags: vec!["Psiphon".to_string(), "Anti-Censorship".to_string(), "65 节点".to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": "US",
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": 65,
                    "available_nodes": 65,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            },
            UnifiedNode {
                id: "psiphon-jp".to_string(),
                name: "Psiphon [JP] 日本 · Japan (13 个可用节点)".to_string(),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: "JP".to_string(),
                country_name: "日本 · Japan".to_string(),
                city: "13 个可用节点 (Best Japan Exit)".to_string(),
                group: "Psiphon".to_string(),
                tags: vec!["Psiphon".to_string(), "Anti-Censorship".to_string(), "13 节点".to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": "JP",
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": 13,
                    "available_nodes": 13,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            },
            UnifiedNode {
                id: "psiphon-sg".to_string(),
                name: "Psiphon [SG] 新加坡 · Singapore (12 个可用节点)".to_string(),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: "SG".to_string(),
                country_name: "新加坡 · Singapore".to_string(),
                city: "12 个可用节点 (Best Singapore Exit)".to_string(),
                group: "Psiphon".to_string(),
                tags: vec!["Psiphon".to_string(), "Anti-Censorship".to_string(), "12 节点".to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": "SG",
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": 12,
                    "available_nodes": 12,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            },
            UnifiedNode {
                id: "psiphon-gb".to_string(),
                name: "Psiphon [GB] 英国 · United Kingdom (31 个可用节点)".to_string(),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: "GB".to_string(),
                country_name: "英国 · United Kingdom".to_string(),
                city: "31 个可用节点 (Best UK Exit)".to_string(),
                group: "Psiphon".to_string(),
                tags: vec!["Psiphon".to_string(), "Anti-Censorship".to_string(), "31 节点".to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": "GB",
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": 31,
                    "available_nodes": 31,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            },
            UnifiedNode {
                id: "psiphon-de".to_string(),
                name: "Psiphon [DE] 德国 · Germany (60 个可用节点)".to_string(),
                protocol: ProtocolType::Psiphon,
                address: "psiphon.network".to_string(),
                port: 0,
                country_code: "DE".to_string(),
                country_name: "德国 · Germany".to_string(),
                city: "60 个可用节点 (Best Germany Exit)".to_string(),
                group: "Psiphon".to_string(),
                tags: vec!["Psiphon".to_string(), "Anti-Censorship".to_string(), "60 节点".to_string()],
                favorite: false,
                latency_ms: None,
                speed_bps: None,
                last_checked: None,
                status: NodeStatus::Unknown,
                config: json!({
                    "egress_region": "DE",
                    "local_socks_port": 1820,
                    "local_http_port": 1821,
                    "node_count": 60,
                    "available_nodes": 60,
                    "propagation_channel_id": "FFFFFFFFFFFFFFFF",
                    "sponsor_id": "1111111111111111"
                }),
            },
            // ==========================================
            // VPNGate SoftEther Official Relays (Tsukuba)
            // ==========================================
            UnifiedNode {
                id: "vpngate-static-219-100-37-96".to_string(),
                name: "VPNGate [JP] 219.100.37.96:443 (SoftEther)".to_string(),
                protocol: ProtocolType::Openvpn,
                address: "219.100.37.96".to_string(),
                port: 443,
                country_code: "JP".to_string(),
                country_name: "Japan".to_string(),
                city: "Tsukuba (SoftEther 443)".to_string(),
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
                    "ca": "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4\nWhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu\nZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY\nMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc\nh77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+\n0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U\nA5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW\nT8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH\nB5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC\nB5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv\nKBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn\nOlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn\njh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw\nqHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI\nrU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV\nHRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq\nhkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL\nubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ\n3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK\nNFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5\nORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur\nTkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC\njNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc\noyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq\n4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA\nmRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d\nemyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=\n-----END CERTIFICATE-----",
                    "cert": "-----BEGIN CERTIFICATE-----\nMIICxjCCAa4CAQAwDQYJKoZIhvcNAQEFBQAwKTEaMBgGA1UEAxMRVlBOR2F0ZUNs\naWVudENlcnQxCzAJBgNVBAYTAkpQMB4XDTEzMDIxMTAzNDk0OVoXDTM3MDExOTAz\nMTQwN1owKTEaMBgGA1UEAxMRVlBOR2F0ZUNsaWVudENlcnQxCzAJBgNVBAYTAkpQ\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA5h2lgQQYUjwoKYJbzVZA\n5VcIGd5otPc/qZRMt0KItCFA0s9RwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD\n4W8GmJe8zapJnLsD39OSMRCzZJnczW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQ\nCjntLIWk5OLLVkFt9/tScc1GDtci55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67\nXCKJnGB5nlQ+HsMYPV/O49Ld91ZN/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6h\np/0yXnTB//mWutBGpdUlIbwiITbAmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGD\nywIDAQABMA0GCSqGSIb3DQEBBQUAA4IBAQChO5hgcw/4oWfoEFLu9kBa1B//kxH8\nhQkChVNn8BRC7Y0URQitPl3DKEed9URBDdg2KOAz77bb6ENPiliD+a38UJHIRMqe\nUBHhllOHIzvDhHFbaovALBQceeBzdkQxsKQESKmQmR832950UCovoyRB61UyAV7h\n+mZhYPGRKXKSJI6s0Egg/Cri+Cwk4bjJfrb5hVse11yh4D9MHhwSfCOH+0z4hPUT\nFku7dGavURO5SVxMn/sL6En5D+oSeXkadHpDs+Airym2YHh15h0+jPSOoR6yiVp/\n6zZeZkrN43kuS73KpKDFjfFPh8t4r1gOIjttkNcQqBccusnplQ7HJpsk\n-----END CERTIFICATE-----",
                    "key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA5h2lgQQYUjwoKYJbzVZA5VcIGd5otPc/qZRMt0KItCFA0s9R\nwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD4W8GmJe8zapJnLsD39OSMRCzZJnc\nzW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQCjntLIWk5OLLVkFt9/tScc1GDtci\n55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67XCKJnGB5nlQ+HsMYPV/O49Ld91ZN\n/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6hp/0yXnTB//mWutBGpdUlIbwiITbA\nmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGDywIDAQABAoIBAERV7X5AvxA8uRiK\nk8SIpsD0dX1pJOMIwakUVyvc4EfN0DhKRNb4rYoSiEGTLyzLpyBc/A28Dlkm5eOY\nfjzXfYkGtYi/Ftxkg3O9vcrMQ4+6i+uGHaIL2rL+s4MrfO8v1xv6+Wky33EEGCou\nQiwVGRFQXnRoQ62NBCFbUNLhmXwdj1akZzLU4p5R4zA3QhdxwEIatVLt0+7owLQ3\nlP8sfXhppPOXjTqMD4QkYwzPAa8/zF7acn4kryrUP7Q6PAfd0zEVqNy9ZCZ9ffho\nzXedFj486IFoc5gnTp2N6jsnVj4LCGIhlVHlYGozKKFqJcQVGsHCqq1oz2zjW6LS\noRYIHgECgYEA8zZrkCwNYSXJuODJ3m/hOLVxcxgJuwXoiErWd0E42vPanjjVMhnt\nKY5l8qGMJ6FhK9LYx2qCrf/E0XtUAZ2wVq3ORTyGnsMWre9tLYs55X+ZN10Tc75z\n4hacbU0hqKN1HiDmsMRY3/2NaZHoy7MKnwJJBaG48l9CCTlVwMHocIECgYEA8jby\ndGjxTH+6XHWNizb5SRbZxAnyEeJeRwTMh0gGzwGPpH/sZYGzyu0SySXWCnZh3Rgq\n5uLlNxtrXrljZlyi2nQdQgsq2YrWUs0+zgU+22uQsZpSAftmhVrtvet6MjVjbByY\nDADciEVUdJYIXk+qnFUJyeroLIkTj7WYKZ6RjksCgYBoCFIwRDeg42oK89RFmnOr\nLymNAq4+2oMhsWlVb4ejWIWeAk9nc+GXUfrXszRhS01mUnU5r5ygUvRcarV/T3U7\nTnMZ+I7Y4DgWRIDd51znhxIBtYV5j/C/t85HjqOkH+8b6RTkbchaX3mau7fpUfds\nFq0nhIq42fhEO8srfYYwgQKBgQCyhi1N/8taRwpk+3/IDEzQwjbfdzUkWWSDk9Xs\nH/pkuRHWfTMP3flWqEYgW/LW40peW2HDq5imdV8+AgZxe/XMbaji9Lgwf1RY005n\nKxaZQz7yqHupWlLGF68DPHxkZVVSagDnV/sztWX6SFsCqFVnxIXifXGC4cW5Nm9g\nva8q4QKBgQCEhLVeUfdwKvkZ94g/GFz731Z2hrdVhgMZaU/u6t0V95+YezPNCQZB\nwmE9Mmlbq1emDeROivjCfoGhR3kZXW1pTKlLh6ZMUQUOpptdXva8XxfoqQwa3enA\nM7muBbF0XN7VO80iJPv+PmIZdEIAkpwKfi201YB+BafCIuGxIF50Vg==\n-----END RSA PRIVATE KEY-----",
                    "openvpn_config_base64": ""
                }),
            },
            UnifiedNode {
                id: "vpngate-static-153-205-147-86".to_string(),
                name: "VPNGate [JP] 153.205.147.86:1936 (SoftEther)".to_string(),
                protocol: ProtocolType::Openvpn,
                address: "153.205.147.86".to_string(),
                port: 1936,
                country_code: "JP".to_string(),
                country_name: "Japan".to_string(),
                city: "Tsukuba (SoftEther 1936)".to_string(),
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
                    "ca": "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4\nWhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu\nZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY\nMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc\nh77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+\n0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U\nA5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW\nT8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH\nB5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC\nB5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv\nKBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn\nOlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn\njh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw\nqHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI\nrU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV\nHRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq\nhkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL\nubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ\n3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK\nNFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5\nORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur\nTkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC\njNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc\noyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq\n4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA\nmRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d\nemyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=\n-----END CERTIFICATE-----",
                    "cert": "-----BEGIN CERTIFICATE-----\nMIICxjCCAa4CAQAwDQYJKoZIhvcNAQEFBQAwKTEaMBgGA1UEAxMRVlBOR2F0ZUNs\naWVudENlcnQxCzAJBgNVBAYTAkpQMB4XDTEzMDIxMTAzNDk0OVoXDTM3MDExOTAz\nMTQwN1owKTEaMBgGA1UEAxMRVlBOR2F0ZUNsaWVudENlcnQxCzAJBgNVBAYTAkpQ\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA5h2lgQQYUjwoKYJbzVZA\n5VcIGd5otPc/qZRMt0KItCFA0s9RwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD\n4W8GmJe8zapJnLsD39OSMRCzZJnczW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQ\nCjntLIWk5OLLVkFt9/tScc1GDtci55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67\nXCKJnGB5nlQ+HsMYPV/O49Ld91ZN/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6h\np/0yXnTB//mWutBGpdUlIbwiITbAmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGD\nywIDAQABMA0GCSqGSIb3DQEBBQUAA4IBAQChO5hgcw/4oWfoEFLu9kBa1B//kxH8\nhQkChVNn8BRC7Y0URQitPl3DKEed9URBDdg2KOAz77bb6ENPiliD+a38UJHIRMqe\nUBHhllOHIzvDhHFbaovALBQceeBzdkQxsKQESKmQmR832950UCovoyRB61UyAV7h\n+mZhYPGRKXKSJI6s0Egg/Cri+Cwk4bjJfrb5hVse11yh4D9MHhwSfCOH+0z4hPUT\nFku7dGavURO5SVxMn/sL6En5D+oSeXkadHpDs+Airym2YHh15h0+jPSOoR6yiVp/\n6zZeZkrN43kuS73KpKDFjfFPh8t4r1gOIjttkNcQqBccusnplQ7HJpsk\n-----END CERTIFICATE-----",
                    "key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA5h2lgQQYUjwoKYJbzVZA5VcIGd5otPc/qZRMt0KItCFA0s9R\nwReNVa9fDRFLRBhcITOlv3FBcW3E8h1Us7RD4W8GmJe8zapJnLsD39OSMRCzZJnc\nzW4OCH1PZRZWKqDtjlNca9AF8a65jTmlDxCQCjntLIWk5OLLVkFt9/tScc1GDtci\n55ofhaNAYMPiH7V8+1g66pGHXAoWK6AQVH67XCKJnGB5nlQ+HsMYPV/O49Ld91ZN\n/2tHkcaLLyNtywxVPRSsRh480jju0fcCsv6hp/0yXnTB//mWutBGpdUlIbwiITbA\nmrsbYnjigRvnPqX1RNJUbi9Fp6C2c/HIFJGDywIDAQABAoIBAERV7X5AvxA8uRiK\nk8SIpsD0dX1pJOMIwakUVyvc4EfN0DhKRNb4rYoSiEGTLyzLpyBc/A28Dlkm5eOY\nfjzXfYkGtYi/Ftxkg3O9vcrMQ4+6i+uGHaIL2rL+s4MrfO8v1xv6+Wky33EEGCou\nQiwVGRFQXnRoQ62NBCFbUNLhmXwdj1akZzLU4p5R4zA3QhdxwEIatVLt0+7owLQ3\nlP8sfXhppPOXjTqMD4QkYwzPAa8/zF7acn4kryrUP7Q6PAfd0zEVqNy9ZCZ9ffho\nzXedFj486IFoc5gnTp2N6jsnVj4LCGIhlVHlYGozKKFqJcQVGsHCqq1oz2zjW6LS\noRYIHgECgYEA8zZrkCwNYSXJuODJ3m/hOLVxcxgJuwXoiErWd0E42vPanjjVMhnt\nKY5l8qGMJ6FhK9LYx2qCrf/E0XtUAZ2wVq3ORTyGnsMWre9tLYs55X+ZN10Tc75z\n4hacbU0hqKN1HiDmsMRY3/2NaZHoy7MKnwJJBaG48l9CCTlVwMHocIECgYEA8jby\ndGjxTH+6XHWNizb5SRbZxAnyEeJeRwTMh0gGzwGPpH/sZYGzyu0SySXWCnZh3Rgq\n5uLlNxtrXrljZlyi2nQdQgsq2YrWUs0+zgU+22uQsZpSAftmhVrtvet6MjVjbByY\nDADciEVUdJYIXk+qnFUJyeroLIkTj7WYKZ6RjksCgYBoCFIwRDeg42oK89RFmnOr\nLymNAq4+2oMhsWlVb4ejWIWeAk9nc+GXUfrXszRhS01mUnU5r5ygUvRcarV/T3U7\nTnMZ+I7Y4DgWRIDd51znhxIBtYV5j/C/t85HjqOkH+8b6RTkbchaX3mau7fpUfds\nFq0nhIq42fhEO8srfYYwgQKBgQCyhi1N/8taRwpk+3/IDEzQwjbfdzUkWWSDk9Xs\nH/pkuRHWfTMP3flWqEYgW/LW40peW2HDq5imdV8+AgZxe/XMbaji9Lgwf1RY005n\nKxaZQz7yqHupWlLGF68DPHxkZVVSagDnV/sztWX6SFsCqFVnxIXifXGC4cW5Nm9g\nva8q4QKBgQCEhLVeUfdwKvkZ94g/GFz731Z2hrdVhgMZaU/u6t0V95+YezPNCQZB\nwmE9Mmlbq1emDeROivjCfoGhR3kZXW1pTKlLh6ZMUQUOpptdXva8XxfoqQwa3enA\nM7muBbF0XN7VO80iJPv+PmIZdEIAkpwKfi201YB+BafCIuGxIF50Vg==\n-----END RSA PRIVATE KEY-----",
                    "openvpn_config_base64": ""
                }),
            },
            // ==========================================
            // MegaV High-Speed Fleet
            // ==========================================
            UnifiedNode {
                id: "megav-nl-reality-1".to_string(),
                name: "MegaV [NL] Naaldwijk · VLESS Reality".to_string(),
                protocol: ProtocolType::Vless,
                address: "45.82.67.183".to_string(),
                port: 443,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Naaldwijk".to_string(),
                group: "MegaV".to_string(),
                tags: vec!["MegaV".to_string(), "VLESS".to_string(), "Reality".to_string()],
                favorite: false,
                latency_ms: Some(220),
                speed_bps: Some(50 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "uuid": "dfc220d1-0b3a-44a2-99e3-8075bcae3b68",
                    "flow": "xtls-rprx-vision",
                    "security": "reality",
                    "sni": "northwaleswildlifetrust.org.uk",
                    "public_key": "daiJkQwpAcBk7oH1iZzRthMj-jAlqovX7vAiwVqfjTU",
                    "short_id": "8880a9f75eb46d91",
                    "fingerprint": "firefox",
                    "network": "tcp"
                }),
            },
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
        ]
    }

    pub fn get_all(&self) -> Vec<UnifiedNode> {
        self.nodes.read().clone()
    }

    pub fn get_by_id(&self, id: &str) -> Option<UnifiedNode> {
        self.nodes.read().iter().find(|n| n.id == id).cloned()
    }

    fn build_seed_relay_nodes() -> Vec<UnifiedNode> {
        vec![
            UnifiedNode {
                id: "seed-relay-nl-reality".to_string(),
                name: "Relay [NL] Naaldwijk · VLESS Reality (Verified)".to_string(),
                protocol: ProtocolType::Vless,
                address: "45.82.67.183".to_string(),
                port: 443,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Naaldwijk".to_string(),
                group: "Relay Seeds".to_string(),
                tags: vec!["Relay".to_string(), "VLESS".to_string(), "Reality".to_string()],
                favorite: false,
                latency_ms: Some(210),
                speed_bps: Some(50 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "uuid": "dfc220d1-0b3a-44a2-99e3-8075bcae3b68",
                    "flow": "xtls-rprx-vision",
                    "security": "reality",
                    "sni": "northwaleswildlifetrust.org.uk",
                    "public_key": "daiJkQwpAcBk7oH1iZzRthMj-jAlqovX7vAiwVqfjTU",
                    "short_id": "8880a9f75eb46d91",
                    "fingerprint": "firefox",
                    "network": "tcp"
                }),
            },
            UnifiedNode {
                id: "seed-relay-nl-ss".to_string(),
                name: "Relay [NL] Amsterdam · Shadowsocks (Verified)".to_string(),
                protocol: ProtocolType::Shadowsocks,
                address: "82.38.31.10".to_string(),
                port: 8080,
                country_code: "NL".to_string(),
                country_name: "Netherlands".to_string(),
                city: "Amsterdam".to_string(),
                group: "Relay Seeds".to_string(),
                tags: vec!["Relay".to_string(), "Shadowsocks".to_string()],
                favorite: false,
                latency_ms: Some(230),
                speed_bps: Some(30 * 1024 * 1024),
                last_checked: None,
                status: NodeStatus::Alive,
                config: json!({
                    "method": "chacha20-ietf-poly1305",
                    "password": "oZIoA69Q8yhcQV8ka3Pa3A"
                }),
            },
        ]
    }

    /// Returns suitable candidates for being a relay / dialer-proxy node.
    /// Returns suitable candidates for being a relay / dialer-proxy node.
    /// Detects active local proxy ports (10808, 7890, 10809), includes MegaV/custom VPS nodes,
    /// and filters out fake-low-latency CDN Anycast / dead Cloudflare worker traps.
    pub fn get_relay_candidates(&self) -> Vec<UnifiedNode> {
        let mut candidates: Vec<UnifiedNode> = Vec::new();

        // 1. Proactively detect active local upstream proxy ports (e.g. v2rayN, Clash, Xray, Sing-Box)
        let local_probes = [
            (10808, "SOCKS5 本地代理 (127.0.0.1:10808 · v2rayN/Xray)", ProtocolType::Socks5),
            (7890, "SOCKS5 本地代理 (127.0.0.1:7890 · Clash)", ProtocolType::Socks5),
            (10809, "HTTP 本地代理 (127.0.0.1:10809 · v2rayN)", ProtocolType::Http),
            (1080, "SOCKS5 本地代理 (127.0.0.1:1080)", ProtocolType::Socks5),
        ];

        for (port, name, proto) in local_probes {
            let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
            if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(25)).is_ok() {
                candidates.push(UnifiedNode {
                    id: format!("local-relay-{}", port),
                    name: name.to_string(),
                    protocol: proto,
                    address: "127.0.0.1".to_string(),
                    port,
                    country_code: "LOCAL".to_string(),
                    country_name: "本地极速中转".to_string(),
                    city: "Localhost".to_string(),
                    group: "LocalProxy".to_string(),
                    tags: vec!["Local".to_string(), "Relay".to_string()],
                    favorite: true,
                    // A port that answers a 25ms connect has not carried a single
                    // byte yet. These used to be pinned at "1ms / 100Mbps / Alive",
                    // which made every local listener win the relay race regardless
                    // of whether the proxy behind it worked. The relay ranking
                    // (managers/relay_selector.rs) dials them like everyone else.
                    latency_ms: None,
                    speed_bps: None,
                    last_checked: None,
                    status: NodeStatus::Unknown,
                    config: serde_json::json!({}),
                });
            }
        }

        let nodes = self.nodes.read();
        let mut sub_candidates: Vec<UnifiedNode> = nodes
            .iter()
            .filter(|n| {
                n.group != "VPNGate"
                    && n.group != "Psiphon"
                    && n.group != "Residential"
                    && n.protocol != ProtocolType::Psiphon
                    && n.protocol != ProtocolType::Openvpn
                    && n.protocol != ProtocolType::Masque
                    && (n.protocol == ProtocolType::Vless
                        || n.protocol == ProtocolType::Trojan
                        || n.protocol == ProtocolType::Shadowsocks
                        || n.protocol == ProtocolType::Vmess
                        || n.protocol == ProtocolType::Hysteria2
                        || n.protocol == ProtocolType::Socks5
                        || n.protocol == ProtocolType::Http)
            })
            .cloned()
            .collect();

        // Helper: identify Cloudflare Anycast IP or Worker/Pages traps (cannot dial arbitrary OpenVPN TCP ports)
        let is_cf_trap = |n: &UnifiedNode| -> bool {
            let addr = &n.address;
            let lat = n.latency_ms.unwrap_or(9999);
            let sni = n.config.get("sni").and_then(|v| v.as_str()).unwrap_or("");
            let host = n.config.get("host").and_then(|v| v.as_str()).unwrap_or("");
            
            // Cloudflare Pages or Workers cannot dial arbitrary OpenVPN TCP ports
            if sni.ends_with(".pages.dev") || sni.ends_with(".workers.dev") 
                || host.ends_with(".pages.dev") || host.ends_with(".workers.dev") {
                return true;
            }

            // Cloudflare anycast CIDRs commonly used as fronting IPs
            let is_cf_ip = addr.starts_with("104.1")
                || addr.starts_with("104.2")
                || addr.starts_with("172.6")
                || addr.starts_with("172.7")
                || addr.starts_with("162.15")
                || addr.starts_with("108.162.")
                || addr.starts_with("198.41.");
            if is_cf_ip && n.protocol == ProtocolType::Vmess {
                return true;
            }
            is_cf_ip && lat < 120
        };

        // Protocol weight: Shadowsocks/Trojan/Vless with direct transport
        let proto_weight = |n: &UnifiedNode| -> u32 {
            match n.protocol {
                ProtocolType::Shadowsocks => 0,
                ProtocolType::Trojan => 1,
                ProtocolType::Vless => 2,
                ProtocolType::Hysteria2 => 3,
                ProtocolType::Vmess => 4,
                ProtocolType::Socks5 => 5,
                ProtocolType::Http => 6,
                _ => 7,
            }
        };

        sub_candidates.sort_by(|a, b| {
            let a_alive = a.status == NodeStatus::Alive;
            let b_alive = b.status == NodeStatus::Alive;
            if a_alive != b_alive {
                return b_alive.cmp(&a_alive);
            }

            let a_trap = is_cf_trap(a);
            let b_trap = is_cf_trap(b);
            if a_trap != b_trap {
                return a_trap.cmp(&b_trap); // Non-trap first
            }

            let a_weight = proto_weight(a);
            let b_weight = proto_weight(b);
            if a_weight != b_weight {
                return a_weight.cmp(&b_weight);
            }

            let a_lat = a.latency_ms.unwrap_or(9999);
            let b_lat = b.latency_ms.unwrap_or(9999);
            if a_lat != b_lat {
                return a_lat.cmp(&b_lat);
            }
            b.favorite.cmp(&a.favorite)
        });

        candidates.extend(sub_candidates);

        // If no candidate from local probes or subscriptions, provide verified built-in relay seeds
        if candidates.is_empty() {
            candidates = Self::build_seed_relay_nodes();
        }

        candidates
    }

    /// Returns the best single relay candidate node.
    ///
    /// `preferred_id` is what the startup ranking actually dialled traffic
    /// through (`settings.preferred_relay_id`): a measured node beats this
    /// heuristic list, so it wins whenever it is still a candidate at all.
    pub fn get_best_relay_node(&self, preferred_id: Option<&str>) -> Option<UnifiedNode> {
        let candidates = self.get_relay_candidates();
        if let Some(id) = preferred_id.filter(|id| !id.trim().is_empty()) {
            if let Some(hit) = candidates.iter().find(|n| n.id == id) {
                return Some(hit.clone());
            }
        }
        candidates.into_iter().next()
    }

    pub fn add_node(&self, mut node: UnifiedNode) -> Result<UnifiedNode, String> {
        if node.id.is_empty() {
            node.id = Uuid::new_v4().to_string();
        }
        let mut lock = self.nodes.write();
        if let Some(existing) = lock.iter_mut().find(|n| n.id == node.id) {
            *existing = node.clone();
            drop(lock);
            self.save()?;
            return Ok(node);
        }
        lock.push(node.clone());
        drop(lock);
        self.save()?;
        Ok(node)
    }

    pub fn update_node(&self, node: UnifiedNode) -> Result<(), String> {
        let mut lock = self.nodes.write();
        if let Some(existing) = lock.iter_mut().find(|n| n.id == node.id) {
            *existing = node;
            drop(lock);
            self.save()?;
            Ok(())
        } else {
            Err("Node not found".to_string())
        }
    }

    pub fn delete_node(&self, id: &str) -> Result<(), String> {
        let mut lock = self.nodes.write();
        lock.retain(|n| n.id != id);
        drop(lock);
        self.save()?;
        Ok(())
    }

    /// Swap a group for a freshly built list in one write.
    ///
    /// A network source that drops a server must drop it here too under
    /// `DropMissing`: servers that fell out of the list would otherwise sit in
    /// the group forever and be dialled by every later liveness pass. What the
    /// new list repeats keeps the old record's favourite flag and its
    /// measurements, so a resync does not wipe the user's stars or make every
    /// row read "未测" again.
    pub fn replace_group(
        &self,
        group: &str,
        nodes: Vec<UnifiedNode>,
        prune: GroupPrune,
    ) -> Result<Vec<UnifiedNode>, String> {
        let mut lock = self.nodes.write();
        let carried: Vec<UnifiedNode> = lock
            .iter()
            .filter(|n| n.group == group)
            .cloned()
            .collect();
        if prune == GroupPrune::DropMissing {
            lock.retain(|n| n.group != group);
        } else {
            let incoming: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
            // Keep everything the new list does not mention; overwrite only what
            // it repeats.
            lock.retain(|n| n.group != group || !incoming.contains(&n.id));
        }

        let merged: Vec<UnifiedNode> = nodes
            .into_iter()
            .map(|mut node| {
                if let Some(old) = carried.iter().find(|o| o.id == node.id) {
                    node.favorite = node.favorite || old.favorite;
                    if node.latency_ms.is_none() {
                        node.latency_ms = old.latency_ms;
                        node.speed_bps = old.speed_bps;
                        node.last_checked = old.last_checked;
                        node.status = old.status.clone();
                    }
                }
                node
            })
            .collect();
        lock.extend(merged.iter().cloned());
        let group_now: Vec<UnifiedNode> =
            lock.iter().filter(|n| n.group == group).cloned().collect();
        drop(lock);
        self.save()?;
        Ok(group_now)
    }

    pub fn update_latency(&self, id: &str, latency: Option<i64>) {
        let mut lock = self.nodes.write();
        if let Some(node) = lock.iter_mut().find(|n| n.id == id) {
            node.latency_ms = latency;
            node.last_checked = Some(chrono::Utc::now().timestamp());
            node.status = match latency {
                Some(lat) if lat > 0 => NodeStatus::Alive,
                _ => NodeStatus::Dead,
            };
        }
    }

    pub fn update_speed(&self, id: &str, speed_bps: Option<u64>) {
        let mut lock = self.nodes.write();
        if let Some(node) = lock.iter_mut().find(|n| n.id == id) {
            node.speed_bps = speed_bps;
            node.last_checked = Some(chrono::Utc::now().timestamp());
        }
    }

    pub fn toggle_favorite(&self, id: &str) -> Result<bool, String> {
        let mut lock = self.nodes.write();
        if let Some(node) = lock.iter_mut().find(|n| n.id == id) {
            node.favorite = !node.favorite;
            let fav = node.favorite;
            drop(lock);
            self.save()?;
            Ok(fav)
        } else {
            Err("Node not found".to_string())
        }
    }

    pub fn parse_share_link(&self, link: &str) -> Result<UnifiedNode, String> {
        let link = link.trim();
        if link.starts_with("vless://") {
            Self::parse_vless_link(link)
        } else if link.starts_with("trojan://") {
            Self::parse_trojan_link(link)
        } else if link.starts_with("socks5://") || link.starts_with("socks://") {
            Self::parse_socks5_link(link)
        } else if link.starts_with("ss://") {
            Self::parse_ss_link(link)
        } else if link.starts_with("warp://") || link.starts_with("wireguard://") {
            Self::parse_wireguard_link(link)
        } else {
            Err("Unsupported share link format. Supported: vless://, trojan://, ss://, socks5://, warp://, wireguard://".to_string())
        }
    }

    pub fn parse_wireguard_link(link: &str) -> Result<UnifiedNode, String> {
        let (raw_url, fragment) = match link.split_once('#') {
            Some((u, f)) => (u, Some(f)),
            None => (link, None),
        };

        // Normalize URL if host is auto or missing
        let normalized = if raw_url.starts_with("warp://auto") || raw_url.starts_with("warp://@auto") {
            raw_url.replacen("auto", "162.159.192.1:2408", 1)
        } else {
            raw_url.to_string()
        };

        let parsed = Url::parse(&normalized).map_err(|e| format!("Invalid URL: {}", e))?;
        let mut host = parsed.host_str().unwrap_or("162.159.192.1").to_string();
        if host.is_empty() || host == "auto" {
            host = "162.159.192.1".to_string();
        }
        let port = parsed.port().unwrap_or(2408);
        let name = fragment
            .map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string())
            .unwrap_or_else(|| format!("Cloudflare WARP - {}", host));

        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        let pub_key = query
            .get("public_key")
            .or_else(|| query.get("pk"))
            .cloned()
            .unwrap_or_else(|| "bmXOC+F1FxEMF9dyiK2H5/1SUtzH0JuVo51h2wPfgyo=".to_string());

        let priv_key = query
            .get("private_key")
            .cloned()
            .unwrap_or_else(|| "iNw48fdfcf4wrc9i7A21gyFG09a3E3NPydvb2ysTQGY=".to_string());

        let ip = query.get("ip").cloned().unwrap_or_else(|| "172.16.0.2/32".to_string());
        let ipv6 = query.get("ipv6").cloned().or_else(|| Some("2606:4700:110:88b9:299d:7012:4548:78eb/128".to_string()));

        let name_upper = name.to_uppercase();
        let country_code = if name_upper.contains("HK") || name.contains("香港") {
            "HK".to_string()
        } else if name_upper.contains("US") || name.contains("美国") {
            "US".to_string()
        } else if name_upper.contains("DE") || name.contains("德国") {
            "DE".to_string()
        } else if name_upper.contains("JP") || name.contains("日本") {
            "JP".to_string()
        } else if name_upper.contains("SG") || name.contains("新加坡") {
            "SG".to_string()
        } else {
            "CF".to_string()
        };

        Ok(UnifiedNode {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: ProtocolType::Wireguard,
            address: host,
            port,
            country_code,
            country_name: "Cloudflare Edge".to_string(),
            city: "WARP Anycast".to_string(),
            group: "Cloudflare WARP (WireGuard)".to_string(),
            tags: vec!["WARP".to_string(), "WireGuard".to_string()],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "private_key": priv_key,
                "public_key": pub_key,
                "ip": ip,
                "ipv6": ipv6,
                "reserved": [0, 0, 0],
                "mtu": 1280
            }),
        })
    }

    pub fn detect_country(name: &str) -> (String, String) {
        let name_upper = name.to_uppercase();
        let has_code = |code: &str| -> bool {
            name_upper.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w == code)
        };
        let matches = |zh: &str, en: &str, code: &str| -> bool {
            name.contains(zh) || name_upper.contains(en) || has_code(code)
        };

        if matches("香港", "HONG KONG", "HK") {
            ("HK".to_string(), "Hong Kong".to_string())
        } else if matches("台湾", "TAIWAN", "TW") || name.contains("台灣") {
            ("TW".to_string(), "Taiwan".to_string())
        } else if matches("日本", "JAPAN", "JP") || name_upper.contains("TOKYO") {
            ("JP".to_string(), "Japan".to_string())
        } else if matches("美国", "UNITED STATES", "US") || name.contains("美國") || has_code("USA") {
            ("US".to_string(), "United States".to_string())
        } else if matches("新加坡", "SINGAPORE", "SG") || name.contains("狮城") {
            ("SG".to_string(), "Singapore".to_string())
        } else if matches("韩国", "KOREA", "KR") || name_upper.contains("SOUTH KOREA") {
            ("KR".to_string(), "South Korea".to_string())
        } else if matches("英国", "UNITED KINGDOM", "GB") || name_upper.contains("BRITAIN") || has_code("UK") {
            ("GB".to_string(), "United Kingdom".to_string())
        } else if matches("德国", "GERMANY", "DE") {
            ("DE".to_string(), "Germany".to_string())
        } else if matches("法国", "FRANCE", "FR") {
            ("FR".to_string(), "France".to_string())
        } else if matches("澳洲", "AUSTRALIA", "AU") || name.contains("澳大利亚") {
            ("AU".to_string(), "Australia".to_string())
        } else if matches("加拿大", "CANADA", "CA") {
            ("CA".to_string(), "Canada".to_string())
        } else if matches("荷兰", "NETHERLANDS", "NL") {
            ("NL".to_string(), "Netherlands".to_string())
        } else if matches("印度", "INDIA", "IN") {
            ("IN".to_string(), "India".to_string())
        } else if matches("巴西", "BRAZIL", "BR") {
            ("BR".to_string(), "Brazil".to_string())
        } else if matches("俄罗斯", "RUSSIA", "RU") {
            ("RU".to_string(), "Russia".to_string())
        } else if matches("土耳其", "TURKEY", "TR") {
            ("TR".to_string(), "Turkey".to_string())
        } else if matches("意大利", "ITALY", "IT") {
            ("IT".to_string(), "Italy".to_string())
        } else if matches("西班牙", "SPAIN", "ES") {
            ("ES".to_string(), "Spain".to_string())
        } else if matches("瑞士", "SWITZERLAND", "CH") {
            ("CH".to_string(), "Switzerland".to_string())
        } else if matches("瑞典", "SWEDEN", "SE") {
            ("SE".to_string(), "Sweden".to_string())
        } else {
            ("".to_string(), "".to_string())
        }
    }

    fn parse_vless_link(link: &str) -> Result<UnifiedNode, String> {
        let parsed = Url::parse(link).map_err(|e| format!("Invalid URL: {}", e))?;
        let uuid = parsed.username();
        let host = parsed.host_str().ok_or("Missing host")?;
        let port = parsed.port().unwrap_or(443);
        let name = parsed.fragment().map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string())
            .unwrap_or_else(|| format!("VLESS-{}", host));

        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

        let security = query.get("security").cloned().unwrap_or_else(|| "none".to_string());
        let flow = query.get("flow").cloned();
        let sni = query.get("sni").cloned().or_else(|| query.get("peer").cloned());
        let public_key = query.get("pbk").cloned();
        let short_id = query.get("sid").cloned();
        let fingerprint = query.get("fp").cloned().unwrap_or_else(|| "chrome".to_string());

        let (country_code, country_name) = Self::detect_country(&name);

        // WS transport params the core MUST replay verbatim. Cheaper panels now
        // ship disguised paths (`/events?ed=2560`) behind Cloudflare ECH:
        // dropping path/host breaks the WS upgrade (404/RST), and without the
        // ECH public name the CDN TLS handshake silently hangs.
        let ws_path = query
            .get("path")
            .cloned()
            .or_else(|| query.get("ws-path").cloned());
        let ws_host = query
            .get("host")
            .cloned()
            .or_else(|| query.get("hostname").cloned())
            .or_else(|| query.get("ws-host").cloned());
        let ech = query.get("ech").cloned().map(|v| {
            // v2rayN format: "<public-name>+<resolver>" — sing-box queries the
            // HTTPS record itself, so only the name matters.
            v.split('+').next().unwrap_or("").to_string()
        });
        let insecure = matches!(
            query.get("allowInsecure").map(|s| s.as_str()).unwrap_or(""),
            "1" | "true"
        );

        let node = UnifiedNode {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: ProtocolType::Vless,
            address: host.to_string(),
            port,
            country_code,
            country_name,
            city: "".to_string(),
            group: "Imported".to_string(),
            tags: vec!["Imported".to_string()],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "uuid": uuid,
                "flow": flow,
                "security": security,
                "sni": sni,
                "public_key": public_key,
                "short_id": short_id,
                "fingerprint": fingerprint,
                "network": query.get("type").cloned().unwrap_or_else(|| "tcp".to_string()),
                "path": ws_path,
                "host": ws_host,
                "ech": ech,
                "insecure": insecure,
            }),
        };
        Ok(node)
    }

    fn parse_trojan_link(link: &str) -> Result<UnifiedNode, String> {
        let parsed = Url::parse(link).map_err(|e| format!("Invalid URL: {}", e))?;
        let password = parsed.username();
        let host = parsed.host_str().ok_or("Missing host")?;
        let port = parsed.port().unwrap_or(443);
        let name = parsed.fragment().map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string())
            .unwrap_or_else(|| format!("Trojan-{}", host));

        let query: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        let sni = query.get("sni").cloned().unwrap_or_else(|| host.to_string());
        let ws_path = query
            .get("path")
            .cloned()
            .or_else(|| query.get("ws-path").cloned());
        let ws_host = query
            .get("host")
            .cloned()
            .or_else(|| query.get("ws-host").cloned());
        let ech = query.get("ech").cloned().map(|v| v.split('+').next().unwrap_or("").to_string());
        let insecure = matches!(
            query.get("allowInsecure").map(|s| s.as_str()).unwrap_or(""),
            "1" | "true"
        );

        let (country_code, country_name) = Self::detect_country(&name);

        Ok(UnifiedNode {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: ProtocolType::Trojan,
            address: host.to_string(),
            port,
            country_code,
            country_name,
            city: "".to_string(),
            group: "Imported".to_string(),
            tags: vec!["Trojan".to_string()],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "password": password,
                "sni": sni,
                "network": query.get("type").cloned().unwrap_or_else(|| "tcp".to_string()),
                "path": ws_path,
                "host": ws_host,
                "ech": ech,
                "insecure": insecure,
            }),
        })
    }

    fn parse_socks5_link(link: &str) -> Result<UnifiedNode, String> {
        let parsed = Url::parse(link).map_err(|e| format!("Invalid URL: {}", e))?;
        let host = parsed.host_str().ok_or("Missing host")?;
        let port = parsed.port().unwrap_or(1080);
        let name = parsed.fragment().map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string())
            .unwrap_or_else(|| format!("SOCKS5-{}", host));

        let (country_code, country_name) = Self::detect_country(&name);

        Ok(UnifiedNode {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: ProtocolType::Socks5,
            address: host.to_string(),
            port,
            country_code,
            country_name,
            city: "".to_string(),
            group: "Imported".to_string(),
            tags: vec!["SOCKS5".to_string()],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "username": parsed.username(),
                "password": parsed.password()
            }),
        })
    }

    fn parse_ss_link(link: &str) -> Result<UnifiedNode, String> {
        let (raw_url, fragment) = match link.split_once('#') {
            Some((u, f)) => (u, Some(f)),
            None => (link, None),
        };

        let without_prefix = raw_url.trim_start_matches("ss://");
        let (user_part, host_port) = without_prefix
            .split_once('@')
            .ok_or_else(|| "Invalid ss:// link: missing '@'".to_string())?;

        let (host, port_str) = host_port
            .rsplit_once(':')
            .ok_or_else(|| "Invalid ss:// link: missing host/port".to_string())?;
        let port: u16 = port_str.parse().map_err(|e| format!("Invalid port: {}", e))?;

        // Decode user info if base64 encoded
        let (method, password) = if user_part.contains(':') {
            let (m, p) = user_part.split_once(':').unwrap();
            (m.to_string(), p.to_string())
        } else {
            let mut padded = user_part.to_string();
            while padded.len() % 4 != 0 {
                padded.push('=');
            };
            let decoded_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &padded)
                .or_else(|_| base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, &padded))
                .map_err(|e| format!("Failed to decode ss base64: {}", e))?;
            let decoded_str = String::from_utf8_lossy(&decoded_bytes).to_string();
            let (m, p) = decoded_str
                .split_once(':')
                .ok_or_else(|| "Invalid ss userinfo after decode".to_string())?;
            (m.to_string(), p.to_string())
        };

        let name = fragment
            .map(|f| urlencoding::decode(f).unwrap_or(f.into()).to_string())
            .unwrap_or_else(|| format!("SS-{}", host));

        let (country_code, country_name) = Self::detect_country(&name);

        Ok(UnifiedNode {
            id: Uuid::new_v4().to_string(),
            name,
            protocol: ProtocolType::Shadowsocks,
            address: host.to_string(),
            port,
            country_code,
            country_name,
            city: "".to_string(),
            group: "Imported".to_string(),
            tags: vec!["Shadowsocks".to_string()],
            favorite: false,
            latency_ms: None,
            speed_bps: None,
            last_checked: None,
            status: NodeStatus::Unknown,
            config: json!({
                "method": method,
                "password": password
            }),
        })
    }

    pub fn save(&self) -> Result<(), String> {
        let list = self.nodes.read().clone();
        let json_data = serde_json::to_string_pretty(&list)
            .map_err(|e| format!("Failed to serialize nodes: {}", e))?;
        if let Some(parent) = self.data_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&self.data_path, json_data)
            .map_err(|e| format!("Failed to write nodes.json: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vless_ws_link_keeps_transport_and_ech_params() {
        // Exact shape of the current freesubplus links (France 122): without
        // path/host/ech/fp the core can never speak to these nodes — v2rayN
        // parses all of them, our old parser dropped everything but uuid.
        let link = "vless://08ea5abf-fbe9-4c40-983d-099076942b93@104.252.111.38:8443?security=tls&type=ws&ech=cloudflare-ech.com%2Bhttps%3A%2F%2Fdns.alidns.com%2Fdns-query&host=forfreesub.tclucky.eu.cc&fp=chrome&sni=forfreesub.tclucky.eu.cc&path=%2Fevents%3Fed%3D2560&encryption=none#%F0%9F%87%AB%F0%9F%87%B7+France+122";
        let node = NodeManager::parse_vless_link(link).expect("parse vless link");
        assert_eq!(node.address, "104.252.111.38");
        assert_eq!(node.port, 8443);
        assert_eq!(node.config["path"], "/events?ed=2560");
        assert_eq!(node.config["host"], "forfreesub.tclucky.eu.cc");
        assert_eq!(node.config["sni"], "forfreesub.tclucky.eu.cc");
        assert_eq!(node.config["network"], "ws");
        assert_eq!(node.config["security"], "tls");
        assert_eq!(node.config["fingerprint"], "chrome");
        assert_eq!(node.config["ech"], "cloudflare-ech.com");
    }

    fn group_node(group: &str, id: &str, status: NodeStatus, latency: Option<i64>) -> UnifiedNode {
        UnifiedNode {
            id: id.to_string(),
            name: id.to_string(),
            protocol: ProtocolType::Openvpn,
            address: "example.com".to_string(),
            port: 443,
            country_code: "JP".to_string(),
            country_name: "Japan".to_string(),
            city: String::new(),
            group: group.to_string(),
            tags: vec![],
            favorite: false,
            latency_ms: latency,
            speed_bps: None,
            last_checked: None,
            status,
            config: json!({}),
        }
    }

    #[test]
    fn replacing_a_group_prunes_gone_servers_and_keeps_measured_ones() {
        let dir = std::env::temp_dir().join(format!("aura-nm-replace-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let manager = NodeManager::new(&dir);

        let measured = group_node("VPNGate", "keep-443", NodeStatus::Alive, Some(123));
        let stale = group_node("VPNGate", "gone-443", NodeStatus::Dead, None);
        let untouched = group_node("Residential", "res-1", NodeStatus::Unknown, None);
        manager.add_node(measured.clone()).unwrap();
        manager.add_node(stale).unwrap();
        manager.add_node(untouched.clone()).unwrap();

        // 重新采集:同一台服务器带着默认值回来,掉出来源的那台不在列表里
        let mut fresh = measured.clone();
        fresh.favorite = true;
        fresh.status = NodeStatus::Unknown;
        fresh.latency_ms = None;
        let merged = manager
            .replace_group(
                "VPNGate",
                vec![
                    fresh,
                    group_node("VPNGate", "new-80", NodeStatus::Unknown, None),
                ],
                GroupPrune::DropMissing,
            )
            .unwrap();

        let ids: Vec<String> = manager.get_all().into_iter().map(|n| n.id).collect();
        assert!(!ids.iter().any(|id| id.ends_with("gone-443")), "掉出来源的节点必须清掉");
        assert!(ids.contains(&untouched.id), "别的组不受影响");
        assert_eq!(merged.len(), 2, "返回的是这一组现在的情况");

        let kept = merged.iter().find(|n| n.id.ends_with("keep-443")).expect("kept node");
        assert_eq!(kept.latency_ms, Some(123), "重同步不该把测活结果抹平成未测");
        assert_eq!(kept.status, NodeStatus::Alive);
        assert!(kept.favorite, "新列表没有星标,旧的该保留");

        // 部分来源(镜像只给了几台):绝不能把没提到的服务器删掉
        let partial = manager
            .replace_group(
                "VPNGate",
                vec![group_node("VPNGate", "mirror-only-443", NodeStatus::Unknown, None)],
                GroupPrune::KeepMissing,
            )
            .unwrap();
        let after: Vec<String> = manager.get_all().into_iter().map(|n| n.id).collect();
        assert!(after.iter().any(|id| id.ends_with("new-80")), "缺信息时宁可不删");
        assert_eq!(partial.len(), 3, "keep-443 + new-80 + 镜像那台");

        // 同一批节点重复写入不能变成重复行
        let again = manager
            .replace_group(
                "VPNGate",
                vec![
                    group_node("VPNGate", "mirror-only-443", NodeStatus::Unknown, None),
                    group_node("VPNGate", "new-80", NodeStatus::Unknown, None),
                ],
                GroupPrune::KeepMissing,
            )
            .unwrap();
        assert_eq!(again.len(), 3, "重写已有节点是覆盖,不是追加");

        drop(manager);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
