use crate::core::{CoreAdapter, SingBoxAdapter};
use crate::models::{AppSettings, ConnectionStatus, ProxyMode, TrafficStats, UnifiedNode};
use crate::platform::{core_binary_name, hide_window_std, PlatformProxy, ProcessGuard};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

pub struct ConnectionManager {
    status: Arc<Mutex<ConnectionStatus>>,
    connected_node: Arc<Mutex<Option<UnifiedNode>>>,
    process: Arc<Mutex<Option<Child>>>,
    aether_process: Arc<Mutex<Option<Child>>>,
    psiphon_process: Arc<Mutex<Option<Child>>>,
    relay_process: Arc<Mutex<Option<Child>>>,
    tunrelay_process: Arc<Mutex<Option<Child>>>,
    job_guard: Option<ProcessGuard>,
    app_data_dir: PathBuf,
    is_traffic_running: Arc<AtomicBool>,
    connect_time: Arc<Mutex<Option<Instant>>>,
    connected_chain: Arc<Mutex<Option<String>>>,
    connect_generation: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn force_kill_all_cores() {
        let binaries = [
            "sing-box",
            "mihomo",
            "aether",
            "psiphon-tunnel-core",
            "tunrelay",
        ];
        #[cfg(windows)]
        {
            for bin in &binaries {
                let bin_exe = format!("{}.exe", bin);
                let mut cmd = Command::new("taskkill");
                cmd.args(&["/F", "/T", "/IM", &bin_exe]);
                hide_window_std(&mut cmd);
                let _ = cmd.output();
            }
        }
        #[cfg(all(unix, not(target_os = "android")))]
        {
            for bin in &binaries {
                let _ = Command::new("pkill")
                    .args(&["-9", "-f", bin])
                    .output();
            }
        }
        #[cfg(target_os = "android")]
        {
            // No pkill on Android; scan /proc (our own uid's processes are
            // visible) and SIGKILL orphaned cores from a previous crash.
            if let Ok(entries) = std::fs::read_dir("/proc") {
                for entry in entries.flatten() {
                    let Some(pid) = entry
                        .file_name()
                        .to_string_lossy()
                        .parse::<i32>()
                        .ok()
                    else {
                        continue;
                    };
                    let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
                        continue;
                    };
                    let cmdline = String::from_utf8_lossy(&cmdline).replace('\0', " ");
                    if binaries
                        .iter()
                        .any(|b| cmdline.contains(b) || cmdline.contains(&format!("lib{}.so", b)))
                    {
                        unsafe {
                            libc::kill(pid, libc::SIGKILL);
                        }
                    }
                }
            }
        }
    }

    async fn wait_for_port_release(port: u16, timeout_ms: u64) -> bool {
        let addr = std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), port);
        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            match std::net::TcpListener::bind(addr) {
                Ok(listener) => {
                    drop(listener);
                    return true;
                }
                Err(_) => {
                    if start.elapsed().as_millis() > 300 {
                        Self::force_kill_all_cores();
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
        false
    }

    pub fn new(app_data_dir: &Path) -> Self {
        let job_guard = ProcessGuard::new().ok();
        Self::force_kill_all_cores();
        Self {
            status: Arc::new(Mutex::new(ConnectionStatus::Disconnected)),
            connected_node: Arc::new(Mutex::new(None)),
            process: Arc::new(Mutex::new(None)),
            aether_process: Arc::new(Mutex::new(None)),
            psiphon_process: Arc::new(Mutex::new(None)),
            relay_process: Arc::new(Mutex::new(None)),
            tunrelay_process: Arc::new(Mutex::new(None)),
            job_guard,
            app_data_dir: app_data_dir.to_path_buf(),
            is_traffic_running: Arc::new(AtomicBool::new(false)),
            connect_time: Arc::new(Mutex::new(None)),
            connected_chain: Arc::new(Mutex::new(None)),
            connect_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn get_status(&self) -> ConnectionStatus {
        self.status.lock().clone()
    }

    pub fn get_connected_node(&self) -> Option<UnifiedNode> {
        self.connected_node.lock().clone()
    }

    pub fn get_connected_chain(&self) -> Option<String> {
        self.connected_chain.lock().clone()
    }

    pub async fn connect(
        &self,
        node: UnifiedNode,
        relay_node: Option<UnifiedNode>,
        settings: AppSettings,
        app: AppHandle,
    ) -> Result<(), String> {
        // Disconnect any existing session first
        let _ = self.disconnect(app.clone()).await;
        let current_gen = self.connect_generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.connected_chain.lock() = None;
        Self::wait_for_port_release(settings.mixed_port, 2000).await;
        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            return Err("Connection cancelled by user".to_string());
        }

        self.ensure_rules_deployed(&app);

        *self.status.lock() = ConnectionStatus::Connecting;
        *self.connected_node.lock() = Some(node.clone());
        let _ = app.emit("core:status-changed", ConnectionStatus::Connecting);

        // Specialized handling for OpenVPN (VPNGate SoftEther / Residential) via Mihomo core
        if node.protocol == crate::models::ProtocolType::Openvpn {
            let binary_path = self.locate_mihomo(&app)?;
            let config_str = Self::generate_mihomo_openvpn_config(&node, relay_node.as_ref(), &settings);
            let config_path = self.app_data_dir.join("current_config.yaml");
            std::fs::write(&config_path, config_str)
                .map_err(|e| format!("Failed to write core config file: {}", e))?;

            let log_file_path = self.app_data_dir.join("mihomo.log");
            let log_out = std::fs::File::create(&log_file_path)
                .map_err(|e| format!("Failed to create log file: {}", e))?;
            let log_err = log_out
                .try_clone()
                .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

            let mut cmd = Command::new(&binary_path);
            cmd.arg("-f")
                .arg(&config_path)
                .current_dir(&self.app_data_dir)
                .stdout(std::process::Stdio::from(log_out))
                .stderr(std::process::Stdio::from(log_err));
            hide_window_std(&mut cmd);

            let child = cmd
                .spawn()
                .map_err(|e| format!("Failed to start mihomo process: {}", e))?;

            if let Some(ref guard) = self.job_guard {
                let _ = guard.assign_process(&child);
            }

            *self.process.lock() = Some(child);

            let is_residential = node.group == "Residential";
            let initial_wait = if is_residential {
                3500
            } else if relay_node.is_some() {
                2000
            } else {
                800
            };
            tokio::time::sleep(std::time::Duration::from_millis(initial_wait)).await;
            if self.connect_generation.load(Ordering::SeqCst) != current_gen {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                return Err("Connection cancelled by user".to_string());
            }

            {
                let mut proc_lock = self.process.lock();
                if let Some(ref mut c) = *proc_lock {
                    if let Ok(Some(exit_status)) = c.try_wait() {
                        let err_log = std::fs::read_to_string(&log_file_path).unwrap_or_default();
                        let _ = PlatformProxy::disable_proxy();
                        *self.status.lock() = ConnectionStatus::Error;
                        *self.connected_node.lock() = None;
                        let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                        *proc_lock = None;
                        return Err(format!(
                            "Mihomo OpenVPN startup failed ({}):\n{}",
                            exit_status,
                            err_log.trim()
                        ));
                    }
                }
            }

            log::info!("Probing real internet connectivity through proxy port {}...", settings.mixed_port);
            if let Err(probe_err) = Self::verify_internet_connectivity(
                settings.mixed_port,
                is_residential,
                current_gen,
                Arc::clone(&self.connect_generation),
            ).await {
                if self.connect_generation.load(Ordering::SeqCst) != current_gen || probe_err.contains("cancelled") {
                    if let Some(mut c) = self.process.lock().take() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    Self::force_kill_all_cores();
                    let _ = PlatformProxy::disable_proxy();
                    log::info!("Connection aborted by user during OpenVPN probe");
                    return Err("Connection cancelled by user".to_string());
                }
                log::warn!("Internet connectivity verification failed: {}", probe_err);
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                *self.status.lock() = ConnectionStatus::Error;
                *self.connected_node.lock() = None;
                let _ = app.emit("core:status-changed", ConnectionStatus::Error);

                let node_kind = if is_residential { "优质住宅IP" } else { "VPNGate" };
                return Err(format!(
                    "外网连通性验证失败：该 {} 节点无法转发国际互联网流量。\n已自动断开以防浏览器无法上网。请切换其他低延迟节点或更换中转节点！\n详细原因: {}",
                    node_kind,
                    probe_err
                ));
            }

            if self.connect_generation.load(Ordering::SeqCst) != current_gen {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                return Err("Connection cancelled by user".to_string());
            }

            *self.connect_time.lock() = Some(Instant::now());

            if settings.proxy_mode == ProxyMode::SystemProxy {
                if let Err(e) = PlatformProxy::enable_proxy(settings.mixed_port) {
                    log::error!("Failed to enable Windows system proxy: {}", e);
                }
            }

            *self.status.lock() = ConnectionStatus::Connected;
            let _ = app.emit("core:status-changed", ConnectionStatus::Connected);
            self.start_traffic_monitor(app.clone(), settings.clash_api_port);
            return Ok(());
        }

        // 0a. If protocol is Psiphon, launch psiphon-tunnel-core.exe helper
        if node.protocol == crate::models::ProtocolType::Psiphon {
            let psiphon_bin = self.locate_psiphon(&app)?;
            let server_entries_path = self.locate_server_entries(&app)?;
            let egress_region = node.config.get("egress_region")
                .and_then(|v| v.as_str())
                .unwrap_or("US")
                .to_string();

            // If relay is specified, launch local upstream socks proxy on 127.0.0.1:1828
            let mut upstream_proxy_url: Option<String> = None;
            if let Some(ref relay) = relay_node {
                let sing_box_bin = self.locate_sing_box(&app)?;
                if let Ok(relay_outbound) = SingBoxAdapter::new().build_outbound(relay) {
                    let relay_cfg = serde_json::json!({
                        "log": { "level": "warn" },
                        "dns": {
                            "servers": [
                                { "tag": "dns-direct", "type": "udp", "server": "223.5.5.5" }
                            ]
                        },
                        "inbounds": [{
                            "type": "socks",
                            "tag": "socks-in",
                            "listen": "127.0.0.1",
                            "listen_port": 1828
                        }],
                        "outbounds": [
                            relay_outbound,
                            { "type": "direct", "tag": "direct" }
                        ],
                        "route": {
                            "default_domain_resolver": "dns-direct",
                            "rules": [
                                { "inbound": ["socks-in"], "outbound": "proxy" }
                            ]
                        }
                    });
                    let relay_cfg_path = self.app_data_dir.join("psiphon_relay.json");
                    let _ = std::fs::write(&relay_cfg_path, relay_cfg.to_string());
                    let mut rcmd = Command::new(&sing_box_bin);
                    rcmd.arg("run").arg("-c").arg(&relay_cfg_path);
                    hide_window_std(&mut rcmd);
                    if let Ok(rchild) = rcmd.spawn() {
                        if let Some(ref guard) = self.job_guard {
                            let _ = guard.assign_process(&rchild);
                        }
                        *self.relay_process.lock() = Some(rchild);
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                        upstream_proxy_url = Some("socks5://127.0.0.1:1828".to_string());
                    }
                }
            }

            // Write psiphon config JSON to app_data_dir with full anti-censorship parameters
            let mut psiphon_cfg = serde_json::json!({
                "PropagationChannelId": "FFFFFFFFFFFFFFFF",
                "SponsorId": "1111111111111111",
                "EstablishTunnelTimeoutSeconds": 0,
                "DataRootDirectory": self.app_data_dir.to_string_lossy().replace('\\', "/"),
                "LocalSocksProxyPort": 1820,
                "LocalHttpProxyPort": 1821,
                "DisableLocalSocksProxy": false,
                "DisableLocalHTTPProxy": false,
                "RemoteServerListSignaturePublicKey": "MIICIDANBgkqhkiG9w0BAQEFAAOCAg0AMIICCAKCAgEAt7Ls+/39r+T6zNW7GiVpJfzq/xvL9SBH5rIFnk0RXYEYavax3WS6HOD35eTAqn8AniOwiH+DOkvgSKF2caqk/y1dfq47Pdymtwzp9ikpB1C5OfAysXzBiwVJlCdajBKvBZDerV1cMvRzCKvKwRmvDmHgphQQ7WfXIGbRbmmk6opMBh3roE42KcotLFtqp0RRwLtcBRNtCdsrVsjiI1Lqz/lH+T61sGjSjQ3CHMuZYSQJZo/KrvzgQXpkaCTdbObxHqb6/+i1qaVOfEsvjoiyzTxJADvSytVtcTjijhPEV6XskJVHE1Zgl+7rATr/pDQkw6DPCNBS1+Y6fy7GstZALQXwEDN/qhQI9kWkHijT8ns+i1vGg00Mk/6J75arLhqcodWsdeG/M/moWgqQAnlZAGVtJI1OgeF5fsPpXu4kctOfuZlGjVZXQNW34aOzm8r8S0eVZitPlbhcPiR4gT/aSMz/wd8lZlzZYsje/Jr8u/YtlwjjreZrGRmG8KMOzukV3lLmMppXFMvl4bxv6YFEmIuTsOhbLTwFgh7KYNjodLj/LsqRVfwz31PgWQFTEPICV7GCvgVlPRxnofqKSjgTWI4mxDhBpVcATvaoBl1L/6WLbFvBsoAUBItWwctO2xalKxF5szhGm8lccoc5MZr8kfE0uxMgsxz4er68iCID+rsCAQM=",
                "ServerEntrySignaturePublicKeys": [
                    "HuUVTWaRyh5pZwy4UguSgkwmBe0EHtJJkoF5WrxmvA="
                ],
                "ExchangeObfuscationKey": "DpXzloJk1Hw6aSzmKKky0xcahsEHubch81Mi6K0XMlU=",
                "ConnectionWorkerPoolSize": 16,
                "DNSResolverPreferredAlternateServers": ["1.1.1.1:53", "9.9.9.9:53", "8.8.8.8:53"],
                "DNSResolverPreferAlternateServerProbability": 0.8,
                "EstablishTunnelServerAffinityGracePeriodMilliseconds": 300000,
                "EgressRegion": egress_region
            });
            if let Some(proxy_url) = upstream_proxy_url {
                psiphon_cfg["UpstreamProxyURL"] = serde_json::json!(proxy_url);
            }
            let psiphon_cfg_path = self.app_data_dir.join("psiphon_config.json");
            std::fs::write(&psiphon_cfg_path, psiphon_cfg.to_string())
                .map_err(|e| format!("Failed to write psiphon config: {}", e))?;

            let psiphon_log_path = self.app_data_dir.join("psiphon.log");
            let plog_out = std::fs::File::create(&psiphon_log_path)
                .map_err(|e| format!("Failed to create psiphon log: {}", e))?;
            let plog_err = plog_out.try_clone()
                .map_err(|e| format!("Failed to clone psiphon log handle: {}", e))?;

            // Prepare active server entries, ensuring restricted provider ID (66FA5AFE5F394DE1) is mapped
            // to active unrestricted provider ID (6F1EC65541D53546) so Japan and other regions never get skipped.
            let active_server_entries = self.app_data_dir.join("active_server_entries.txt");
            if let Ok(raw_entries) = std::fs::read_to_string(&server_entries_path) {
                let sanitized = raw_entries
                    .replace("36364641354146453546333934444531", "36463145433635353431443533353436")
                    .replace("66FA5AFE5F394DE1", "6F1EC65541D53546");
                let _ = std::fs::write(&active_server_entries, sanitized);
            }
            let entries_arg = if active_server_entries.exists() {
                active_server_entries
            } else {
                server_entries_path
            };

            let mut pcmd = Command::new(&psiphon_bin);
            pcmd.arg("-config").arg(&psiphon_cfg_path)
                .arg("-serverList").arg(&entries_arg)
                .arg("-dataRootDirectory").arg(&self.app_data_dir)
                .arg("-formatNotices")
                .stdout(std::process::Stdio::from(plog_out))
                .stderr(std::process::Stdio::from(plog_err));
            hide_window_std(&mut pcmd);

            let pchild = pcmd.spawn()
                .map_err(|e| format!("Failed to start psiphon-tunnel-core: {}", e))?;
            if let Some(ref guard) = self.job_guard {
                let _ = guard.assign_process(&pchild);
            }
            *self.psiphon_process.lock() = Some(pchild);

            // Wait up to 45s for psiphon tunnel to establish (checking log for real active tunnel)
            log::info!("Waiting for psiphon-tunnel-core tunnel to establish on 127.0.0.1:1820...");
            let mut psiphon_ready = false;
            for i in 0..90 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if let Ok(log_content) = std::fs::read_to_string(&psiphon_log_path) {
                    if log_content.contains("\"ActiveTunnel\"")
                        || log_content.contains("ConnectedServerRegion")
                        || log_content.contains("\"count\":1")
                        || (log_content.contains("\"Tunnels\"") && !log_content.contains("\"count\":0"))
                    {
                        psiphon_ready = true;
                        log::info!("Psiphon tunnel connected after {}ms", (i + 1) * 500);
                        break;
                    }
                }
            }
            if !psiphon_ready {
                let last_log = std::fs::read_to_string(&psiphon_log_path).unwrap_or_default();
                let snippet = last_log.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                if let Some(mut pproc) = self.psiphon_process.lock().take() {
                    let _ = pproc.kill();
                    let _ = pproc.wait();
                }
                if let Some(mut rproc) = self.relay_process.lock().take() {
                    let _ = rproc.kill();
                    let _ = rproc.wait();
                }
                let _ = PlatformProxy::disable_proxy();
                *self.status.lock() = ConnectionStatus::Error;
                *self.connected_node.lock() = None;
                let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                return Err(format!(
                    "Psiphon 隧道建立超时（未能与出口服务器建立加密连接）。\n建议：如果直连受阻，可在上方中转栏中选择一个低延迟订阅节点作为中转加速，即可 100% 成功建联！\n诊断日志:\n{}",
                    snippet
                ));
            }
        }

        // 0b. If protocol is MASQUE, launch aether helper daemon to establish local SOCKS5 upstream (127.0.0.1:1819)
        if node.protocol == crate::models::ProtocolType::Masque {
            let aether_bin = self.locate_aether(&app)?;
            let aether_log_path = self.app_data_dir.join("aether.log");
            let alog_out = std::fs::File::create(&aether_log_path)
                .map_err(|e| format!("Failed to create aether log: {}", e))?;
            let alog_err = alog_out
                .try_clone()
                .map_err(|e| format!("Failed to clone aether log handle: {}", e))?;

            let mut acmd = Command::new(&aether_bin);
            acmd.arg("--protocol")
                .arg("masque")
                .arg("--peer")
                .arg(format!("{}:{}", node.address, node.port))
                .arg("--bind")
                .arg("127.0.0.1:1819")
                .arg("--noize")
                .arg("firewall")
                .arg("--no-data-check")
                .current_dir(&self.app_data_dir)
                .stdout(std::process::Stdio::from(alog_out))
                .stderr(std::process::Stdio::from(alog_err));
            hide_window_std(&mut acmd);

            let achild = acmd
                .spawn()
                .map_err(|e| format!("Failed to start aether MASQUE core: {}", e))?;

            if let Some(ref guard) = self.job_guard {
                let _ = guard.assign_process(&achild);
            }
            *self.aether_process.lock() = Some(achild);

            // Wait for aether to expose SOCKS5 port 1819 (up to 3s, breaking immediately when ready)
            let mut port_ready = false;
            for _ in 0..15 {
                if std::net::TcpStream::connect("127.0.0.1:1819").is_ok() {
                    port_ready = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            if !port_ready {
                log::warn!("MASQUE SOCKS5 port 127.0.0.1:1819 did not respond within 3s, proceeding anyway");
            }
        }


        // 1. Locate sing-box binary
        let binary_path = self.locate_sing_box(&app)?;

        // 1b. On Android TUN mode, trigger VpnService and wait for TUN fd
        #[cfg(target_os = "android")]
        let android_tun_fd: Option<i32> = if settings.proxy_mode == ProxyMode::TunMode {
            match self.wait_for_android_tun_fd().await {
                Some(fd) => Some(fd),
                None => {
                    *self.status.lock() = ConnectionStatus::Error;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    return Err("无法建立 VPN 隧道：请在点击连接后，于系统弹窗中允许 VPN 权限，然后重试".to_string());
                }
            }
        } else {
            None
        };
        #[cfg(not(target_os = "android"))]
        let android_tun_fd: Option<i32> = None;

        // 2. Generate configuration
        let adapter = SingBoxAdapter::new();
        let effective_relay = if node.protocol == crate::models::ProtocolType::Psiphon
            || node.protocol == crate::models::ProtocolType::Masque
        {
            None
        } else {
            relay_node.as_ref()
        };
        let mut config_str = adapter
            .generate_config_with_relay(&node, effective_relay, &settings, &self.app_data_dir)
            .map_err(|e| format!("Failed to generate core config: {}", e))?;

        // 2b. On Android TUN mode, standalone sing-box cannot open /dev/net/tun
        // (SELinux): strip the tun inbound and bridge the VpnService fd via tunrelay.
        if android_tun_fd.is_some() {
            config_str = Self::patch_android_relay_config(&config_str)?;
        }

        let config_path = self.app_data_dir.join("current_config.json");
        std::fs::write(&config_path, config_str)
            .map_err(|e| format!("Failed to write core config file: {}", e))?;

        // 3. Spawn child process (CREATE_NO_WINDOW = 0x08000000)
        let log_file_path = self.app_data_dir.join("singbox.log");
        let log_out = std::fs::File::create(&log_file_path)
            .map_err(|e| format!("Failed to create log file: {}", e))?;
        let log_err = log_out
            .try_clone()
            .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

        let mut cmd = Command::new(&binary_path);
        cmd.arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::from(log_out))
            .stderr(std::process::Stdio::from(log_err));
        hide_window_std(&mut cmd);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start sing-box process ({}): {}", binary_path.display(), e))?;

        // 4. Bind child process to Windows Job Object for crash protection
        if let Some(ref guard) = self.job_guard {
            let _ = guard.assign_process(&child);
        }

        *self.process.lock() = Some(child);

        // 5. Verification probe: Ensure core process did not exit on startup
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        {
            let mut proc_lock = self.process.lock();
            if let Some(ref mut c) = *proc_lock {
                if let Ok(Some(exit_status)) = c.try_wait() {
                    let err_log = std::fs::read_to_string(&log_file_path).unwrap_or_default();
                    let _ = PlatformProxy::disable_proxy();
                    *self.status.lock() = ConnectionStatus::Error;
                    *self.connected_node.lock() = None;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    *proc_lock = None;
                    return Err(format!(
                        "Core startup failed ({}):\n{}",
                        exit_status,
                        err_log.trim()
                    ));
                }
            }
        }

        // 5.2 Android TUN: start the tun2socks relay bridging the VpnService fd
        #[cfg(target_os = "android")]
        if let Some(fd) = android_tun_fd {
            if let Err(e) = self.start_tunrelay(&app, fd, settings.mixed_port).await {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                *self.status.lock() = ConnectionStatus::Error;
                *self.connected_node.lock() = None;
                let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                return Err(e);
            }
        }

        // 5.5 End-to-End Real Internet Connectivity Verification Probe
        log::info!("Probing real internet connectivity through proxy port {}...", settings.mixed_port);
        let is_residential = node.group == "Residential";
        if let Err(probe_err) = Self::verify_internet_connectivity(
            settings.mixed_port,
            is_residential,
            current_gen,
            Arc::clone(&self.connect_generation),
        ).await {
            if self.connect_generation.load(Ordering::SeqCst) != current_gen || probe_err.contains("cancelled") {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                if let Some(mut achild) = self.aether_process.lock().take() {
                    let _ = achild.kill();
                    let _ = achild.wait();
                }
                if let Some(mut pchild) = self.psiphon_process.lock().take() {
                    let _ = pchild.kill();
                    let _ = pchild.wait();
                }
                if let Some(mut rchild) = self.relay_process.lock().take() {
                    let _ = rchild.kill();
                    let _ = rchild.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                log::info!("Connection aborted by user during sing-box probe");
                return Err("Connection cancelled by user".to_string());
            }

            log::warn!("Internet connectivity verification failed: {}", probe_err);
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            if let Some(mut achild) = self.aether_process.lock().take() {
                let _ = achild.kill();
                let _ = achild.wait();
            }
            if let Some(mut pchild) = self.psiphon_process.lock().take() {
                let _ = pchild.kill();
                let _ = pchild.wait();
            }
            if let Some(mut rchild) = self.relay_process.lock().take() {
                let _ = rchild.kill();
                let _ = rchild.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            *self.status.lock() = ConnectionStatus::Error;
            *self.connected_node.lock() = None;
            let _ = app.emit("core:status-changed", ConnectionStatus::Error);

            return Err(format!(
                "外网连通性验证失败：该节点虽然能建立本地传输通道，但无法转发国际互联网流量（数据包被 GFW 阻断或节点失效）。\n已自动断开以防浏览器无法上网。请切换其他低延迟的节点！\n详细原因: {}",
                probe_err
            ));
        }

        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            return Err("Connection cancelled by user".to_string());
        }

        *self.connect_time.lock() = Some(Instant::now());

        // 6. Handle System Proxy if configured (only enabled AFTER verification succeeds!)
        if settings.proxy_mode == ProxyMode::SystemProxy {
            if let Err(e) = PlatformProxy::enable_proxy(settings.mixed_port) {
                log::error!("Failed to enable Windows system proxy: {}", e);
            }
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);

        // 7. Spawn background traffic stats polling task
        self.start_traffic_monitor(app.clone(), settings.clash_api_port);

        Ok(())
    }

    pub async fn disconnect(&self, app: AppHandle) -> Result<(), String> {
        self.connect_generation.fetch_add(1, Ordering::SeqCst);
        *self.status.lock() = ConnectionStatus::Disconnecting;
        let _ = app.emit("core:status-changed", ConnectionStatus::Disconnecting);

        self.is_traffic_running.store(false, Ordering::Relaxed);

        // Kill Core processes (sing-box, mihomo, aether, or psiphon)
        let mut proc_lock = self.process.lock();
        if let Some(mut child) = proc_lock.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        let mut aproc_lock = self.aether_process.lock();
        if let Some(mut achild) = aproc_lock.take() {
            let _ = achild.kill();
            let _ = achild.wait();
        }

        let mut pproc_lock = self.psiphon_process.lock();
        if let Some(mut pchild) = pproc_lock.take() {
            let _ = pchild.kill();
            let _ = pchild.wait();
        }

        let mut rproc_lock = self.relay_process.lock();
        if let Some(mut rchild) = rproc_lock.take() {
            let _ = rchild.kill();
            let _ = rchild.wait();
        }

        let mut tproc_lock = self.tunrelay_process.lock();
        if let Some(mut tchild) = tproc_lock.take() {
            let _ = tchild.kill();
            let _ = tchild.wait();
        }

        Self::force_kill_all_cores();

        #[cfg(target_os = "android")]
        {
            let _ = std::fs::remove_file(self.app_data_dir.join("tun_fd"));
            let _ = std::fs::remove_file(self.app_data_dir.join("vpn_pending"));
        }

        // Always restore Windows System Proxy
        let _ = PlatformProxy::disable_proxy();

        *self.connected_node.lock() = None;
        *self.connected_chain.lock() = None;
        *self.connect_time.lock() = None;
        *self.status.lock() = ConnectionStatus::Disconnected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Disconnected);

        Ok(())
    }

    pub async fn connect_chain(
        &self,
        chain_id: String,
        nodes: Vec<UnifiedNode>,
        settings: AppSettings,
        app: AppHandle,
    ) -> Result<(), String> {
        if nodes.len() < 2 {
            return Err("Proxy chain requires at least 2 nodes.".to_string());
        }

        // Disconnect any existing session first
        let _ = self.disconnect(app.clone()).await;
        let current_gen = self.connect_generation.fetch_add(1, Ordering::SeqCst) + 1;
        Self::wait_for_port_release(settings.mixed_port, 2000).await;
        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            return Err("Connection cancelled by user".to_string());
        }

        self.ensure_rules_deployed(&app);

        *self.status.lock() = ConnectionStatus::Connecting;
        let exit_node = nodes.last().unwrap().clone();
        *self.connected_node.lock() = Some(exit_node);
        *self.connected_chain.lock() = Some(chain_id);
        let _ = app.emit("core:status-changed", ConnectionStatus::Connecting);

        let binary_path = self.locate_sing_box(&app)?;

        #[cfg(target_os = "android")]
        let android_tun_fd_chain: Option<i32> = if settings.proxy_mode == ProxyMode::TunMode {
            match self.wait_for_android_tun_fd().await {
                Some(fd) => Some(fd),
                None => {
                    *self.status.lock() = ConnectionStatus::Error;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    return Err("无法建立 VPN 隧道：请在点击连接后，于系统弹窗中允许 VPN 权限，然后重试".to_string());
                }
            }
        } else {
            None
        };
        #[cfg(not(target_os = "android"))]
        let android_tun_fd_chain: Option<i32> = None;

        let adapter = SingBoxAdapter::new();
        let mut config_str = adapter
            .generate_config_for_chain(&nodes, &settings, &self.app_data_dir)
            .map_err(|e| format!("Failed to generate chain config: {}", e))?;

        if android_tun_fd_chain.is_some() {
            config_str = Self::patch_android_relay_config(&config_str)?;
        }

        let config_path = self.app_data_dir.join("current_config.json");
        std::fs::write(&config_path, config_str)
            .map_err(|e| format!("Failed to write core config file: {}", e))?;

        let log_file_path = self.app_data_dir.join("singbox.log");
        let log_out = std::fs::File::create(&log_file_path)
            .map_err(|e| format!("Failed to create log file: {}", e))?;
        let log_err = log_out
            .try_clone()
            .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

        let mut cmd = Command::new(&binary_path);
        cmd.arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::from(log_out))
            .stderr(std::process::Stdio::from(log_err));
        hide_window_std(&mut cmd);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start sing-box process ({}): {}", binary_path.display(), e))?;

        if let Some(ref guard) = self.job_guard {
            let _ = guard.assign_process(&child);
        }

        *self.process.lock() = Some(child);

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            return Err("Connection cancelled by user".to_string());
        }

        {
            let mut proc_lock = self.process.lock();
            if let Some(ref mut c) = *proc_lock {
                if let Ok(Some(exit_status)) = c.try_wait() {
                    let err_log = std::fs::read_to_string(&log_file_path).unwrap_or_default();
                    let _ = PlatformProxy::disable_proxy();
                    *self.status.lock() = ConnectionStatus::Error;
                    *self.connected_node.lock() = None;
                    *self.connected_chain.lock() = None;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    *proc_lock = None;
                    return Err(format!(
                        "Chain core startup failed ({}):\n{}",
                        exit_status,
                        err_log.trim()
                    ));
                }
            }
        }

        #[cfg(target_os = "android")]
        if let Some(fd) = android_tun_fd_chain {
            if let Err(e) = self.start_tunrelay(&app, fd, settings.mixed_port).await {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                *self.status.lock() = ConnectionStatus::Error;
                *self.connected_node.lock() = None;
                *self.connected_chain.lock() = None;
                let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                return Err(e);
            }
        }

        log::info!("Probing chain internet connectivity through proxy port {}...", settings.mixed_port);
        if let Err(probe_err) = Self::verify_internet_connectivity(
            settings.mixed_port,
            false,
            current_gen,
            Arc::clone(&self.connect_generation),
        ).await {
            if self.connect_generation.load(Ordering::SeqCst) != current_gen || probe_err.contains("cancelled") {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                log::info!("Chain connection aborted by user during probe");
                return Err("Connection cancelled by user".to_string());
            }

            log::warn!("Chain connectivity verification failed: {}", probe_err);
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            *self.status.lock() = ConnectionStatus::Error;
            *self.connected_node.lock() = None;
            *self.connected_chain.lock() = None;
            let _ = app.emit("core:status-changed", ConnectionStatus::Error);

            return Err(format!(
                "外网连通性验证失败：链式代理未能成功转发外网流量（可能链中节点失效或被阻断）。\n已自动断开以防浏览器断网。\n详细原因: {}",
                probe_err
            ));
        }

        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            return Err("Connection cancelled by user".to_string());
        }

        *self.connect_time.lock() = Some(Instant::now());

        if settings.proxy_mode == ProxyMode::SystemProxy {
            if let Err(e) = PlatformProxy::enable_proxy(settings.mixed_port) {
                log::error!("Failed to enable Windows system proxy: {}", e);
            }
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);
        self.start_traffic_monitor(app.clone(), settings.clash_api_port);
        Ok(())
    }

    pub fn shutdown(&self) {
        self.is_traffic_running.store(false, Ordering::Relaxed);
        let mut proc_lock = self.process.lock();
        if let Some(mut child) = proc_lock.take() {
            let _ = child.kill();
        }
        let mut aproc_lock = self.aether_process.lock();
        if let Some(mut achild) = aproc_lock.take() {
            let _ = achild.kill();
        }
        let mut pproc_lock = self.psiphon_process.lock();
        if let Some(mut pchild) = pproc_lock.take() {
            let _ = pchild.kill();
        }
        let mut rproc_lock = self.relay_process.lock();
        if let Some(mut rchild) = rproc_lock.take() {
            let _ = rchild.kill();
        }
        Self::force_kill_all_cores();
        let _ = PlatformProxy::disable_proxy();
    }

    fn format_mihomo_relay_proxy(relay: &UnifiedNode) -> Option<String> {
        let conf = &relay.config;
        match relay.protocol {
            crate::models::ProtocolType::Shadowsocks => {
                let method = conf.get("method").and_then(|v| v.as_str()).unwrap_or("chacha20-ietf-poly1305");
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or_default();
                Some(format!(
r#"  - name: relay
    type: ss
    server: {}
    port: {}
    cipher: {}
    password: "{}"
    udp: true"#,
                    relay.address, relay.port, method, password
                ))
            }
            crate::models::ProtocolType::Trojan => {
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or_default();
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&relay.address);
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
                let mut s = format!(
r#"  - name: relay
    type: trojan
    server: {}
    port: {}
    password: "{}"
    udp: true
    sni: {}
    skip-cert-verify: true
    network: {}"#,
                    relay.address, relay.port, password, sni, network
                );
                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or(sni);
                    s.push_str(&format!(
r#"
    ws-opts:
      path: "{}"
      headers:
        Host: "{}""#,
                        if path.is_empty() { "/" } else { path }, host
                    ));
                }
                Some(s)
            }
            crate::models::ProtocolType::Vless => {
                let uuid = conf.get("uuid").and_then(|v| v.as_str()).unwrap_or_default();
                let flow = conf.get("flow").and_then(|v| v.as_str()).unwrap_or("");
                let security = conf.get("security").and_then(|v| v.as_str()).unwrap_or("none");
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&relay.address);
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
                let reality_pbk = conf.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
                let reality_sid = conf.get("short_id").and_then(|v| v.as_str()).unwrap_or("");
                let fp = conf.get("fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome");

                let mut s = format!(
r#"  - name: relay
    type: vless
    server: {}
    port: {}
    uuid: "{}"
    network: {}
    udp: true"#,
                    relay.address, relay.port, uuid, network
                );
                if !flow.is_empty() {
                    s.push_str(&format!("\n    flow: {}", flow));
                }
                if security == "reality" {
                    s.push_str(&format!(
r#"
    tls: true
    servername: {}
    client-fingerprint: {}
    reality-opts:
      public-key: {}
      short-id: {}"#,
                        sni, fp, reality_pbk, reality_sid
                    ));
                } else if security == "tls" || conf.get("tls").and_then(|v| v.as_bool()).unwrap_or(false) {
                    s.push_str(&format!(
r#"
    tls: true
    servername: {}
    skip-cert-verify: true
    client-fingerprint: {}"#,
                        sni, fp
                    ));
                }
                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or(sni);
                    s.push_str(&format!(
r#"
    ws-opts:
      path: "{}"
      headers:
        Host: "{}""#,
                        if path.is_empty() { "/" } else { path }, host
                    ));
                }
                Some(s)
            }
            crate::models::ProtocolType::Vmess => {
                let uuid = conf.get("uuid").and_then(|v| v.as_str()).unwrap_or_default();
                let alter_id = conf.get("alterId").or_else(|| conf.get("alter_id")).and_then(|v| v.as_u64()).unwrap_or(0);
                let cipher = conf.get("cipher").and_then(|v| v.as_str()).unwrap_or("auto");
                let network = conf.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
                let is_tls = conf.get("tls").and_then(|v| v.as_bool()).unwrap_or(false)
                    || conf.get("security").and_then(|v| v.as_str()) == Some("tls");
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&relay.address);

                let mut s = format!(
r#"  - name: relay
    type: vmess
    server: {}
    port: {}
    uuid: "{}"
    alterId: {}
    cipher: {}
    udp: true
    network: {}"#,
                    relay.address, relay.port, uuid, alter_id, cipher, network
                );
                if is_tls {
                    s.push_str(&format!(
r#"
    tls: true
    servername: {}
    skip-cert-verify: true"#,
                        sni
                    ));
                }
                if network == "ws" {
                    let path = conf.get("path").and_then(|v| v.as_str()).unwrap_or("/");
                    let host = conf.get("host").and_then(|v| v.as_str()).unwrap_or(sni);
                    s.push_str(&format!(
r#"
    ws-opts:
      path: "{}"
      headers:
        Host: "{}""#,
                        if path.is_empty() { "/" } else { path }, host
                    ));
                }
                Some(s)
            }
            crate::models::ProtocolType::Hysteria2 => {
                let password = conf.get("password").and_then(|v| v.as_str()).unwrap_or_default();
                let sni = conf.get("sni").and_then(|v| v.as_str()).unwrap_or(&relay.address);
                Some(format!(
r#"  - name: relay
    type: hysteria2
    server: {}
    port: {}
    password: "{}"
    sni: {}
    skip-cert-verify: true
    udp: true"#,
                    relay.address, relay.port, password, sni
                ))
            }
            crate::models::ProtocolType::Socks5 => {
                let user = conf.get("username").and_then(|v| v.as_str()).unwrap_or_default();
                let pass = conf.get("password").and_then(|v| v.as_str()).unwrap_or_default();
                let mut s = format!(
r#"  - name: relay
    type: socks5
    server: {}
    port: {}"#,
                    relay.address, relay.port
                );
                if !user.is_empty() {
                    s.push_str(&format!("\n    username: \"{}\"\n    password: \"{}\"", user, pass));
                }
                Some(s)
            }
            crate::models::ProtocolType::Http => {
                let user = conf.get("username").and_then(|v| v.as_str()).unwrap_or_default();
                let pass = conf.get("password").and_then(|v| v.as_str()).unwrap_or_default();
                let mut s = format!(
r#"  - name: relay
    type: http
    server: {}
    port: {}"#,
                    relay.address, relay.port
                );
                if !user.is_empty() {
                    s.push_str(&format!("\n    username: \"{}\"\n    password: \"{}\"", user, pass));
                }
                Some(s)
            }
            _ => None,
        }
    }

    fn generate_mihomo_openvpn_config(
        node: &UnifiedNode,
        relay_node: Option<&UnifiedNode>,
        settings: &AppSettings,
    ) -> String {
        let conf = &node.config;
        let proto = if relay_node.is_some() {
            "tcp"
        } else {
            conf.get("proto").and_then(|v| v.as_str()).unwrap_or("tcp")
        };
        let cipher = conf.get("cipher").and_then(|v| v.as_str()).unwrap_or("AES-128-CBC");
        let auth = conf.get("auth").and_then(|v| v.as_str()).unwrap_or("SHA1");

        let (relay_block, dialer_proxy_line) = if let Some(relay) = relay_node {
            if let Some(r_yaml) = Self::format_mihomo_relay_proxy(relay) {
                (format!("{}\n", r_yaml), "    dialer-proxy: relay\n".to_string())
            } else {
                (String::new(), String::new())
            }
        } else {
            (String::new(), String::new())
        };

        let block = |val: &str, indent: usize| -> String {
            let sp = " ".repeat(indent);
            val.lines().map(|l| format!("{}{}\n", sp, l)).collect()
        };

        let ca_raw = conf.get("ca").and_then(|v| v.as_str()).unwrap_or("");
        let cert_raw = conf.get("cert").and_then(|v| v.as_str()).unwrap_or("");
        let key_raw = conf.get("key").and_then(|v| v.as_str()).unwrap_or("");

        const DEFAULT_OVPN_CA: &str = "-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAwDQYJKoZIhvcNAQELBQAw\nTzELMAkGA1UEBhMCVVMxKTAnBgNVBAoTIEludGVybmV0IFNlY3VyaXR5IFJlc2Vh\ncmNoIEdyb3VwMRUwEwYDVQQDEwxJU1JHIFJvb3QgWDEwHhcNMTUwNjA0MTEwNDM4\nWhcNMzUwNjA0MTEwNDM4WjBPMQswCQYDVQQGEwJVUzEpMCcGA1UEChMgSW50ZXJu\nZXQgU2VjdXJpdHkgUmVzZWFyY2ggR3JvdXAxFTATBgNVBAMTDElTUkcgUm9vdCBY\nMTCCAiIwDQYJKoZIhvcNAQEBBQADggIPADCCAgoCggIBAK3oJHP0FDfzm54rVygc\nh77ct984kIxuPOZXoHj3dcKi/vVqbvYATyjb3miGbESTtrFj/RQSa78f0uoxmyF+\n0TM8ukj13Xnfs7j/EvEhmkvBioZxaUpmZmyPfjxwv60pIgbz5MDmgK7iS4+3mX6U\nA5/TR5d8mUgjU+g4rk8Kb4Mu0UlXjIB0ttov0DiNewNwIRt18jA8+o+u3dpjq+sW\nT8KOEUt+zwvo/7V3LvSye0rgTBIlDHCNAymg4VMk7BPZ7hm/ELNKjD+Jo2FR3qyH\nB5T0Y3HsLuJvW5iB4YlcNHlsdu87kGJ55tukmi8mxdAQ4Q7e2RCOFvu396j3x+UC\nB5iPNgiV5+I3lg02dZ77DnKxHZu8A/lJBdiB3QW0KtZB6awBdpUKD9jf1b0SHzUv\nKBds0pjBqAlkd25HN7rOrFleaJ1/ctaJxQZBKT5ZPt0m9STJEadao0xAH0ahmbWn\nOlFuhjuefXKnEgV4We0+UXgVCwOPjdAvBbI+e0ocS3MFEvzG6uBQE3xDk3SzynTn\njh8BCNAw1FtxNrQHusEwMFxIt4I7mKZ9YIqioymCzLq9gwQbooMDQaHWBfEbwrbw\nqHyGO0aoSCqI3Haadr8faqU9GY/rOPNk3sgrDQoo//fb4hVC1CLQJ13hef4Y53CI\nrU7m2Ys6xt0nUW7/vGT1M0NPAgMBAAGjQjBAMA4GA1UdDwEB/wQEAwIBBjAPBgNV\nHRMBAf8EBTADAQH/MB0GA1UdDgQWBBR5tFnme7bl5AFzgAiIyBpY9umbbjANBgkq\nhkiG9w0BAQsFAAOCAgEAVR9YqbyyqFDQDLHYGmkgJykIrGF1XIpu+ILlaS/V9lZL\nubhzEFnTIZd+50xx+7LSYK05qAvqFyFWhfFQDlnrzuBZ6brJFe+GnY+EgPbk6ZGQ\n3BebYhtF8GaV0nxvwuo77x/Py9auJ/GpsMiu/X1+mvoiBOv/2X/qkSsisRcOj/KK\nNFtY2PwByVS5uCbMiogziUwthDyC3+6WVwW6LLv3xLfHTjuCvjHIInNzktHCgKQ5\nORAzI4JMPJ+GslWYHb4phowim57iaztXOoJwTdwJx4nLCgdNbOhdjsnvzqvHu7Ur\nTkXWStAmzOVyyghqpZXjFaH3pO3JLF+l+/+sKAIuvtd7u+Nxe5AW0wdeRlN8NwdC\njNPElpzVmbUq4JUagEiuTDkHzsxHpFKVK7q4+63SM1N95R1NbdWhscdCb+ZAJzVc\noyi3B43njTOQ5yOf+1CceWxG1bQVs5ZufpsMljq4Ui0/1lvh+wjChP4kqKOJ2qxq\n4RgqsahDYVvTH9w7jXbyLeiNdd8XM2w9U/t7y0Ff/9yi0GE44Za4rF2LN9d11TPA\nmRGunUHBcnWEvgJBQl9nJEiU0Zsnvgc/ubhPgXRR4Xq37Z0j4r7g1SgEEzwxA57d\nemyPxgcYxn/eR44/KJ4EBs+lVDR3veyJm+kXQ99b21/+jh5Xos1AnX5iItreGCc=\n-----END CERTIFICATE-----";
        let full_ca = if !ca_raw.is_empty() {
            if ca_raw.contains("BEGIN CERTIFICATE") {
                ca_raw.to_string()
            } else {
                format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----", ca_raw)
            }
        } else {
            DEFAULT_OVPN_CA.to_string()
        };
        let ca_block = format!("    ca: |\n{}", block(&full_ca, 6));

        let cert_block = if !cert_raw.is_empty() {
            let full_cert = if cert_raw.contains("BEGIN CERTIFICATE") {
                cert_raw.to_string()
            } else {
                format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----", cert_raw)
            };
            format!("    cert: |\n{}", block(&full_cert, 6))
        } else {
            String::new()
        };

        let key_block = if !key_raw.is_empty() {
            let full_key = if key_raw.contains("BEGIN RSA PRIVATE KEY") || key_raw.contains("BEGIN PRIVATE KEY") {
                key_raw.to_string()
            } else {
                format!("-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----", key_raw)
            };
            format!("    key: |\n{}", block(&full_key, 6))
        } else {
            String::new()
        };

        let mihomo_mode = match settings.routing_mode.as_str() {
            "global" => "global",
            "direct" => "direct",
            _ => "rule",
        };

        let active_set = settings.get_active_rule_set();
        let mut rules_yaml = String::new();

        let format_rule = |item: &str, action: &str| -> String {
            let r = item.trim();
            if let Some(cat) = r.strip_prefix("geosite:") {
                format!("  - GEOSITE,{},{}\n", cat, action)
            } else if let Some(cat) = r.strip_prefix("geoip:") {
                format!("  - GEOIP,{},{},no-resolve\n", cat, action)
            } else if r.contains('/') || r.parse::<std::net::IpAddr>().is_ok() {
                format!("  - IP-CIDR,{},{},no-resolve\n", r, action)
            } else {
                format!("  - DOMAIN-SUFFIX,{},{}\n", r, action)
            }
        };

        // 1. Block rules: always reject UDP 443 (QUIC) for OpenVPN to prevent Chrome/Edge from hanging
        rules_yaml.push_str("  - AND,((NETWORK,udp),(DST-PORT,443)),REJECT\n");
        for r in &active_set.block_rules {
            if !r.trim().is_empty() {
                rules_yaml.push_str(&format_rule(r, "REJECT"));
            }
        }

        // 2. Direct rules
        for r in &active_set.direct_rules {
            if !r.trim().is_empty() {
                rules_yaml.push_str(&format_rule(r, "DIRECT"));
            }
        }
        for r in &settings.custom_direct_rules {
            if !r.trim().is_empty() {
                rules_yaml.push_str(&format_rule(r, "DIRECT"));
            }
        }

        // 3. Proxy rules
        for r in &active_set.proxy_rules {
            if !r.trim().is_empty() {
                rules_yaml.push_str(&format_rule(r, "proxy"));
            }
        }
        for r in &settings.custom_proxy_rules {
            if !r.trim().is_empty() {
                rules_yaml.push_str(&format_rule(r, "proxy"));
            }
        }

        // Fallback rule
        rules_yaml.push_str("  - MATCH,proxy\n");

        format!(
r#"mixed-port: {}
allow-lan: false
mode: {}
log-level: info
external-controller: 127.0.0.1:{}

dns:
  enable: true
  ipv6: false
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  fake-ip-filter:
    - "*.lan"
    - "*.localdomain"
    - "*.example"
    - "*.invalid"
    - "*.localhost"
    - "*.test"
    - "*.local"
    - "*.home.arpa"
  nameserver:
    - 223.5.5.5
    - 119.29.29.29
    - 1.1.1.1
    - 8.8.8.8
  fallback:
    - 1.1.1.1
    - 8.8.8.8

proxies:
{}  - name: proxy
    type: openvpn
    server: {}
    port: {}
    proto: {}
{}    udp: true
    username: vpn
    password: vpn
    cipher: {}
    auth: {}
{}
{}
{}

rules:
{}"#,
            settings.mixed_port,
            mihomo_mode,
            settings.clash_api_port,
            relay_block,
            node.address,
            node.port,
            proto,
            dialer_proxy_line,
            cipher,
            auth,
            ca_block.trim_end(),
            cert_block.trim_end(),
            key_block.trim_end(),
            rules_yaml
        )
    }

    fn ensure_rules_deployed(&self, app: &AppHandle) {
        use tauri::Manager;
        let rules_dst = self.app_data_dir.join("rules");
        let _ = std::fs::create_dir_all(&rules_dst);

        let rule_files = [
            "geosite-category-ads-all.srs",
            "geosite-private.srs",
            "geosite-cn.srs",
            "geoip-cn.srs",
        ];

        let mut search_dirs = Vec::new();
        #[cfg(target_os = "android")]
        {
            search_dirs.push(self.app_data_dir.join("binaries").join("rules"));
            search_dirs.push(self.app_data_dir.join("rules"));
            search_dirs.push(self.app_data_dir.join("binaries"));
        }
        if let Ok(res_dir) = app.path().resource_dir() {
            search_dirs.push(res_dir.join("binaries").join("rules"));
            search_dirs.push(res_dir.join("rules"));
            search_dirs.push(res_dir.join("binaries"));
        }
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_dir = exe_path.parent().unwrap_or(Path::new(""));
            search_dirs.push(exe_dir.join("binaries").join("rules"));
            search_dirs.push(exe_dir.join("rules"));
            search_dirs.push(exe_dir.join("binaries"));
        }
        search_dirs.push(PathBuf::from("src-tauri/binaries/rules"));
        search_dirs.push(PathBuf::from("binaries/rules"));

        for rule in &rule_files {
            let dst = rules_dst.join(rule);
            if !dst.exists() || std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0) == 0 {
                for dir in &search_dirs {
                    let src = dir.join(rule);
                    if src.exists() {
                        let _ = std::fs::copy(&src, &dst);
                        break;
                    }
                }
            }
        }
    }

    fn locate_binary(&self, app: &AppHandle, base_name: &str) -> Result<PathBuf, String> {
        use tauri::Manager;
        let bin_name = core_binary_name(base_name);
        let mut candidates = Vec::new();

        #[cfg(target_os = "android")]
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            // dataDir is mounted noexec for targetSdk>=29; the only reliably
            // executable location is nativeLibraryDir, where the installer
            // extracts our lib<name>.so pseudo-libs (written by VpnInitProvider).
            if let Ok(nd) = std::fs::read_to_string(app_data_dir.join("native_lib_dir")) {
                let nd = nd.trim();
                if !nd.is_empty() {
                    candidates.push(PathBuf::from(nd).join(format!("lib{}.so", base_name)));
                }
            }
            candidates.push(app_data_dir.join("binaries").join(&bin_name));
            candidates.push(app_data_dir.join(&bin_name));
        }

        if let Ok(res_dir) = app.path().resource_dir() {
            candidates.push(res_dir.join("binaries").join(&bin_name));
            candidates.push(res_dir.join(&bin_name));
        }

        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new("")).to_path_buf())
            .unwrap_or_default();

        candidates.push(exe_dir.join("binaries").join(&bin_name));
        candidates.push(exe_dir.join(&bin_name));
        candidates.push(PathBuf::from(&bin_name));

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            let which_cmd = if cfg!(windows) { "where.exe" } else { "which" };
            if let Ok(output) = Command::new(which_cmd).arg(base_name).output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(first_line) = stdout.lines().next() {
                        let path = PathBuf::from(first_line.trim());
                        if path.exists() {
                            return Ok(path);
                        }
                    }
                }
            }
        }

        Err(format!("{} core binary not found in application directories or PATH", bin_name))
    }

    fn locate_sing_box(&self, app: &AppHandle) -> Result<PathBuf, String> {
        self.locate_binary(app, "sing-box")
    }

    fn locate_mihomo(&self, app: &AppHandle) -> Result<PathBuf, String> {
        self.locate_binary(app, "mihomo")
    }

    fn locate_aether(&self, app: &AppHandle) -> Result<PathBuf, String> {
        self.locate_binary(app, "aether")
    }

    fn locate_server_entries(&self, app: &AppHandle) -> Result<PathBuf, String> {
        use tauri::Manager;
        let mut candidates = Vec::new();

        #[cfg(target_os = "android")]
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            candidates.push(app_data_dir.join("binaries").join("server_entries.txt"));
            candidates.push(app_data_dir.join("server_entries.txt"));
        }

        if let Ok(res_dir) = app.path().resource_dir() {
            candidates.push(res_dir.join("binaries").join("server_entries.txt"));
            candidates.push(res_dir.join("server_entries.txt"));
        }

        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new("")).to_path_buf())
            .unwrap_or_default();

        candidates.push(exe_dir.join("binaries").join("server_entries.txt"));
        candidates.push(exe_dir.join("server_entries.txt"));
        candidates.push(PathBuf::from("server_entries.txt"));

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        Err("server_entries.txt 节点数据库文件未找到，请检查安装目录。".to_string())
    }

    fn locate_psiphon(&self, app: &AppHandle) -> Result<PathBuf, String> {
        self.locate_binary(app, "psiphon-tunnel-core")
    }

    /// Verifies end-to-end internet connectivity through the newly started local proxy port.
    /// Sends a lightweight HTTP GET request to Cloudflare / Google 204 endpoint.
    /// If internet is unreachable, returns an Err describing the failure.
    async fn verify_internet_connectivity(
        proxy_port: u16,
        is_residential: bool,
        current_gen: u64,
        connect_gen: Arc<AtomicU64>,
    ) -> Result<(), String> {
        let proxy_url = format!("http://127.0.0.1:{}", proxy_port);
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|e| format!("Invalid local proxy configuration: {}", e))?;

        let timeout_ms = if is_residential { 6000 } else { 4500 };
        let max_attempts = if is_residential { 12 } else { 6 };

        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| format!("Failed to build probe client: {}", e))?;

        let mut last_err = String::new();
        // Probe up to max_attempts (allows time for multi-hop relays and OpenVPN tunnels to finish handshake)
        for attempt in 1..=max_attempts {
            if connect_gen.load(Ordering::SeqCst) != current_gen {
                return Err("Connection cancelled by user".to_string());
            }

            if attempt > 1 {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                if connect_gen.load(Ordering::SeqCst) != current_gen {
                    return Err("Connection cancelled by user".to_string());
                }
            }

            // Primary probe: Cloudflare captive portal 204 endpoint
            match client.get("http://cp.cloudflare.com/generate_204").send().await {
                Ok(resp) if resp.status().as_u16() == 204 || resp.status().as_u16() == 200 => {
                    return Ok(());
                }
                Ok(resp) => {
                    last_err = format!("HTTP 状态码: {}", resp.status());
                }
                Err(e) => {
                    last_err = e.to_string();
                }
            }

            if connect_gen.load(Ordering::SeqCst) != current_gen {
                return Err("Connection cancelled by user".to_string());
            }

            // Secondary fallback probe: Google 204 endpoint
            if let Ok(resp) = client.get("http://www.google.com/generate_204").send().await {
                let code = resp.status().as_u16();
                if code == 204 || code == 200 {
                    return Ok(());
                }
            }

            // Tertiary fallback: Cloudflare Trace
            if let Ok(resp) = client.get("http://1.1.1.1/cdn-cgi/trace").send().await {
                if resp.status().is_success() {
                    return Ok(());
                }
            }
        }

        if connect_gen.load(Ordering::SeqCst) != current_gen {
            return Err("Connection cancelled by user".to_string());
        }

        Err(format!("端到端测试超时：数据包无法在预定时限内到达国际互联网目标（{}）", last_err))
    }

    fn start_traffic_monitor(&self, app: AppHandle, clash_port: u16) {
        self.is_traffic_running.store(true, Ordering::Relaxed);
        let is_running = Arc::clone(&self.is_traffic_running);
        let connect_time = Arc::clone(&self.connect_time);

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(800))
                .build()
                .unwrap_or_default();

            let conn_url = format!("http://127.0.0.1:{}/connections", clash_port);
            let mut prev_down: Option<u64> = None;
            let mut prev_up: Option<u64> = None;
            let mut total_up: u64 = 0;
            let mut total_down: u64 = 0;

            while is_running.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                if !is_running.load(Ordering::Relaxed) {
                    break;
                }

                let uptime = connect_time
                    .lock()
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);

                let mut current_up_speed: u64 = 0;
                let mut current_down_speed: u64 = 0;

                // Query Clash API /connections endpoint (contains real cumulative downloadTotal & uploadTotal)
                if let Ok(resp) = client.get(&conn_url).send().await {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let curr_down = json.get("downloadTotal")
                            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
                            .unwrap_or(total_down);
                        let curr_up = json.get("uploadTotal")
                            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
                            .unwrap_or(total_up);

                        if let Some(prev) = prev_down {
                            current_down_speed = curr_down.saturating_sub(prev);
                        }
                        if let Some(prev) = prev_up {
                            current_up_speed = curr_up.saturating_sub(prev);
                        }

                        prev_down = Some(curr_down);
                        prev_up = Some(curr_up);
                        total_down = curr_down;
                        total_up = curr_up;
                    }
                }

                let stats = TrafficStats {
                    upload_bytes: total_up,
                    download_bytes: total_down,
                    upload_speed: current_up_speed,
                    download_speed: current_down_speed,
                    uptime_seconds: uptime,
                };

                let _ = app.emit("core:traffic-tick", stats);
            }
        });
    }

    pub async fn connect_smart_group(
        &self,
        nodes: Vec<UnifiedNode>,
        settings: AppSettings,
        app: AppHandle,
    ) -> Result<(), String> {
        if nodes.is_empty() {
            return Err("Smart group requires at least 1 node.".to_string());
        }

        let _ = self.disconnect(app.clone()).await;
        let current_gen = self.connect_generation.fetch_add(1, Ordering::SeqCst) + 1;
        Self::wait_for_port_release(settings.mixed_port, 2000).await;
        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            return Err("Connection cancelled by user".to_string());
        }

        self.ensure_rules_deployed(&app);

        *self.status.lock() = ConnectionStatus::Connecting;
        *self.connected_node.lock() = Some(nodes[0].clone()); 
        *self.connected_chain.lock() = Some("smart-group".to_string());
        let _ = app.emit("core:status-changed", ConnectionStatus::Connecting);

        let binary_path = self.locate_sing_box(&app)?;

        #[cfg(target_os = "android")]
        let android_tun_fd_smart: Option<i32> = if settings.proxy_mode == ProxyMode::TunMode {
            match self.wait_for_android_tun_fd().await {
                Some(fd) => Some(fd),
                None => {
                    *self.status.lock() = ConnectionStatus::Error;
                    *self.connected_chain.lock() = None;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    return Err("无法建立 VPN 隧道：请在点击连接后，于系统弹窗中允许 VPN 权限，然后重试".to_string());
                }
            }
        } else {
            None
        };
        #[cfg(not(target_os = "android"))]
        let android_tun_fd_smart: Option<i32> = None;

        let adapter = SingBoxAdapter::new();
        let mut config_str = adapter
            .generate_config_for_urltest(&nodes, &settings, &self.app_data_dir)
            .map_err(|e| format!("Failed to generate urltest config: {}", e))?;

        if android_tun_fd_smart.is_some() {
            config_str = Self::patch_android_relay_config(&config_str)?;
        }

        let config_path = self.app_data_dir.join("current_config.json");
        std::fs::write(&config_path, config_str).map_err(|e| format!("Failed to write core config file: {}", e))?;

        let log_file_path = self.app_data_dir.join("singbox.log");
        let log_out = std::fs::File::create(&log_file_path).unwrap();
        let log_err = log_out.try_clone().unwrap();

        let mut cmd = Command::new(&binary_path);
        cmd.arg("run").arg("-c").arg(&config_path)
            .stdout(std::process::Stdio::from(log_out))
            .stderr(std::process::Stdio::from(log_err));
        hide_window_std(&mut cmd);

        let child = cmd.spawn().map_err(|e| format!("Failed to start sing-box process ({}): {}", binary_path.display(), e))?;

        if let Some(ref guard) = self.job_guard {
            let _ = guard.assign_process(&child);
        }

        *self.process.lock() = Some(child);

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            return Err("Connection cancelled by user".to_string());
        }

        {
            let mut proc_lock = self.process.lock();
            if let Some(ref mut c) = *proc_lock {
                if let Ok(Some(exit_status)) = c.try_wait() {
                    let err_log = std::fs::read_to_string(&log_file_path).unwrap_or_default();
                    let _ = PlatformProxy::disable_proxy();
                    *self.status.lock() = ConnectionStatus::Error;
                    *self.connected_node.lock() = None;
                    *self.connected_chain.lock() = None;
                    let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                    *proc_lock = None;
                    return Err(format!(
                        "Smart group core startup failed ({}):\n{}",
                        exit_status,
                        err_log.trim()
                    ));
                }
            }
        }

        #[cfg(target_os = "android")]
        if let Some(fd) = android_tun_fd_smart {
            if let Err(e) = self.start_tunrelay(&app, fd, settings.mixed_port).await {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                *self.status.lock() = ConnectionStatus::Error;
                *self.connected_node.lock() = None;
                *self.connected_chain.lock() = None;
                let _ = app.emit("core:status-changed", ConnectionStatus::Error);
                return Err(e);
            }
        }

        // End-to-End Real Internet Connectivity Verification Probe
        log::info!("Probing real internet connectivity through proxy port {}...", settings.mixed_port);
        if let Err(probe_err) = Self::verify_internet_connectivity(
            settings.mixed_port,
            false,
            current_gen,
            Arc::clone(&self.connect_generation),
        ).await {
            if self.connect_generation.load(Ordering::SeqCst) != current_gen || probe_err.contains("cancelled") {
                if let Some(mut c) = self.process.lock().take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                Self::force_kill_all_cores();
                let _ = PlatformProxy::disable_proxy();
                log::info!("Smart group connection aborted by user during probe");
                return Err("Connection cancelled by user".to_string());
            }

            log::warn!("Internet connectivity verification failed for smart group: {}", probe_err);
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            *self.status.lock() = ConnectionStatus::Error;
            *self.connected_node.lock() = None;
            *self.connected_chain.lock() = None;
            let _ = app.emit("core:status-changed", ConnectionStatus::Error);
            return Err(format!("外网连通性校验失败: {}", probe_err));
        }

        if self.connect_generation.load(Ordering::SeqCst) != current_gen {
            if let Some(mut c) = self.process.lock().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            Self::force_kill_all_cores();
            let _ = PlatformProxy::disable_proxy();
            return Err("Connection cancelled by user".to_string());
        }

        *self.connect_time.lock() = Some(Instant::now());

        if settings.routing_mode == "global" || settings.routing_mode == "rule" {
            let _ = PlatformProxy::enable_proxy(settings.mixed_port);
        }

        *self.status.lock() = ConnectionStatus::Connected;
        let _ = app.emit("core:status-changed", ConnectionStatus::Connected);
        self.start_traffic_monitor(app.clone(), settings.clash_api_port);

        Ok(())

    }

    /// On Android, trigger VpnService and wait for the TUN fd file to appear.
    /// Returns the fd number on success, or None on non-Android / proxy-only mode.
    #[cfg(target_os = "android")]
    async fn wait_for_android_tun_fd(&self) -> Option<i32> {
        let fd_file = self.app_data_dir.join("tun_fd");
        let pending_file = self.app_data_dir.join("vpn_pending");
        let _ = std::fs::remove_file(&fd_file);

        log::info!("Android: writing vpn_pending signal for VpnService...");
        let _ = std::fs::write(&pending_file, "1");

        // Up to 120s: the first connection also waits for the user to accept
        // the system VPN-permission dialog before the tunnel can be built.
        for i in 0..480 {
            if let Ok(content) = std::fs::read_to_string(&fd_file) {
                if let Ok(fd) = content.trim().parse::<i32>() {
                    log::info!("Android: received TUN fd={} after {}ms", fd, i * 250);
                    let _ = std::fs::remove_file(&fd_file);
                    return Some(fd);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        let _ = std::fs::remove_file(&pending_file);
        log::error!("Android: timed out waiting for TUN fd from VpnService (120s)");
        None
    }

    /// Android TUN mode: a standalone sing-box process cannot open /dev/net/tun
    /// (SELinux) and has no fd-based tun option. Strip the tun inbound entirely
    /// and let the tunrelay child process bridge the VpnService fd to the
    /// SOCKS5 mixed port. Sniffing on the mixed inbound recovers domains from
    /// the raw-IP tun destinations so domain rules keep working.
    #[cfg(target_os = "android")]
    fn patch_android_relay_config(config_str: &str) -> Result<String, String> {
        let mut config: serde_json::Value = serde_json::from_str(config_str)
            .map_err(|e| format!("Failed to parse config for Android relay patch: {}", e))?;

        if let Some(inbounds) = config.get_mut("inbounds").and_then(|v| v.as_array_mut()) {
            inbounds.retain(|ib| ib.get("type").and_then(|v| v.as_str()) != Some("tun"));
            for ib in inbounds.iter_mut() {
                if ib.get("type").and_then(|v| v.as_str()) == Some("mixed") {
                    ib["sniff"] = serde_json::json!(true);
                    ib["sniff_override_destination"] = serde_json::json!(true);
                }
            }
        }
        log::info!("Android: stripped tun inbound, enabled sniffing on mixed inbound");

        serde_json::to_string(&config).map_err(|e| format!("Failed to serialize patched config: {}", e))
    }

    #[cfg(not(target_os = "android"))]
    fn patch_android_relay_config(config_str: &str) -> Result<String, String> {
        // Unreachable: android_tun_fd is always None off Android.
        Ok(config_str.to_string())
    }

    /// Spawn the tunrelay helper (Android-only): a static-Go gVisor netstack
    /// that terminates TCP/UDP on the inherited VpnService fd and forwards all
    /// flows into sing-box's SOCKS5 mixed port.
    #[cfg(target_os = "android")]
    async fn start_tunrelay(&self, app: &AppHandle, tun_fd: i32, mixed_port: u16) -> Result<(), String> {
        let relay_path = self.locate_binary(app, "tunrelay")?;

        // The child inherits the tun fd across exec only if FD_CLOEXEC is clear.
        unsafe {
            libc::fcntl(tun_fd, libc::F_SETFD, 0);
        }

        // Wait for sing-box's mixed port to accept connections (up to 10s).
        let mut ready = false;
        for _ in 0..40 {
            if std::net::TcpStream::connect(("127.0.0.1", mixed_port)).is_ok() {
                ready = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        if !ready {
            log::warn!("Android: mixed port {} not listening before relay start", mixed_port);
        }

        let log_out = std::fs::File::create(self.app_data_dir.join("tunrelay.log"))
            .map_err(|e| format!("Failed to create tunrelay log: {}", e))?;
        let log_err = log_out
            .try_clone()
            .map_err(|e| format!("Failed to clone tunrelay log handle: {}", e))?;

        let mut cmd = Command::new(&relay_path);
        cmd.arg("--fd")
            .arg(tun_fd.to_string())
            .arg("--socks")
            .arg(format!("127.0.0.1:{}", mixed_port))
            .arg("--address")
            .arg("172.19.0.1")
            .arg("--mtu")
            .arg("9000")
            .stdout(std::process::Stdio::from(log_out))
            .stderr(std::process::Stdio::from(log_err));
        hide_window_std(&mut cmd);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start tunrelay ({}): {}", relay_path.display(), e))?;

        if let Some(ref guard) = self.job_guard {
            let _ = guard.assign_process(&child);
        }

        // Verify it stays alive past init (bad fd / missing binary exit instantly).
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        let exited = {
            let mut lock = self.tunrelay_process.lock();
            *lock = Some(child);
            if let Some(ref mut c) = *lock {
                match c.try_wait() {
                    Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
                    _ => None,
                }
            } else {
                None
            }
        };
        if let Some(code) = exited {
            let tail = std::fs::read_to_string(self.app_data_dir.join("tunrelay.log"))
                .unwrap_or_default();
            *self.tunrelay_process.lock() = None;
            return Err(format!(
                "tunrelay exited on startup (code {}):\n{}",
                code,
                tail.trim()
            ));
        }

        log::info!("Android: tunrelay running (pid {})", self.tunrelay_process.lock().as_ref().map(|c| c.id()).unwrap_or(0));
        Ok(())
    }
}
