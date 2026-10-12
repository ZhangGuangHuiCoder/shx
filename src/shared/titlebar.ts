// macOS 上主窗口没有自己的标题栏（tauri.conf.json 中的 `titleBarStyle: "Overlay"`）：
// 页面一直延伸到窗口顶边，红绿灯按钮浮在左上角。界面的顶行要给它们让出位置，
// 同时用来拖动窗口。
//
// Windows 上主窗口没有系统的标题栏和边框（tauri.windows.conf.json 中的 `decorations: false`）：
// 标题栏由页面自己画，菜单、标题和最小化、最大化、关闭按钮都在里面（TitleBar.vue）。
// 其他平台保留原生标题栏，与这些无关。
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 当前平台的窗口按钮是否绘制在页面之上。 */
export const OVERLAY_TITLEBAR = /Macintosh|Mac OS X/.test(navigator.userAgent);
/** 当前平台的标题栏是否由页面自己画。 */
export const CUSTOM_TITLEBAR = /Windows/.test(navigator.userAgent);

/**
 * 为红绿灯按钮留出位置（<html> 上的 `traffic-lights` 类，见 styles.css）；
 * 全屏时 macOS 会隐藏它们，不需要留。
 */
export async function installTitlebar() {
  if (!OVERLAY_TITLEBAR) return;
  const root = document.documentElement.classList;
  root.add("traffic-lights");
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | undefined;
  // 进入和退出全屏都表现为窗口尺寸变化；拖动调整大小的每一步也是。
  await win.onResized(() => {
    clearTimeout(timer);
    timer = setTimeout(async () => root.toggle("traffic-lights", !(await win.isFullscreen())), 120);
  });
  root.toggle("traffic-lights", !(await win.isFullscreen()));
}
