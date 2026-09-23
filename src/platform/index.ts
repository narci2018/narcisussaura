// 平台差异分发入口(目录级平台隔离的共享内核侧)。
// 组件只允许 import 这里,不允许自行判断平台。

import { detectPlatform } from './detect';
import type { PlatformProfile, ExpertLayer } from './types';
import { androidProfile } from './android';
import { windowsProfile } from './windows';
import { macosProfile } from './macos';

export function currentProfile(userAgent?: string): PlatformProfile {
  // 开发调试用:浏览器里 ?forcePlatform=android 可强制渲染手机端视图
  // (手机布局不依赖视口宽度,桌面窗口内也能准确预览)。正式环境无此参数,零影响。
  const forced = new URLSearchParams(location.search).get('forcePlatform');
  switch (forced === 'android' || forced === 'windows' || forced === 'macos'
    ? forced : detectPlatform(userAgent)) {
    case 'android':
    case 'ios':
      return androidProfile; // iOS 暂非构建目标,与 android 共用移动形态
    case 'macos':
      return macosProfile;
    default:
      return windowsProfile; // windows / linux / unknown:桌面窗口形态
  }
}

export { detectPlatform };
export type { PlatformProfile, ExpertLayer };
