//! Lane: a background, TUN-free mihomo instance used for three things that all
//! need "traffic that leaves through a relay without touching the system":
//! measuring relay quality, fetching GFW-blocked node lists through the best
//! relay, and doing real OpenVPN handshakes against candidate exit nodes.
//!
//! Why a second core instead of reusing the connect path: a lane never opens a
//! tunnel, never sets a system proxy and binds only 127.0.0.1, so a probe burst
//! cannot black-hole the device (the failure class v0.2.103 shipped with). It is
//! also the only way to test many OpenVPN endpoints without paying a
//! VpnService consent round-trip per node — mihomo's `type: openvpn` entries are
//! dialed lazily per request, so one process holds the whole candidate set.
//!
//! Verified behaviour this module depends on (measured against mihomo v1.19.30):
//!   * `GET /proxies/{name}/delay` does NOT answer for openvpn members, so
//!     liveness is decided by a real request through the mixed port instead;
//!   * `dialer-proxy` on an openvpn entry really does route the handshake through
//!     the named relay (the dial error switches from the node IP to the relay IP);
//!   * a failed openvpn dial returns in ~5s rather than hanging;
//!   * `PUT /proxies/PROBE {"name": x}` re-targets the next request;
//!   * a dead node costs ~2.5s end to end because the core retransmits the
//!     OpenVPN handshake (~10 attempts) before failing — so probing is serial
//!     per lane, and a 50-node batch is a few minutes of background work.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::json;

use crate::platform::hide_window_std;

/// Deliberately far from the connect path's 2080/9090 and the inspector's 30000+.
const LANE_PORT_BASE: u16 = 21980;
/// One openvpn handshake that never completes costs the whole batch this much.
pub const NODE_DIAL_TIMEOUT: Duration = Duration::from_secs(6);
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// HTTPS on purpose: nodes that only egress 443 RST plain-HTTP tunnels, and were
/// misrated by the inspector's old port-80 probe (see inspector_manager).
pub const PROBE_204_URL: &str = "https://cp.cloudflare.com/generate_204";
/// 2MB is enough to separate a 5Mbps link from a 500Mbps one inside ~10s.
pub const THROUGHPUT_URL: &str = "https://speed.cloudflare.com/__down?bytes=2000000";

fn lane_mixed_port(index: usize) -> u16 {
    LANE_PORT_BASE + (index as u16 * 2)
}

fn lane_api_port(index: usize) -> u16 {
    LANE_PORT_BASE + (index as u16 * 2) + 1
}

pub struct Lane {
    index: usize,
    app_data_dir: PathBuf,
    mixed_port: u16,
    api_port: u16,
    child: Option<Child>,
    /// mihomo holds exactly one selection per group, so two concurrent probes on
    /// one lane would retarget each other's requests. Callers take this guard.
    gate: tokio::sync::Mutex<()>,
    members: Vec<String>,
}

/// Outcome of one real handshake attempt at an exit node.
#[derive(Debug, Clone)]
pub struct NodeVerdict {
    pub alive: bool,
    pub latency_ms: Option<i64>,
}

fn controller_url(api_port: u16, path: &str) -> String {
    format!("http://127.0.0.1:{}{}", api_port, path)
}

/// DNS stays off. Turning it on makes mihomo demand its GeoIP MMDB and block
/// startup on a download (measured: the controller never comes up), and a lane
/// has no TUN client to answer for anyway — hostnames in proxy entries resolve
/// through the system resolver, the same as every other outbound here.
const LANE_DNS_BLOCK: &str = "dns:
  enable: false
";

/// Builds the lane config: `relay_yaml` (when the lane's traffic must hop through
/// a relay) plus `nodes` (name, proxy block) collected into one probe group.
/// `None` relay is used by the ranking lane, which dials its candidates head-on.
pub fn build_lane_config(
    index: usize,
    relay_yaml: Option<&str>,
    nodes: &[(String, String)],
) -> String {
    let mut names: Vec<String> = Vec::new();
    if relay_yaml.is_some() {
        names.push("relay".to_string());
    }
    names.extend(nodes.iter().map(|(n, _)| n.clone()));

    let mut proxies = match relay_yaml {
        Some(y) => y.trim_end().to_string(),
        None => String::new(),
    };
    for (_, block) in nodes {
        if !proxies.is_empty() {
            proxies.push('\n');
        }
        proxies.push_str(block.trim_end());
    }

    format!(
        r#"mixed-port: {port}
allow-lan: false
mode: rule
log-level: warning
system-proxy: false
external-controller: 127.0.0.1:{api}

{dns}
proxies:
{proxies}

proxy-groups:
  - name: PROBE
    type: select
    proxies: [{names}]

rules:
  - MATCH,PROBE
"#,
        port = lane_mixed_port(index),
        api = lane_api_port(index),
        dns = LANE_DNS_BLOCK.trim_end(),
        proxies = proxies,
        names = names.join(", ")
    )
}

impl Lane {
    /// Spawns the core and waits until its controller answers.
    pub fn start(
        index: usize,
        app_data_dir: &Path,
        binary: &Path,
        relay_yaml: Option<&str>,
        nodes: &[(String, String)],
    ) -> Result<Self, String> {
        let lane = Self {
            index,
            app_data_dir: app_data_dir.to_path_buf(),
            mixed_port: lane_mixed_port(index),
            api_port: lane_api_port(index),
            child: None,
            gate: tokio::sync::Mutex::new(()),
            members: nodes.iter().map(|(n, _)| n.clone()).collect(),
        };

        let config = build_lane_config(index, relay_yaml, nodes);
        let config_path = lane.config_path();
        std::fs::write(&config_path, &config).map_err(|e| format!("lane config write: {}", e))?;

        let log_file = std::fs::File::create(lane.log_path())
            .map_err(|e| format!("lane log open: {}", e))?;
        let err_file = log_file
            .try_clone()
            .map_err(|e| format!("lane log clone: {}", e))?;
        let mut cmd = Command::new(binary);
        cmd.arg("-d").arg(app_data_dir).arg("-f").arg(&config_path);
        hide_window_std(&mut cmd);
        let child = cmd
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .spawn()
            .map_err(|e| format!("lane spawn: {}", e))?;

        let mut lane = lane;
        lane.child = Some(child);
        if let Err(e) = lane.blocking_wait_ready() {
            lane.stop();
            let tail = lane.tail_log(20);
            return Err(format!("{} (lane log tail: {})", e, tail));
        }
        log::info!("Lane {}: controller up on :{}", lane.index, lane.api_port);
        Ok(lane)
    }

    pub fn index(&self) -> usize {
        self.index
    }

    fn config_path(&self) -> PathBuf {
        self.app_data_dir.join(format!("lane_{}.yaml", self.index))
    }

    fn log_path(&self) -> PathBuf {
        self.app_data_dir.join(format!("lane_{}.log", self.index))
    }

    fn tail_log(&self, lines: usize) -> String {
        match std::fs::read_to_string(self.log_path()) {
            Ok(s) => {
                let all: Vec<&str> = s.lines().collect();
                all[all.len().saturating_sub(lines)..].join(" | ")
            }
            Err(_) => "无日志".to_string(),
        }
    }

    fn http(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }

    fn blocking_wait_ready(&self) -> Result<(), String> {
        let deadline = std::time::Instant::now() + READY_TIMEOUT;
        while std::time::Instant::now() < deadline {
            let ok = std::net::TcpStream::connect(("127.0.0.1", self.api_port)).is_ok();
            if ok {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!(
            "lane {} controller did not listen on :{} within {}s",
            self.index,
            self.api_port,
            READY_TIMEOUT.as_secs()
        ))
    }

    /// Route the lane's next requests through one of its members.
    pub async fn select(&self, name: &str) -> Result<(), String> {
        let body = json!({ "name": name });
        let resp = self
            .http()
            .request(reqwest::Method::PUT, controller_url(self.api_port, "/proxies/PROBE"))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("lane select {}: {}", name, e))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("lane select {} rejected", name))
        }
    }

    /// Every member mihomo actually accepted into the probe group. If a node block
    /// is malformed the core either rejects the whole config or drops that member,
    /// so callers use this to catch a silently-short batch.
    pub async fn group_members(&self) -> Vec<String> {
        let v: serde_json::Value = match self
            .http()
            .get(controller_url(self.api_port, "/proxies/PROBE"))
            .send()
            .await
        {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(_) => return Vec::new(),
            },
            Err(_) => return Vec::new(),
        };
        match v.get("all").and_then(|a| a.as_array()) {
            Some(items) => items
                .iter()
                .filter_map(|i| i.as_str().map(|s| s.to_string()))
                .collect(),
            None => Vec::new(),
        }
    }

    pub async fn current_selection(&self) -> Option<String> {
        let v: serde_json::Value = self
            .http()
            .get(controller_url(self.api_port, "/proxies/PROBE"))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        v.get("now").and_then(|s| s.as_str()).map(|s| s.to_string())
    }

    fn proxy_client(&self, timeout: Duration) -> Result<reqwest::Client, String> {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", self.mixed_port))
            .map_err(|e| e.to_string())?;
        reqwest::Client::builder()
            .proxy(proxy)
            .timeout(timeout)
            .build()
            .map_err(|e| e.to_string())
    }

    /// A GET that leaves through the currently selected member. Returns the status
    /// code; any transport-level failure is an error string (which for an openvpn
    /// member means the real handshake did not complete).
    pub async fn get_via_lane(&self, url: &str, timeout: Duration) -> Result<reqwest::Response, String> {
        let client = self.proxy_client(timeout)?;
        client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("{}", e))
    }

    /// Measure a member: real handshake + one 204 round trip.
    pub async fn measure_member(&self, name: &str, url: &str) -> Option<i64> {
        let _guard = self.gate.lock().await;
        if self.select(name).await.is_err() {
            return None;
        }
        let client = self.proxy_client(NODE_DIAL_TIMEOUT).ok()?;
        let start = std::time::Instant::now();
        match client.get(url).send().await {
            Ok(r) if r.status().as_u16() < 500 => Some(start.elapsed().as_millis() as i64),
            _ => None,
        }
    }

    /// Download throughput of a member, in bytes/sec (None if it cannot carry bulk).
    pub async fn measure_member_throughput(&self, name: &str, url: &str) -> Option<u64> {
        let _guard = self.gate.lock().await;
        self.select(name).await.ok()?;
        let client = self.proxy_client(Duration::from_secs(12)).ok()?;
        let start = std::time::Instant::now();
        let resp = client.get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let bytes = resp.bytes().await.ok()?;
        let secs = start.elapsed().as_secs_f64();
        if secs <= 0.05 || bytes.len() < 65536 {
            return None;
        }
        Some((bytes.len() as f64 / secs) as u64)
    }

    /// The whole point of the lane: decide whether `node` can carry traffic right
    /// now, by actually dialling it through the relay.
    pub async fn test_node(&self, name: &str, probe_url: &str) -> NodeVerdict {
        match self.measure_member(name, probe_url).await {
            Some(ms) => NodeVerdict { alive: true, latency_ms: Some(ms.max(1)) },
            None => NodeVerdict { alive: false, latency_ms: None },
        }
    }

    /// Fetch a GFW-blocked document through the currently selected member
    /// (callers `select("relay")` first).
    pub async fn fetch_through(&self, url: &str, timeout: Duration) -> Result<String, String> {
        let _guard = self.gate.lock().await;
        let client = self.proxy_client(timeout)?;
        let resp = client.get(url).send().await.map_err(|e| format!("{}", e))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("HTTP {}", status.as_u16()));
        }
        resp.text().await.map_err(|e| format!("{}", e))
    }

    pub fn members(&self) -> &[String] {
        &self.members
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Owns the lanes the pipeline is allowed to run at once. Mobile is held to one
/// process: each lane is a full mihomo instance carrying dozens of node configs.
pub struct LanePool {
    lanes: Vec<Lane>,
    max_lanes: usize,
}

impl LanePool {
    pub fn new(max_lanes: usize) -> Self {
        Self { lanes: Vec::new(), max_lanes }
    }

    pub fn len(&self) -> usize {
        self.lanes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
    }

    /// Start one lane; `index` picks its port pair, so callers must keep indices
    /// distinct (the pool hands them out in order).
    pub fn spawn(
        &mut self,
        app_data_dir: &Path,
        binary: &Path,
        relay_yaml: Option<&str>,
        nodes: &[(String, String)],
    ) -> Result<usize, String> {
        if self.lanes.len() >= self.max_lanes {
            return Err(format!("lane pool saturated ({} lanes)", self.max_lanes));
        }
        let index = self.lanes.len();
        let lane = Lane::start(index, app_data_dir, binary, relay_yaml, nodes)?;
        self.lanes.push(lane);
        Ok(index)
    }

    pub fn get(&self, index: usize) -> Option<&Lane> {
        self.lanes.get(index)
    }

    pub fn stop_all(&mut self) {
        for mut lane in self.lanes.drain(..) {
            lane.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_block(name: &str, host: &str, port: u16) -> String {
        format!(
r#"  - name: {}
    type: openvpn
    server: {}
    port: {}
    proto: tcp
    dialer-proxy: relay
    udp: true
    username: vpn
    password: vpn
    cipher: AES-128-CBC
    auth: SHA1
    ca: |
      -----BEGIN CERTIFICATE-----
      AAA
      -----END CERTIFICATE-----"#,
            name, host, port
        )
    }

    #[test]
    fn lane_config_lists_relay_then_nodes_in_one_group() {
        let relay = "  - name: relay\n    type: ss\n    server: 1.2.3.4\n    port: 8443";
        let nodes = vec![
            ("n0".to_string(), node_block("n0", "5.6.7.8", 443)),
            ("n1".to_string(), node_block("n1", "9.10.11.12", 1194)),
        ];
        let cfg = build_lane_config(0, Some(relay), &nodes);

        assert!(cfg.contains("system-proxy: false"), "lane must never touch system proxy");
        assert!(cfg.contains("mode: rule"));
        assert!(cfg.contains("dns:\n  enable: false"), "lane DNS must stay off");
        assert!(cfg.contains(&format!("mixed-port: {}", LANE_PORT_BASE)));
        assert!(cfg.contains(&format!("external-controller: 127.0.0.1:{}", LANE_PORT_BASE + 1)));
        assert!(cfg.contains("proxies: [relay, n0, n1]"));
        // second lane gets its own port pair, so lanes never fight over listeners
        assert!(build_lane_config(1, Some(relay), &[]).contains(&format!("mixed-port: {}", LANE_PORT_BASE + 2)));
    }

    #[test]
    fn ranking_lane_dials_its_candidates_head_on() {
        // No relay hop means no phantom "relay" member the ranker could select.
        let nodes = vec![("c0".to_string(), "  - name: c0\n    type: ss\n    server: 1.2.3.4\n    port: 1".to_string())];
        let cfg = build_lane_config(3, None, &nodes);
        assert!(cfg.contains("proxies: [c0]"), "{}", cfg);
        assert!(!cfg.contains("relay"));
        assert!(!cfg.contains("dialer-proxy"));
    }

    #[tokio::test]
    async fn test_node_reports_dead_when_dial_fails() {
        // No core is running on these ports, so the dial cannot succeed: the
        // verdict must be "dead", never "unknown-but-pass".
        let lane = Lane {
            index: 7,
            app_data_dir: std::env::temp_dir(),
            mixed_port: lane_mixed_port(7),
            api_port: lane_api_port(7),
            child: None,
            gate: tokio::sync::Mutex::new(()),
            members: vec!["n0".to_string()],
        };
        let verdict = lane.test_node("n0", "http://cp.cloudflare.com/generate_204").await;
        assert!(!verdict.alive);
        assert_eq!(verdict.latency_ms, None);
    }

    /// End-to-end proof of the two things the whole liveness feature rests on,
    /// against the real core binary: a 50-entry openvpn lane is accepted (the
    /// resource question the user raised for mobile), and a document really does
    /// come back through the relay hop rather than a direct socket.
    ///
    /// Run with: cargo test --manifest-path src-tauri/Cargo.toml lane_end_to_end -- --ignored
    #[tokio::test]
    #[ignore]
    async fn lane_end_to_end_carries_50_nodes_and_fetches_through_relay() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::{TcpListener, TcpStream};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        use crate::managers::connection_manager::ConnectionManager;
        use crate::models::{NodeStatus, ProtocolType, UnifiedNode};

        let binary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(crate::platform::core_binary_name("mihomo"));
        if !binary.exists() {
            eprintln!("SKIP: {} not present", binary.display());
            return;
        }

        const OVPN_CA: &str = "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4\n-----END CERTIFICATE-----";

        // ---- fake relay. mihomo's `type: http` upstream tunnels everything with
        // CONNECT, so this answers CONNECT: port 80 becomes a working tunnel that
        // serves the canned list document (proof the bytes came through the relay),
        // any other port is accepted then dropped (proof a failed openvpn hop is
        // reported dead instead of hanging).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let relay_port = listener.local_addr().unwrap().port();
        let served_lists = Arc::new(AtomicUsize::new(0));
        let node_hops = Arc::new(AtomicUsize::new(0));
        let (lists, hops) = (served_lists.clone(), node_hops.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let (lists, hops) = (lists.clone(), hops.clone());
                std::thread::spawn(move || {
                    let _ = handle_fake_relay(stream, lists, hops);
                });
            }
        });

        const LIST_BODY: &str = "#hostname,servername,port_443_ltd,hmm\n";

        fn serve_origin(stream: &mut TcpStream) -> std::io::Result<()> {
            stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    LIST_BODY.len(),
                    LIST_BODY
                )
                .as_bytes(),
            )?;
            Ok(())
        }

        fn handle_fake_relay(
            mut stream: TcpStream,
            served_lists: Arc<AtomicUsize>,
            node_hops: Arc<AtomicUsize>,
        ) -> std::io::Result<()> {
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut reader = BufReader::new(stream.try_clone()?);
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                return Ok(());
            }
            loop {
                let mut scratch = String::new();
                if reader.read_line(&mut scratch)? == 0 {
                    break;
                }
                if scratch.trim().is_empty() {
                    break;
                }
            }

            let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
            if line.starts_with("CONNECT") {
                if target.ends_with(":80") {
                    stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
                    // the real request now arrives inside the tunnel
                    let mut inner = String::new();
                    let _ = reader.read_line(&mut inner);
                    served_lists.fetch_add(1, Ordering::SeqCst);
                    serve_origin(&mut stream)?;
                } else {
                    node_hops.fetch_add(1, Ordering::SeqCst);
                    stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
                    // drop it: the exit node behind this hop is unreachable
                }
            } else {
                served_lists.fetch_add(1, Ordering::SeqCst);
                serve_origin(&mut stream)?;
            }
            Ok(())
        }

        let relay_yaml = format!(
            "  - name: relay\n    type: http\n    server: 127.0.0.1\n    port: {}\n",
            relay_port
        );

        // ---- 50 real openvpn blocks, produced by the same function the connect
        // path uses, so the lane config and the connect config cannot drift.
        let nodes: Vec<(String, String)> = (0..50)
            .map(|i| {
                let node = UnifiedNode {
                    id: format!("vpngate-10-0-0-{}-443", i),
                    name: format!("probe {}", i),
                    protocol: ProtocolType::Openvpn,
                    address: format!("10.0.0.{}", i + 1),
                    port: 443,
                    country_code: "JP".to_string(),
                    country_name: "Japan".to_string(),
                    city: String::new(),
                    group: "VPNGate".to_string(),
                    tags: vec![],
                    favorite: false,
                    latency_ms: None,
                    speed_bps: None,
                    last_checked: None,
                    status: NodeStatus::Unknown,
                    config: json!({ "ca": OVPN_CA, "proto": "udp" }),
                };
                let name = format!("n{}", i);
                let block =
                    ConnectionManager::mihomo_openvpn_proxy_block(&node, &name, Some("relay"));
                (name, block)
            })
            .collect();

        let dir = std::env::temp_dir().join("aura-lane-e2e");
        std::fs::create_dir_all(&dir).unwrap();
        let bin = binary.clone();
        let relay = relay_yaml.clone();
        let lane_dir = dir.clone();
        let lane = tokio::task::spawn_blocking(move || {
            Lane::start(9, &lane_dir, &bin, Some(relay.as_str()), &nodes)
        })
        .await
        .unwrap()
        .expect("lane should start with 50 openvpn members");

        // relay + all 50 nodes survived config parsing
        let all = lane.group_members().await;
        assert_eq!(
            all.len(),
            51,
            "core dropped members: got {:?} (+relay)",
            all.iter().filter(|n| **n != "relay").count()
        );

        // a blocked list document is reachable only if it left through the relay
        lane.select("relay").await.unwrap();
        let doc = lane
            .fetch_through("http://www.vpngate.net/api/iphone/", Duration::from_secs(8))
            .await
            .expect("fetch_through must succeed via the relay hop");
        assert!(doc.starts_with("#hostname"), "unexpected body: {}", doc);
        assert_eq!(served_lists.load(Ordering::SeqCst), 1);
        assert_eq!(node_hops.load(Ordering::SeqCst), 0);

        // and a node that cannot handshake is reported dead, quickly, not hanging
        let started = std::time::Instant::now();
        let verdict = lane
            .test_node("n0", "http://cp.cloudflare.com/generate_204")
            .await;
        assert!(!verdict.alive, "a dropped tunnel must not read as alive");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "dead node took {:?} to reject",
            started.elapsed()
        );
        // the openvpn handshake really did leave through the relay hop. The count
        // is above 1 because mihomo's openvpn dialer retransmits the handshake
        // before giving up (measured here: 10 attempts per dead node) — that is the
        // per-node cost a probe batch pays, which is why a lane probes serially.
        assert!(
            node_hops.load(Ordering::SeqCst) >= 1,
            "node dial never reached the relay"
        );

        // ---- the ranking lane: candidates dialled head-on, no relay hop, which is
        // how a candidate relay gets a real RTT before anyone trusts it.
        let candidate = format!(
            "  - name: c0\n    type: http\n    server: 127.0.0.1\n    port: {}",
            relay_port
        );
        let rank_nodes = vec![("c0".to_string(), candidate)];
        let dir2 = dir.clone();
        let bin2 = binary.clone();
        let rank = tokio::task::spawn_blocking(move || {
            Lane::start(8, &dir2, &bin2, None, &rank_nodes)
        })
        .await
        .unwrap()
        .expect("ranking lane starts without a relay entry");

        assert_eq!(rank.group_members().await, vec!["c0".to_string()]);
        let rtt = rank
            .measure_member("c0", "http://www.vpngate.net/api/iphone/")
            .await
            .expect("candidate behind a working tunnel must measure an RTT");
        assert!(rtt > 0, "rtt was {}", rtt);
        // both lanes drop here, and Drop is what reaps the child processes
    }
}
