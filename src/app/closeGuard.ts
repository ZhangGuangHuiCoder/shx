// 关闭主窗口不再直接退出：可选择后台运行（保留会话与隧道）或断开退出。
// 有会话或隧道时先询问；托盘“退出”也走同一流程（事件 app:quit）。
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TriangleAlert } from "@lucide/vue";
import { t } from "@/shared/i18n";
import { ask } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { hideToBackground, revealWindow } from "@/app/window";

/** 询问结果；关闭/取消为 null。 */
type Choice = "quit" | "background";

/** 进行中的询问，避免叠出多个对话框。 */
let asking: Promise<Choice | null> | null = null;

const openSessionCount = () => useApp().tabs.filter((tab) => tab.status !== "closed").length;

/** 退出会造成的影响，用于询问文案；没有时为空串。 */
export function quitConsequences(): string {
  const count = openSessionCount();
  const lines = [count ? t("app.quitBody", { count }) : "", useApp().tunnel ? t("app.quitTunnel") : ""];
  return lines.filter(Boolean).join("\n");
}

/** 询问如何处置；没有会话和隧道时没什么可留，直接退出。 */
function askQuit(): Promise<Choice | null> {
  const consequences = quitConsequences();
  if (!consequences) return Promise.resolve("quit");
  if (!asking) {
    // 窗口可能已隐藏或最小化，先让询问可见。
    void revealWindow();
    asking = ask<Choice>({
      icon: TriangleAlert,
      danger: true,
      title: t("app.quitTitle"),
      content: consequences,
      actions: [
        { value: "background", label: t("app.quitBackground"), tone: "primary" },
        { value: "quit", label: t("app.quitOk"), tone: "danger" },
      ],
    }).finally(() => (asking = null));
  }
  return asking;
}

export async function installCloseGuard() {
  await getCurrentWindow().onCloseRequested(async (event) => {
    const choice = await askQuit();
    // 退出：放行默认关闭（销毁 → 结束进程）。
    if (choice === "quit") return;
    event.preventDefault();
    if (choice === "background") await hideToBackground();
  });
  // 托盘“退出”：交回统一的询问流程。
  await listen("app:quit", () => void quit());
}

export async function quit() {
  const choice = await askQuit();
  if (choice === "quit") await getCurrentWindow().destroy();
  else if (choice === "background") await hideToBackground();
}
