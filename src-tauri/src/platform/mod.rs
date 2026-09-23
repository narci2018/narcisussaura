pub mod binary;
pub mod command;
pub mod process_guard;
pub mod proxy;

// 平台专属实现目录（规则见仓库根 AGENTS.md）：
// android 逻辑集中在 android.rs（其中 patch_android_relay_config 故意不加
// cfg 门控，供主机集成测试复现手机配置）；windows 在 windows.rs +
// job_object.rs + win_proxy.rs；macos 在 macos.rs。
pub mod android;
#[cfg(windows)]
pub mod windows;
#[cfg(all(unix, not(target_os = "android")))]
pub mod macos;

#[cfg(windows)]
pub mod job_object;
#[cfg(windows)]
pub mod win_proxy;

pub use binary::core_binary_name;
pub use command::{hide_window, hide_window_std};
pub use process_guard::ProcessGuard;
pub use proxy::PlatformProxy;

/// Desktop fallback: look up a core binary on PATH (`where.exe` on Windows,
/// `which` on macOS). Android has no PATH lookup — its candidates come from
/// `android::prepend_binary_candidates`.
#[cfg(not(target_os = "android"))]
pub(crate) fn which_on_path(base_name: &str) -> Option<std::path::PathBuf> {
    let which_cmd = if cfg!(windows) { "where.exe" } else { "which" };
    let output = std::process::Command::new(which_cmd).arg(base_name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = std::path::PathBuf::from(stdout.lines().next()?.trim());
    path.exists().then_some(path)
}
