// 全局明暗主题，不依赖 Pinia，启动渲染和独立对话框也能跟随。
import { computed, ref } from "vue";
import { readStored, STORAGE, type Settings } from "@/shared/settings";

export type ThemeSetting = Settings["theme"];
export type ThemeMode = "dark" | "light";

export const THEME_SETTINGS: ThemeSetting[] = ["light", "dark", "auto"];

export function isThemeSetting(value: unknown): value is ThemeSetting {
  return THEME_SETTINGS.includes(value as ThemeSetting);
}

function initialSetting(): ThemeSetting {
  const saved = readStored<Partial<Settings>>(STORAGE.settings, {}).theme;
  return isThemeSetting(saved) ? saved : "dark";
}

const system = window.matchMedia("(prefers-color-scheme: dark)");
const systemDark = ref(system.matches);

export const themeSetting = ref<ThemeSetting>(initialSetting());
export const themeMode = computed<ThemeMode>(() => (themeSetting.value === "auto" ? (systemDark.value ? "dark" : "light") : themeSetting.value));
export const isDark = computed(() => themeMode.value === "dark");

let settle: number | undefined;

/** 写入 <html data-theme>；切换时暂停过渡（.theme-switching），避免两种配色混杂。 */
function paint() {
  const root = document.documentElement;
  if (root.dataset.theme === themeMode.value) return;
  const first = !root.dataset.theme;
  root.dataset.theme = themeMode.value;
  if (first) return;
  root.classList.add("theme-switching");
  if (settle !== undefined) cancelAnimationFrame(settle);
  settle = requestAnimationFrame(() => (settle = requestAnimationFrame(() => root.classList.remove("theme-switching"))));
}

function followSystem() {
  systemDark.value = system.matches;
  paint();
}

// 窗口不可见时系统通知可能延迟，回到窗口时再核对一次。
system.addEventListener("change", followSystem);
window.addEventListener("focus", followSystem);
document.addEventListener("visibilitychange", followSystem);

export function applyTheme(setting: ThemeSetting) {
  if (!isThemeSetting(setting)) return;
  themeSetting.value = setting;
  paint();
}
