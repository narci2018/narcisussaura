import type { PlatformProfile } from '../types';

// Android 前端差异代码放这里(mobile 形态:无窗口控件)。
// Kotlin 侧在 mobile/android/,Rust 侧在 src-tauri/src/platform/android.rs。
export const androidProfile: PlatformProfile = {
  windowChrome: false,
};
