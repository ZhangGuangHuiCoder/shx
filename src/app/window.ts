// 主窗口可能被最小化，或压在别的窗口下面。从菜单栏发起的操作，结果要让用户看得见。
import { getCurrentWindow } from "@tauri-apps/api/window";

/** 让主窗口回到眼前：取消最小化、显示出来并取得焦点。尽力而为，失败不影响接下来要做的事。 */
export async function revealWindow() {
  const win = getCurrentWindow();
  for (const step of [() => win.unminimize(), () => win.show(), () => win.setFocus()]) await step().catch(() => {});
}
