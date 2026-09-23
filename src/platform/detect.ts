// 平台判定唯一入口。规则(AGENTS.md):组件里不允许再写 UA嗅探/平台判断,
// 一律从这里取,保证 android / windows / macos 三端的差异只经由本目录分发。

export type Platform = 'android' | 'ios' | 'windows' | 'macos' | 'linux' | 'unknown';

export function detectPlatform(userAgent: string = navigator.userAgent): Platform {
  const ua = userAgent.toLowerCase();
  if (ua.includes('android')) return 'android';
  if (ua.includes('iphone') || ua.includes('ipad')) return 'ios';
  if (ua.includes('win')) return 'windows';
  if (ua.includes('mac')) return 'macos';
  if (ua.includes('linux')) return 'linux';
  return 'unknown';
}
