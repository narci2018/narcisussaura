//! Android 平台专属实现。
//!
//! 目录级平台隔离约定（见仓库根 AGENTS.md）：所有 android 独有的 Rust 逻辑
//! 集中在本文件（及 `mobile/android/` 下的 Kotlin 侧），共享代码里只允许
//! 存在极薄的 `#[cfg(target_os = "android")]` 调用缝——缝里不许写业务逻辑。
//!
//! 唯一故意不加 cfg 门控的项是 [`ConnectionManager::patch_android_relay_config`]：
//! 集成测试 `tests/android_smart_config.rs` 必须在主机上调用它复现手机配置，
//! 再用 `sing-box check` 验证（CI 不编译 android cfg 分支、手机没有日志，
//! 门控住就只能等真机翻车）。

use crate::managers::connection_manager::ConnectionManager;

impl ConnectionManager {
    /// Android TUN mode: a standalone sing-box process cannot open /dev/net/tun
    /// (SELinux) and has no fd-based tun option. Strip the tun inbound entirely
    /// and let the tunrelay child process bridge the VpnService fd to the
    /// SOCKS5 mixed port.
    ///
    /// `auto_detect_interface` must also be off: it forces sing-box to open a
    /// netlink socket at startup, which Android SELinux bans for app uids and
    /// turns into a fatal "create network monitor" error. With it false, sing-box
    /// tolerates the missing monitor and starts normally (route/network.go).
    ///
    /// pub + cfg-free so an integration test can validate the exact Android
    /// config with `sing-box check` on the host (CI only compiles Kotlin).
    pub fn patch_android_relay_config(config_str: &str) -> Result<String, String> {
        let mut config: serde_json::Value = serde_json::from_str(config_str)
            .map_err(|e| format!("Failed to parse config for Android relay patch: {}", e))?;

        if let Some(inbounds) = config.get_mut("inbounds").and_then(|v| v.as_array_mut()) {
            inbounds.retain(|ib| ib.get("type").and_then(|v| v.as_str()) != Some("tun"));
        }
        if let Some(route) = config.get_mut("route").and_then(|v| v.as_object_mut()) {
            route.insert(
                "auto_detect_interface".to_string(),
                serde_json::json!(false),
            );
        }
        log::info!("Android: stripped tun inbound, disabled auto_detect_interface");

        serde_json::to_string(&config).map_err(|e| format!("Failed to serialize patched config: {}", e))
    }
}

#[cfg(target_os = "android")]
mod android_only {
    use super::ConnectionManager;
    use crate::platform::hide_window_std;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::Ordering;
    use tauri::{AppHandle, Emitter};

    impl ConnectionManager {
        /// On Android, trigger VpnService and wait for the TUN fd file to appear.
        /// Returns the fd number on success, or None on non-Android / proxy-only mode.
        ///
        /// The whole handshake is FILE-DRIVEN on purpose: MIUI can kill the
        /// VpnService mid-handshake, so no in-process signal is trustworthy.
        /// - `vpn_pending` carries "new" for the first signal and "again" for the
        ///   ~4s re-signals. The app-process watchdog in VpnInitProvider owns the
        ///   consent dialog itself, in the exact v2rayNG order: foreground
        ///   Activity + startActivityForResult(prepare()) FIRST, and the
        ///   VpnService is only started once consent exists. The service-side
        ///   fallback is the tappable consent notification + Settings guidance;
        ///   it never launches the dialog itself. Every re-signal that finds
        ///   consent granted establishes the tunnel immediately, so the user
        ///   never has to tap 连接 twice after allowing.
        pub(crate) async fn wait_for_android_tun_fd(&self, my_gen: u64, app: &AppHandle) -> Option<i32> {
            let fd_file = self.app_data_dir.join("tun_fd");
            let pending_file = self.app_data_dir.join("vpn_pending");
            let status_file = self.app_data_dir.join("vpn_status");
            let stop_file = self.app_data_dir.join("vpn_stop");
            let _ = std::fs::remove_file(&fd_file);
            // Drop the previous session's stage report so diagnostics can never
            // show a stale stage from an earlier connect attempt.
            let _ = std::fs::remove_file(&status_file);

            log::info!("Android: writing vpn_pending signal for VpnService...");
            // "new" tells the Kotlin watchdog this is a fresh user tap: it re-arms
            // the v2rayNG-style consent dialog launch. The 4s re-signals below use
            // "again" so the watchdog never storms the dialog behind our back.
            let _ = std::fs::write(&pending_file, "new");

            // Up to 180s: consent now happens in the system Settings app, which
            // means the user leaves our app, navigates pages, taps 允许, and comes
            // back — budget for that round trip.
            let mut last_stage = String::new();
            for i in 0..720 {
                // Abandon promptly if the user hit terminate (disconnect bumps the
                // generation) or a newer connect superseded this one; otherwise the
                // UI is frozen on "Connecting" for the full timeout and cannot be stopped.
                if self.connect_generation.load(Ordering::SeqCst) != my_gen {
                    let _ = std::fs::remove_file(&pending_file);
                    log::info!("Android: tun fd wait cancelled (generation changed)");
                    return None;
                }
                if let Ok(content) = std::fs::read_to_string(&fd_file) {
                    if let Ok(fd) = content.trim().parse::<i32>() {
                        log::info!("Android: received TUN fd={} after {}ms", fd, i * 250);
                        let _ = std::fs::remove_file(&fd_file);
                        // The periodic re-signal may have raced the fd write; drop
                        // it so the watchdog does not restart a handshake that is
                        // already complete.
                        let _ = std::fs::remove_file(&pending_file);
                        return Some(fd);
                    }
                }
                // Self-heal the service side: MIUI kills background services freely.
                // Re-signalling makes the watchdog start it again; the service
                // treats a re-entry as the same attempt (no consent-dialog storm).
                // This is ALSO the auto-continue mechanism: the moment the user
                // grants consent in system Settings, the next re-entry's
                // prepare() returns null and the tunnel is built without any
                // further tap from the user.
                if i % 16 == 8 {
                    let _ = std::fs::write(&pending_file, "again");
                }
                // Surface the tunnel service's current stage to the UI every ~2s
                // while we wait. Without this an early terminate shows nothing at
                // all, and a stuck handshake is undiagnosable from the phone.
                if i % 8 == 4 {
                    let stage = match std::fs::read_to_string(&status_file) {
                        Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
                        _ => "service_starting".to_string(),
                    };
                    // Fatal stages: the service already gave up for a reason the
                    // user cannot recover from within this attempt (FGS promise
                    // could not be kept, the service was never allowed to start).
                    // NOTE: every consent_* stage is NOT fatal — it just means
                    // consent is still missing, and the UI text points the user
                    // at system Settings to grant it.
                    if stage.starts_with("fgs_start_failed")
                        || stage.starts_with("service_start_failed")
                    {
                        let _ = std::fs::remove_file(&pending_file);
                        // Tell the service to drop tunnelRequested; otherwise it
                        // re-raises the consent flow on every activity resume.
                        let _ = std::fs::write(&stop_file, "1");
                        log::error!("Android: VpnService reported fatal stage {}, abandoning", stage);
                        let _ = app.emit("core:vpn-stage", stage.clone());
                        return None;
                    }
                    if stage != last_stage {
                        last_stage = stage.clone();
                        log::info!("Android: vpn tunnel stage -> {}", stage);
                        let _ = app.emit("core:vpn-stage", stage);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            let _ = std::fs::remove_file(&pending_file);
            let _ = std::fs::write(&stop_file, "1");
            let diag = self.android_vpn_diag();
            log::error!("Android: timed out waiting for TUN fd from VpnService (180s). {}", diag);
            None
        }

        /// Read the VpnService's self-reported stage (written to vpn_status by the
        /// Kotlin side) so a stuck tunnel surfaces an actionable reason in-app
        /// instead of a generic timeout.
        fn android_vpn_diag(&self) -> String {
            match std::fs::read_to_string(self.app_data_dir.join("vpn_status")) {
                Ok(s) if !s.trim().is_empty() => format!("（隧道服务状态: {}）", s.trim()),
                _ => "（隧道服务无响应：VpnService 可能已被系统冻结，请在系统设置中允许本应用无限制/自启动后台运行后重试）".to_string(),
            }
        }

        /// The uniform user-facing error for "no tunnel = no VPN consent",
        /// shared by all three connect paths (single node / chain / smart
        /// group) so the guidance text can never drift between them.
        pub(crate) fn android_vpn_consent_error(&self) -> String {
            format!(
                "无法建立 VPN 隧道：缺少系统 VPN 授权。请打开手机「设置 → 连接与共享（或 更多连接）→ VPN」，点击「Narcissus Aura」，在弹窗中点「允许」并勾选「不再询问」，然后回到本应用重新点击连接{}",
                self.android_vpn_diag()
            )
        }

        /// Parse the newest tunrelay `stats` line for bytes actually relayed
        /// (bytes_tcp + bytes_udp). The relay counts what it moved itself, so a
        /// dead bridge can't fake success and a busy one can't be missed the way
        /// Clash's downloadTotal (which skips some direct/dns flows) could.
        fn relay_total_bytes(log_text: &str) -> u64 {
            let parse = |hay: &str, key: &str| -> u64 {
                hay.split_once(key)
                    .map(|(_, rest)| rest.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0))
                    .unwrap_or(0)
            };
            log_text
                .lines()
                .rev()
                .find(|l| l.contains("stats ") && l.contains("bytes_tcp="))
                .map(|l| parse(l, "bytes_tcp=") + parse(l, "bytes_udp="))
                .unwrap_or(0)
        }

        /// Count packets the OS pushed into the tunnel: prefer the newest stats
        /// line's flow counters, fall back to the per-connection `flow` lines
        /// (which appear before the first 2s stats tick).
        fn relay_total_flows(log_text: &str) -> u64 {
            let parse = |hay: &str, key: &str| -> u64 {
                hay.split_once(key)
                    .map(|(_, rest)| rest.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0))
                    .unwrap_or(0)
            };
            let stats_flows = log_text
                .lines()
                .rev()
                .find(|l| l.contains("stats ") && l.contains("tcp_flows="))
                .map(|l| parse(l, "tcp_flows=") + parse(l, "udp_flows="))
                .unwrap_or(0);
            let line_flows = log_text
                .lines()
                .filter(|l| l.contains(" flow ") || l.contains("flow udp#") || l.contains("flow tcp#"))
                .count() as u64;
            stats_flows.max(line_flows)
        }

        /// Spawn the tunrelay helper (Android-only): a static-Go gVisor netstack
        /// that terminates TCP/UDP on the inherited VpnService fd and forwards all
        /// flows into sing-box's SOCKS5 mixed port.
        pub(crate) async fn start_tunrelay(&self, app: &AppHandle, tun_fd: i32, mixed_port: u16) -> Result<(), String> {
            let relay_path = self.locate_binary(app, "tunrelay")?;

            // The service may have re-established the tunnel since
            // wait_for_android_tun_fd ran (MIUI loves restarting services); the fd
            // number read back then can now point at a closed pipe. Re-read the
            // handshake file and prefer its value.
            let tun_fd = match std::fs::read_to_string(self.app_data_dir.join("tun_fd"))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
            {
                Some(fresh) if fresh > 0 && fresh != tun_fd => {
                    log::warn!("Android: tun fd changed {} -> {}", tun_fd, fresh);
                    fresh
                }
                _ => tun_fd,
            };

            // Validate the fd before spawning. gVisor's fdbased used to stop its
            // read dispatcher silently on a bad fd — relay alive, zero packets.
            if unsafe { libc::fcntl(tun_fd, libc::F_GETFD) } == -1 {
                let errno = std::io::Error::last_os_error();
                return Err(format!(
                    "隧道 fd {} 已失效（{}），VPN 服务可能重建了隧道，请重新点击连接",
                    tun_fd, errno
                ));
            }

            // The child inherits the tun fd across exec only if FD_CLOEXEC is clear.
            unsafe {
                libc::fcntl(tun_fd, libc::F_SETFD, 0);
            }

            // Wait for the core's mixed/SOCKS port to accept connections (up to
            // 10s). If it never listens, tunrelay would forward into nothing and
            // the bridge check below could only report a misleading "no traffic"
            // — fail here with the real reason instead.
            let mut ready = false;
            for _ in 0..40 {
                if std::net::TcpStream::connect(("127.0.0.1", mixed_port)).is_ok() {
                    ready = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            if !ready {
                return Err(format!(
                    "[核心端口未就绪] 代理核心未在 10 秒内监听本地端口 {}（核心启动失败或上游握手卡死）",
                    mixed_port
                ));
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

            // Verify the bridge actually moves traffic. Baseline the core's byte
            // counters, then watch for growth: the OS and apps fire DNS /
            // connectivity chatter the instant a VPN interface comes up, so five
            // seconds of total silence means the bridge is dead. A relay that dies
            // (gVisor now hard-exits on fd errors via ClosedFunc) is caught the
            // same moment, with its log attached.
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            *self.tunrelay_process.lock() = Some(child);
            let baseline = Self::relay_total_bytes(
                &std::fs::read_to_string(self.app_data_dir.join("tunrelay.log")).unwrap_or_default(),
            );
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                let exited = {
                    let mut lock = self.tunrelay_process.lock();
                    if let Some(ref mut c) = *lock {
                        match c.try_wait() {
                            Ok(Some(status)) => {
                                *lock = None;
                                Some(status.code().unwrap_or(-1))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                };
                let tail = || std::fs::read_to_string(self.app_data_dir.join("tunrelay.log")).unwrap_or_default();
                if let Some(code) = exited {
                    let log_now = tail();
                    log::warn!("Android: tunrelay exited (code {}), log tail:\n{}", code, log_now.trim());
                    return Err(format!(
                        "[桥接进程退出] tunrelay 桥接进程异常退出（退出码 {}），完整日志请用「拷贝完整日志」",
                        code
                    ));
                }
                if Self::relay_total_bytes(&tail()) > baseline {
                    log::info!("Android: tunrelay bridge verified (bytes relayed through fd)");
                    return Ok(());
                }
                if tokio::time::Instant::now() >= deadline {
                    let log_now = tail();
                    let flows = Self::relay_total_flows(&log_now);
                    log::warn!("Android: bridge silent after 5s (flows={}), log tail:\n{}", flows, log_now.trim());
                    // flows>0 with zero bytes: the OS routed into the tunnel but
                    // the core never forwarded anything — a node problem.
                    // flows==0: the OS never used this tunnel at all — a system
                    // routing problem, NOT the node's fault; don't mislabel it.
                    return Err(if flows > 0 {
                        "[核心未转发流量] 系统已向隧道发送数据包，但代理核心 5 秒内未转发任何字节".to_string()
                    } else {
                        "[系统隧道无流量] 5 秒内系统未向 VPN 隧道发送任何数据包（隧道未被选为默认网络）".to_string()
                    });
                }
            }
        }

        /// Tear the VpnService tunnel down and wait for the service to confirm
        /// "standby" before returning. Without this settle gap the next connect's
        /// establish() can land within milliseconds of the old tunnel's close (the
        /// 400ms watchdog dispatches both signals back-to-back) and Android leaves
        /// the fresh tunnel off the default route — tunrelay then sees zero system
        /// flows for its whole 5s window, exactly the v0.2.103 field failure where
        /// one residential/VPNGate error poisoned every later connect.
        pub(crate) async fn android_teardown_tunnel_settled(&self) {
            let dir = &self.app_data_dir;
            let _ = std::fs::remove_file(dir.join("vpn_status"));
            vpn_teardown_files(dir);
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1500);
            loop {
                if let Ok(s) = std::fs::read_to_string(dir.join("vpn_status")) {
                    if s.trim() == "standby" {
                        break;
                    }
                }
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            // Extra grace for the OS to finish deprovisioning the old network.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }

    /// No pkill on Android; scan /proc (our own uid's processes are
    /// visible) and SIGKILL orphaned cores from a previous crash.
    pub(crate) fn kill_orphan_cores(binaries: &[&str]) {
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

    /// Disconnect teardown: drop the tun-fd handshake files and tell the
    /// VpnService to tear the tunnel down; the service keeps running so
    /// a later connect can re-hand it over without a process restart.
    pub(crate) fn vpn_teardown_files(app_data_dir: &Path) {
        let _ = std::fs::remove_file(app_data_dir.join("tun_fd"));
        let _ = std::fs::remove_file(app_data_dir.join("vpn_pending"));
        let _ = std::fs::write(app_data_dir.join("vpn_stop"), "1");
    }

    /// Android-only binary search paths, prepended so they win over the
    /// desktop candidates. dataDir is mounted noexec for targetSdk>=29; the
    /// only reliably executable location is nativeLibraryDir, where the
    /// installer extracts our lib<name>.so pseudo-libs (written by
    /// VpnInitProvider).
    pub(crate) fn prepend_binary_candidates(
        app: &AppHandle,
        base_name: &str,
        bin_name: &str,
        candidates: &mut Vec<PathBuf>,
    ) {
        use tauri::Manager;
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            if let Ok(nd) = std::fs::read_to_string(app_data_dir.join("native_lib_dir")) {
                let nd = nd.trim();
                if !nd.is_empty() {
                    candidates.push(PathBuf::from(nd).join(format!("lib{}.so", base_name)));
                }
            }
            candidates.push(app_data_dir.join("binaries").join(bin_name));
            candidates.push(app_data_dir.join(bin_name));
        }
    }

    pub(crate) fn prepend_server_entries_candidates(app: &AppHandle, candidates: &mut Vec<PathBuf>) {
        use tauri::Manager;
        if let Ok(app_data_dir) = app.path().app_data_dir() {
            candidates.push(app_data_dir.join("binaries").join("server_entries.txt"));
            candidates.push(app_data_dir.join("server_entries.txt"));
        }
    }

    pub(crate) fn prepend_rule_search_dirs(app_data_dir: &Path, dirs: &mut Vec<PathBuf>) {
        dirs.push(app_data_dir.join("binaries").join("rules"));
        dirs.push(app_data_dir.join("rules"));
        dirs.push(app_data_dir.join("binaries"));
    }

    /// Android has no adb access for us, so a Rust panic must leave a
    /// readable trace behind: write it to panic_log, surfaced in the UI
    /// on the next launch via get_crash_report.
    pub(crate) fn install_panic_hook(app_data_dir: &Path) {
        let panic_dir = app_data_dir.to_path_buf();
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::fs::write(panic_dir.join("panic_log"), format!("{info}"));
        }));
    }

    pub(crate) fn read_crash_report(app_data_dir: &Path) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        for name in ["crash_log", "panic_log"] {
            let f = app_data_dir.join(name);
            if let Ok(s) = std::fs::read_to_string(&f) {
                if !s.trim().is_empty() {
                    parts.push(format!("[{}] {}", name, s.trim()));
                }
                let _ = std::fs::remove_file(&f);
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }
}

#[cfg(target_os = "android")]
pub(crate) use android_only::{
    install_panic_hook, kill_orphan_cores, prepend_binary_candidates,
    prepend_rule_search_dirs, prepend_server_entries_candidates, read_crash_report, vpn_teardown_files,
};
