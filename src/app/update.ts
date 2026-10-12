// 应用更新：从菜单检查 GitHub Releases 上有没有新版本，有就询问；下载完成后重启生效。
// 只在用户要求时检查，应用不会自己去检查。更新包的签名由后端用 tauri.conf.json 里的公钥校验。
import { CircleArrowUp } from "@lucide/vue";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { errorMessage } from "../shared/api";
import { t } from "../shared/i18n";
import { ask, confirm, message } from "../shared/ui";
import { openSessionCount } from "./closeGuard";

const RELEASES = "https://github.com/ZhangGuangHuiCoder/shx/releases/latest";
/** 连不上 GitHub 时，检查最多等这么久（毫秒）。 */
const CHECK_TIMEOUT = 20_000;

/** 一次只走一个更新流程：上一次检查还没结束又从菜单点了一次，不能叠出两个对话框。 */
let busy = false;
/** 已经下载好、只等重启的更新。 */
let downloaded: Update | null = null;

/** 更新对象在后端占着资源，不安装就要释放。 */
const discard = (update: Update) => void update.close().catch(() => {});

/** 询问是否重启；确认后安装并重启。不重启的话更新留着，下次检查时接着问。 */
async function restart(update: Update) {
  const count = openSessionCount();
  const body = t("update.restartBody", { version: update.version }) + (count ? `\n${t("app.quitBody", { count })}` : "");
  const agreed = await confirm(t("update.restartTitle"), body, { icon: CircleArrowUp, danger: count > 0, ok: t("update.restart") });
  if (!agreed) return void message.info(t("update.later"));
  try {
    // Windows 上安装程序会结束当前进程，装完自己重新打开应用，走不到下一行。
    await update.install();
    await relaunch();
  } catch (e) {
    message.error(t("update.failed", { error: errorMessage(e) }));
  }
}

async function run() {
  if (downloaded) return restart(downloaded);

  const checking = message.loading(t("update.checking"), { duration: 0 });
  let update: Update | null;
  try {
    update = await check({ timeout: CHECK_TIMEOUT });
  } catch (e) {
    return void message.error(t("update.checkFailed", { error: errorMessage(e) }));
  } finally {
    checking.destroy();
  }
  if (!update) return void message.success(t("update.upToDate", { version: await getVersion() }));

  const answer = await ask({
    icon: CircleArrowUp,
    title: t("update.available", { version: update.version }),
    detail: t("update.current", { version: update.currentVersion }),
    content: t("update.body"),
    actions: [
      { value: "later", label: t("common.cancel") },
      { value: "install", label: t("update.now"), tone: "primary" },
    ],
  });
  if (answer !== "install") return discard(update);
  // 开发时运行的不是安装好的应用，安装更新会把更新包解压到 target 目录里；改为打开下载页。
  if (import.meta.env.DEV) {
    discard(update);
    return void openUrl(RELEASES);
  }

  const progress = message.loading(t("update.downloading"), { duration: 0 });
  try {
    let total = 0;
    let received = 0;
    await update.download((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event !== "Progress" || !total) return;
      received += event.data.chunkLength;
      progress.content = `${t("update.downloading")} ${Math.floor((received / total) * 100)}%`;
    });
  } catch (e) {
    message.error(t("update.failed", { error: errorMessage(e) }));
    return discard(update);
  } finally {
    progress.destroy();
  }
  downloaded = update;
  await restart(update);
}

/** 检查更新，由用户从菜单发起：“已是最新”和检查失败也都告诉用户。 */
export async function checkForUpdates() {
  if (busy) return;
  busy = true;
  try {
    await run();
  } finally {
    busy = false;
  }
}
