//! Windows 平台专属实现（Job Object 见 job_object.rs，系统代理见 win_proxy.rs）。
//!
//! 目录级平台隔离约定（见仓库根 AGENTS.md）：windows 独有的 Rust 逻辑集中在
//! 本文件，共享代码里只允许极薄的 `#[cfg(windows)]` 调用缝。

use crate::platform::hide_window_std;
use std::process::Command;

/// Hard-kill any core process left behind by a previous crash before we
/// claim its ports.
pub(crate) fn kill_orphan_cores(binaries: &[&str]) {
    for bin in binaries {
        let bin_exe = format!("{}.exe", bin);
        let mut cmd = Command::new("taskkill");
        cmd.args(&["/F", "/T", "/IM", &bin_exe]);
        hide_window_std(&mut cmd);
        let _ = cmd.output();
    }
}
