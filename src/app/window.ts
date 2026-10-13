// 主窗口可能被最小化，或压在别的窗口下面。从菜单栏发起的操作，结果要让用户看得见。
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WINDOW_MIN } from "../features/session/layout";

/** 让主窗口回到眼前：取消最小化、显示出来并取得焦点。尽力而为，失败不影响接下来要做的事。 */
export async function revealWindow() {
  const win = getCurrentWindow();
  for (const step of [() => win.unminimize(), () => win.show(), () => win.setFocus()]) await step().catch(() => {});
}

/** 设定窗口的最小宽度（逻辑像素）。窗口此刻比它窄的话，系统会把窗口撑到这个宽度。尽力而为。 */
export async function setMinWidth(width: number) {
  await getCurrentWindow().setMinSize(new LogicalSize(width, WINDOW_MIN.height)).catch(() => {});
}
