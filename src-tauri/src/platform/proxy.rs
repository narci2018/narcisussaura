/// Cross-platform System Proxy Manager.
/// Supports Windows (via registry and wininet) and macOS (via networksetup).

pub struct PlatformProxy;

impl PlatformProxy {
    pub fn enable_proxy(port: u16) -> Result<(), String> {
        #[cfg(windows)]
        {
            crate::platform::win_proxy::WindowsProxy::enable_proxy(port)
        }
        #[cfg(target_os = "macos")]
        {
            Self::enable_macos_proxy(port)
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            log::info!("PlatformProxy: enable_proxy not supported or required on this platform (port: {})", port);
            Ok(())
        }
    }

    pub fn disable_proxy() -> Result<(), String> {
        #[cfg(windows)]
        {
            crate::platform::win_proxy::WindowsProxy::disable_proxy()
        }
        #[cfg(target_os = "macos")]
        {
            Self::disable_macos_proxy()
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            log::info!("PlatformProxy: disable_proxy not supported or required on this platform");
            Ok(())
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_network_services() -> Vec<String> {
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
    fn enable_macos_proxy(port: u16) -> Result<(), String> {
        let services = Self::get_macos_network_services();
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
    fn disable_macos_proxy() -> Result<(), String> {
        let services = Self::get_macos_network_services();

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
}
