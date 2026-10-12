// 全局的明暗主题。选择保存在设置里；本模块让不使用 store 的入口
// （启动时的首次渲染、独立的对话框 API）不依赖 Pinia 也能跟随主题。
import { darkTheme, type GlobalTheme, type GlobalThemeOverrides } from "naive-ui";
import { computed, ref } from "vue";
import { readStored, STORAGE, type Settings } from "./settings";

/** 用户选的主题：固定浅色、固定深色，或者跟随系统。 */
export type ThemeSetting = Settings["theme"];
/** 实际使用的配色。 */
export type ThemeMode = "dark" | "light";

/** 可选的主题，按菜单里的顺序排列。 */
export const THEME_SETTINGS: ThemeSetting[] = ["light", "dark", "auto"];

export function isThemeSetting(value: unknown): value is ThemeSetting {
  return THEME_SETTINGS.includes(value as ThemeSetting);
}

function initialSetting(): ThemeSetting {
  const saved = readStored<Partial<Settings>>(STORAGE.settings, {}).theme;
  return isThemeSetting(saved) ? saved : "dark";
}

/** 系统当前是否为深色外观；选了“自动”时跟着它走。 */
const system = window.matchMedia("(prefers-color-scheme: dark)");
const systemDark = ref(system.matches);

/** 应用对 Naive UI 的样式调整，应用本身和独立对话框共用。 */
export const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: "#409eff",
    primaryColorHover: "#66b1ff",
    primaryColorPressed: "#3a8ee6",
    primaryColorSuppl: "#409eff",
    fontSize: "13px",
    borderRadius: "7px",
    borderRadiusSmall: "5px",
  },
  Card: { borderRadius: "14px" },
  // 和应用其他地方的滚动条一个样子（styles.css）。
  Scrollbar: { color: "var(--scrollbar)", colorHover: "var(--scrollbar-hover)", width: "6px", height: "6px", borderRadius: "3px" },
  Dialog: { borderRadius: "14px", padding: "0" },
};

export const themeSetting = ref<ThemeSetting>(initialSetting());
export const themeMode = computed<ThemeMode>(() => (themeSetting.value === "auto" ? (systemDark.value ? "dark" : "light") : themeSetting.value));
export const isDark = computed(() => themeMode.value === "dark");
/** 当前模式对应的 Naive UI 主题（浅色是 Naive 的默认主题，所以为 null）。 */
export const naiveTheme = computed<GlobalTheme | null>(() => (isDark.value ? darkTheme : null));

/** 把实际的配色写到 <html data-theme="…">，CSS 以它为选择器。 */
function paint() {
  document.documentElement.dataset.theme = themeMode.value;
}

/** 重新读取系统的外观；选了“自动”的话界面随之改变。 */
function followSystem() {
  systemDark.value = system.matches;
  paint();
}

// 系统在深浅之间切换（比如到了晚上）时立刻跟上。窗口不可见期间发生的切换，
// 通知可能迟迟不来，所以回到窗口时再主动核对一次。
system.addEventListener("change", followSystem);
window.addEventListener("focus", followSystem);
document.addEventListener("visibilitychange", followSystem);

/** 采用用户选的主题。 */
export function applyTheme(setting: ThemeSetting) {
  if (!isThemeSetting(setting)) return;
  themeSetting.value = setting;
  paint();
}
