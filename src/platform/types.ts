import type { FC } from 'react';
import type { ActiveTab } from '../types';

// windowsProfile / macosProfile / androidProfile 的差异契约。
export interface PlatformProfile {
  /** 是否渲染桌面窗口控件(最小化/最大化/关闭 + 自定义标题栏拖拽区)。
   *  移动端为 false:没有桌面窗口,拖拽区还会吞掉触摸事件。 */
  windowChrome: boolean;
  /** 专家模式视图层:PC(win+mac)与 Android 各自持有一套物理独立的实现,
   *  互不共享;改手机端专家 UI 只允许动 src/platform/android/expert/。 */
  expert: ExpertLayer;
}

/** 一个平台专家模式的完整视图注册表。
 *  views 以 ActiveTab 为键,新增 tab 时两套实现都会被类型系统强制补齐。 */
export interface ExpertLayer {
  Navigation: FC;
  views: Record<ActiveTab, FC>;
}
