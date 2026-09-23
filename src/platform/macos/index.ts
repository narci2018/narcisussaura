import type { PlatformProfile } from '../types';

// macOS 前端差异代码放这里。
// Rust 侧在 src-tauri/src/platform/macos.rs。
export const macosProfile: PlatformProfile = {
  windowChrome: true,
};
