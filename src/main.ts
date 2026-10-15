import { getCurrentWindow } from "@tauri-apps/api/window";
import { createPinia } from "pinia";
import { createApp, watch } from "vue";
import "@/assets/base.css";
import { revealWindow } from "@/app/window";
import { fontReady } from "@/shared/fonts";
import { applyTheme, themeMode, themeSetting } from "@/shared/theme";
import { installTitlebar, VIBRANCY } from "@/shared/titlebar";
import { hardenWebview } from "@/shared/webview";
hardenWebview();
// 首次渲染前应用配色，避免闪烁。
applyTheme(themeSetting.value);

/** 窗口初始隐藏，首帧绘制后再显示，避免白屏；前端未启动时由后端兜底（lib.rs）。 */
async function showWhenPainted() {
  const win = getCurrentWindow();
  // 开发时重载页面，窗口已可见则跳过。
  if (await win.isVisible().catch(() => false)) return;
  // 窗口底色与 index.html 一致，覆盖首帧前的空隙；毛玻璃窗口不需要。
  if (!VIBRANCY) {
    const background = document.documentElement.dataset.theme === "light" ? "#f4f4f5" : "#09090b";
    await win.setBackgroundColor(background).catch(() => {});
  }
  // 稍等字体加载，避免显示后字体闪变。
  await fontReady(200);
  // 等待首帧；隐藏窗口可能不出帧，加超时兜底。
  await new Promise<void>((done) => {
    requestAnimationFrame(() => requestAnimationFrame(() => done()));
    setTimeout(done, 120);
  });
  await revealWindow();
}

// 毛玻璃明暗跟随窗口外观，需与应用主题同步。
if (VIBRANCY) watch(themeMode, (mode) => void getCurrentWindow().setTheme(mode).catch(() => {}), { immediate: true });

void installTitlebar().catch(() => {});
createApp((await import("@/app/App.vue")).default).use(createPinia()).mount("#app");
void showWhenPainted();
