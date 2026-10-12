// 关闭主窗口即退出应用，所有 SSH 会话随之断开；因此有会话打开时，
// 两种退出方式（关闭窗口、菜单里的“退出”）都先询问。
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../shared/i18n";
import { confirm } from "../shared/ui";
import { useApp } from "../stores/app";
import { revealWindow } from "./window";

/** 正在显示的询问（如果有）：重复询问不能叠出两个对话框。 */
let asking: Promise<boolean> | null = null;

/** 还连着的会话数：退出或重启应用会把它们全部断开。 */
export const openSessionCount = () => useApp().tabs.filter((tab) => tab.status !== "closed").length;

/** 应用能否退出：没有会话时立即得出结果，否则等用户回答。 */
function mayClose(): Promise<boolean> {
  const count = openSessionCount();
  if (!count) return Promise.resolve(true);
  if (!asking) {
    // 窗口最小化时也能选“退出”，必须让用户看到这个询问。
    void revealWindow();
    asking = confirm(t("app.quitTitle"), t("app.quitBody", { count }), { danger: true, ok: t("app.quitOk") }).finally(
      () => (asking = null),
    );
  }
  return asking;
}

/** 让窗口的关闭按钮（以及 ⌘W）在关闭前先询问。 */
export async function installCloseGuard() {
  await getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await mayClose())) event.preventDefault();
  });
}

/** 退出应用，有会话打开时先询问。主窗口销毁后应用即退出。 */
export async function quit() {
  if (await mayClose()) await getCurrentWindow().destroy();
}
