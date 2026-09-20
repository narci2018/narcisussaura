pub mod binary;
pub mod command;
pub mod process_guard;
pub mod proxy;

#[cfg(windows)]
pub mod job_object;
#[cfg(windows)]
pub mod win_proxy;

pub use binary::core_binary_name;
pub use command::{hide_window, hide_window_std};
pub use process_guard::ProcessGuard;
pub use proxy::PlatformProxy;
