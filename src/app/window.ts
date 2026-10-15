import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WINDOW_MIN } from "@/features/session/layout";

/** 把主窗口带到前台，失败忽略。 */
export async function revealWindow() {
  const win = getCurrentWindow();
  for (const step of [() => win.unminimize(), () => win.show(), () => win.setFocus()]) await step().catch(() => {});
}

/** 设定窗口最小宽度（逻辑像素），失败忽略。 */
export async function setMinWidth(width: number) {
  await getCurrentWindow().setMinSize(new LogicalSize(width, WINDOW_MIN.height)).catch(() => {});
}
