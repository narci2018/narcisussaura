import type { PlatformProfile } from '../types';
import { pcProfile } from '../pc';

// macOS 前端差异代码放这里。
// Rust 侧在 src-tauri/src/platform/macos.rs。
export const macosProfile: PlatformProfile = {
  ...pcProfile,
};
