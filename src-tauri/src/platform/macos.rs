//! macOS / 桌面 Unix 平台专属实现。
//!
//! 目录级平台隔离约定（见仓库根 AGENTS.md）：macos 独有的 Rust 逻辑集中在
//! 本文件，共享代码里只允许极薄的 `#[cfg(target_os = "macos")]` 调用缝。
//! 本模块整体编译门控是 `unix && !android`（即 macOS；Linux 桌面不在产品
//! 目标内，但 pkill 清理逻辑对它同样成立）。

use std::process::Command;

pub(crate) fn pkill_orphan_cores(binaries: &[&str]) {
    for bin in binaries {
        let _ = Command::new("pkill")
            .args(&["-9", "-f", bin])
            .output();
    }
}

fn get_network_services() -> Vec<String> {
    let output = match std::process::Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            log::warn!("PlatformProxy: failed to run networksetup -listallnetworkservices: {}", e);
            return vec!["Wi-Fi".to_string(), "Ethernet".to_string()];
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // Skip explanatory header lines from networksetup
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.contains("denotes that") {
            continue;
        }
        services.push(trimmed.to_string());
    }

    if services.is_empty() {
        services.push("Wi-Fi".to_string());
    }
    services
}

#[cfg(target_os = "macos")]
pub(crate) fn enable_proxy(port: u16) -> Result<(), String> {
    let services = get_network_services();
    let port_str = port.to_string();

    for service in &services {
        log::info!("PlatformProxy: enabling proxy on macOS service '{}' (port {})", service, port);
        // HTTP Web Proxy
        let _ = std::process::Command::new("networksetup")
            .args(["-setwebproxy", service, "127.0.0.1", &port_str])
            .status();
        let _ = std::process::Command::new("networksetup")
            .args(["-setwebproxystate", service, "on"])
            .status();

        // HTTPS Secure Web Proxy
        let _ = std::process::Command::new("networksetup")
            .args(["-setsecurewebproxy", service, "127.0.0.1", &port_str])
            .status();
        let _ = std::process::Command::new("networksetup")
            .args(["-setsecurewebproxystate", service, "on"])
            .status();

        // SOCKS Proxy
        let _ = std::process::Command::new("networksetup")
            .args(["-setsocksfirewallproxy", service, "127.0.0.1", &port_str])
            .status();
        let _ = std::process::Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", service, "on"])
            .status();

        // Bypass list
        let _ = std::process::Command::new("networksetup")
            .args(["-setproxybypassdomains", service, "127.0.0.1", "localhost", "192.168.0.0/16", "10.0.0.0/8"])
            .status();
    }

    log::info!("PlatformProxy: macOS system proxy configured on port {}", port);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn disable_proxy() -> Result<(), String> {
    let services = get_network_services();

    for service in &services {
        log::info!("PlatformProxy: disabling proxy on macOS service '{}'", service);
        let _ = std::process::Command::new("networksetup")
            .args(["-setwebproxystate", service, "off"])
            .status();
        let _ = std::process::Command::new("networksetup")
            .args(["-setsecurewebproxystate", service, "off"])
            .status();
        let _ = std::process::Command::new("networksetup")
            .args(["-setsocksfirewallproxystate", service, "off"])
            .status();
    }

    log::info!("PlatformProxy: macOS system proxy disabled");
    Ok(())
}
