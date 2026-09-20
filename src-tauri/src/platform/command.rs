/// Helper to hide the terminal window of child command processes across platforms.
/// On Windows, sets `CREATE_NO_WINDOW` (0x08000000).
/// On macOS / Linux / Android, this is a no-op as processes do not create consoles by default.

#[allow(unused_mut)]
pub fn hide_window(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd
}

#[allow(unused_mut)]
pub fn hide_window_std(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
