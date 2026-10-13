import { getCurrentWindow } from "@tauri-apps/api/window";
import { createPinia } from "pinia";
import { createApp } from "vue";
import "./styles.css";
import { revealWindow } from "./app/window";
import { fontReady } from "./shared/fonts";
import { applyTheme, themeSetting } from "./shared/theme";
import { installTitlebar } from "./shared/titlebar";
import { hardenWebview } from "./shared/webview";

hardenWebview();
// 首次渲染前先应用保存的配色，避免明暗闪烁。
applyTheme(themeSetting.value);

/**
 * 主窗口创建时是隐藏的（tauri.conf.json 的 `visible: false`），界面画好了才显示：
 * 不然启动时会先看到一下 webview 加载之前的空白。前端没跑起来时由后端兜底显示（lib.rs）。
 */
async function showWhenPainted() {
  const win = getCurrentWindow();
  // 开发时整页重载也会走到这里：窗口已经在了就别再把它拉到前面。
  if (await win.isVisible().catch(() => false)) return;
  // 窗口自己的底色换成当前主题的背景色：窗口露面到第一帧画出来之间，露出来的是它。
  const background = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
  await win.setBackgroundColor(background).catch(() => {});
  // 内置字体通常这时已经加载好了；没好就稍等一下，免得窗口露面后文字再换一次字体。
  await fontReady(200);
  // 等这一帧画完。隐藏的窗口未必还在出帧，所以不能只等它。
  await new Promise<void>((done) => {
    requestAnimationFrame(() => requestAnimationFrame(() => done()));
    setTimeout(done, 120);
  });
  await revealWindow();
}

void installTitlebar().catch(() => {});
createApp((await import("./app/App.vue")).default).use(createPinia()).mount("#app");
void showWhenPainted();
