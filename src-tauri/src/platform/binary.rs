/// Returns the platform-specific executable binary name.
/// On Windows, appends `.exe`.
/// On Unix, macOS, Android, returns the base name without extension.
pub fn core_binary_name(base: &str) -> String {
    #[cfg(windows)]
    {
        if base.ends_with(".exe") {
            base.to_string()
        } else {
            format!("{}.exe", base)
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(stripped) = base.strip_suffix(".exe") {
            stripped.to_string()
        } else {
            base.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_binary_name() {
        let name = core_binary_name("sing-box");
        #[cfg(windows)]
        assert_eq!(name, "sing-box.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "sing-box");
    }
}
