// 关闭主窗口即退出应用，会断开所有会话和隧道，所以有会话或隧道时先询问。
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "@/shared/i18n";
import { confirm } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { revealWindow } from "@/app/window";

/** 进行中的询问，避免叠出多个对话框。 */
let asking: Promise<boolean> | null = null;

const openSessionCount = () => useApp().tabs.filter((tab) => tab.status !== "closed").length;

/** 退出会造成的影响，用于询问文案；没有时为空串。 */
export function quitConsequences(): string {
  const count = openSessionCount();
  const lines = [count ? t("app.quitBody", { count }) : "", useApp().tunnel ? t("app.quitTunnel") : ""];
  return lines.filter(Boolean).join("\n");
}

function mayClose(): Promise<boolean> {
  const consequences = quitConsequences();
  if (!consequences) return Promise.resolve(true);
  if (!asking) {
    // 窗口可能已最小化，先让询问可见。
    void revealWindow();
    asking = confirm(t("app.quitTitle"), consequences, { danger: true, ok: t("app.quitOk") }).finally(() => (asking = null));
  }
  return asking;
}

export async function installCloseGuard() {
  await getCurrentWindow().onCloseRequested(async (event) => {
    if (!(await mayClose())) event.preventDefault();
  });
}

export async function quit() {
  if (await mayClose()) await getCurrentWindow().destroy();
}
