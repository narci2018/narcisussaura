use winreg::enums::*;
use winreg::RegKey;
use std::os::raw::c_void;

#[link(name = "wininet")]
extern "system" {
    fn InternetSetOptionW(
        h_internet: *mut c_void,
        dw_option: u32,
        lp_buffer: *mut c_void,
        dw_buffer_length: u32,
    ) -> i32;
}

const INTERNET_OPTION_SETTINGS_CHANGED: u32 = 39;
const INTERNET_OPTION_REFRESH: u32 = 37;

pub struct WindowsProxy;

impl WindowsProxy {
    pub fn enable_proxy(port: u16) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let settings = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                KEY_SET_VALUE,
            )
            .map_err(|e| format!("Failed to open Internet Settings registry key: {}", e))?;

        let server = format!("127.0.0.1:{}", port);
        settings
            .set_value("ProxyServer", &server)
            .map_err(|e| format!("Failed to set ProxyServer: {}", e))?;
        settings
            .set_value("ProxyEnable", &1u32)
            .map_err(|e| format!("Failed to set ProxyEnable: {}", e))?;
        settings
            .set_value("ProxyOverride", &"<local>;localhost;127.*;10.*;172.16.*;192.168.*")
            .map_err(|e| format!("Failed to set ProxyOverride: {}", e))?;

        Self::refresh_system();
        log::info!("Windows System Proxy enabled: {}", server);
        Ok(())
    }

    pub fn disable_proxy() -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(settings) = hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            KEY_SET_VALUE,
        ) {
            let _ = settings.set_value("ProxyEnable", &0u32);
            Self::refresh_system();
            log::info!("Windows System Proxy disabled");
        }
        Ok(())
    }

    fn refresh_system() {
        unsafe {
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null_mut(), 0);
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_REFRESH, std::ptr::null_mut(), 0);
        }
    }
}
