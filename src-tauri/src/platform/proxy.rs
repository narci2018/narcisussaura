/// Cross-platform System Proxy Manager.
/// Supports Windows (via registry and wininet) and macOS (via networksetup).
/// macOS implementation lives in `crate::platform::macos` (AGENTS.md 平台隔离规则).

pub struct PlatformProxy;

impl PlatformProxy {
    pub fn enable_proxy(port: u16) -> Result<(), String> {
        #[cfg(windows)]
        {
            crate::platform::win_proxy::WindowsProxy::enable_proxy(port)
        }
        #[cfg(target_os = "macos")]
        {
            crate::platform::macos::enable_proxy(port)
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
            crate::platform::macos::disable_proxy()
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            log::info!("PlatformProxy: disable_proxy not supported or required on this platform");
            Ok(())
        }
    }
}
