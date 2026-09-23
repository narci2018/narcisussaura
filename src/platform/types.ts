export interface PlatformProfile {
  /** 是否渲染桌面窗口控件(最小化/最大化/关闭 + 自定义标题栏拖拽区)。
   *  移动端为 false:没有桌面窗口,拖拽区还会吞掉触摸事件。 */
  windowChrome: boolean;
}
