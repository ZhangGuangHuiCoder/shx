// 应用更新：启动时静默检查一次（跳过的版本不再问），之后由菜单手动检查。
// 更新包签名由后端用 tauri.conf.json 中的公钥校验。
import { CircleArrowUp } from "@lucide/vue";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { errorMessage } from "@/shared/api";
import { t } from "@/shared/i18n";
import { readStored, STORAGE } from "@/shared/settings";
import { ask, confirm, message } from "@/shared/ui";
import { quitConsequences } from "@/app/closeGuard";

const RELEASES = "https://github.com/ZhangGuangHuiCoder/shx/releases/latest";
/** 检查超时（毫秒）。 */
const CHECK_TIMEOUT = 20_000;

/** 同一时间只跑一个更新流程。 */
let busy = false;
/** 已下载、待重启安装的更新。 */
let downloaded: Update | null = null;

function skip(version: string) {
  try {
    localStorage.setItem(STORAGE.skippedUpdate, JSON.stringify(version));
  } catch {
  }
}

/** 不安装时释放后端资源。 */
const discard = (update: Update) => void update.close().catch(() => {});

/** 询问后安装并重启；拒绝则保留更新，下次检查再问。 */
async function restart(update: Update) {
  const consequences = quitConsequences();
  const body = t("update.restartBody", { version: update.version }) + (consequences ? `\n${consequences}` : "");
  const agreed = await confirm(t("update.restartTitle"), body, { icon: CircleArrowUp, danger: !!consequences, ok: t("update.restart") });
  if (!agreed) return void message.info(t("update.later"));
  try {
    // Windows 安装程序会结束当前进程并自行重启应用，走不到下一行。
    await update.install();
    await relaunch();
  } catch (e) {
    message.error(t("update.failed", { error: errorMessage(e) }));
  }
}

/** `quiet` 时只在有需要询问的新版本时出声。 */
async function run(quiet: boolean) {
  if (downloaded) return restart(downloaded);

  const checking = quiet ? null : message.loading(t("update.checking"), { duration: 0 });
  let update: Update | null;
  try {
    update = await check({ timeout: CHECK_TIMEOUT });
  } catch (e) {
    return quiet ? undefined : void message.error(t("update.checkFailed", { error: errorMessage(e) }));
  } finally {
    checking?.destroy();
  }
  if (!update) return quiet ? undefined : void message.success(t("update.upToDate", { version: await getVersion() }));
  // 手动检查时不理会已跳过的版本。
  if (quiet && update.version === readStored<string | null>(STORAGE.skippedUpdate, null)) return discard(update);

  const answer = await ask({
    icon: CircleArrowUp,
    title: t("update.available", { version: update.version }),
    detail: t("update.current", { version: update.currentVersion }),
    content: t("update.body"),
    actions: [
      { value: "skip", label: t("update.skip") },
      { value: "later", label: t("update.remindLater") },
      { value: "install", label: t("update.now"), tone: "primary" },
    ],
  });
  if (answer === "skip") skip(update.version);
  if (answer !== "install") return discard(update);
  // 开发模式下安装会解压到 target 目录，改为打开下载页。
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

async function start(quiet: boolean) {
  if (busy) return;
  busy = true;
  try {
    await run(quiet);
  } finally {
    busy = false;
  }
}

/** 手动检查更新，结果都会提示。 */
export const checkForUpdates = () => start(false);

/** 启动时静默检查；开发模式不检查。 */
export function checkForUpdatesOnLaunch() {
  if (!import.meta.env.DEV) void start(true).catch(() => {});
}
