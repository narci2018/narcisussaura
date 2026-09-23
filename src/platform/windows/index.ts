import type { PlatformProfile } from '../types';
import { pcProfile } from '../pc';

// Windows 前端差异代码放这里。
// Rust 侧在 src-tauri/src/platform/windows.rs(+ job_object / win_proxy)。
export const windowsProfile: PlatformProfile = {
  ...pcProfile,
};
