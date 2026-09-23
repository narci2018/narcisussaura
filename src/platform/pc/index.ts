import type { PlatformProfile } from '../types';
import { pcExpert } from './expert';

// PC(windows+macOS)共用形态。真正的平台差异(rust/窗口行为)在
// src/platform/windows/ 与 src/platform/macos/ 各自目录里覆盖。
export const pcProfile: PlatformProfile = {
  windowChrome: true,
  expert: pcExpert,
};
