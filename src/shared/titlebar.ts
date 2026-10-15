// macOS：标题栏为 Overlay，页面延伸到顶边，需为红绿灯按钮让位。
// Windows：去掉系统标题栏，由 TitleBar.vue 自绘。其他平台保留原生标题栏。
import { getCurrentWindow } from "@tauri-apps/api/window";
import { IS_MAC, IS_WINDOWS } from "@/shared/platform";

export const OVERLAY_TITLEBAR = IS_MAC;
/** 窗口透明并带系统毛玻璃（仅 macOS），页面底色为半透明。 */
export const VIBRANCY = OVERLAY_TITLEBAR;
export const CUSTOM_TITLEBAR = IS_WINDOWS;

/** 通过 traffic-lights 类为红绿灯按钮让位；全屏时按钮隐藏，无需让位。 */
export async function installTitlebar() {
  if (!OVERLAY_TITLEBAR) return;
  const root = document.documentElement.classList;
  root.add("traffic-lights");
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | undefined;
  // 全屏切换表现为尺寸变化，防抖处理。
  await win.onResized(() => {
    clearTimeout(timer);
    timer = setTimeout(async () => root.toggle("traffic-lights", !(await win.isFullscreen())), 120);
  });
  root.toggle("traffic-lights", !(await win.isFullscreen()));
}
