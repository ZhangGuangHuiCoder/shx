import type { ITheme } from "@xterm/xterm";
import { computed } from "vue";
import { isDark } from "@/shared/theme";
import { VIBRANCY } from "@/shared/titlebar";

const DARK: ITheme = { background: "#18181b", foreground: "#d6d9e0", cursor: "#d6d9e0", selectionBackground: "#3a5a8c" };

/** 浅色：默认 ANSI 色在白底上部分看不清，整套重新调校。 */
const LIGHT: ITheme = {
  background: "#ffffff",
  foreground: "#1f2d3d",
  cursor: "#1f2d3d",
  cursorAccent: "#ffffff",
  selectionBackground: "#b9d7fb",
  black: "#24292f",
  red: "#cf222e",
  green: "#116329",
  yellow: "#9a6700",
  blue: "#0969da",
  magenta: "#8250df",
  cyan: "#1b7c83",
  white: "#6e7781",
  brightBlack: "#57606a",
  brightRed: "#a40e26",
  brightGreen: "#1a7f37",
  brightYellow: "#7d4e00",
  brightBlue: "#218bff",
  brightMagenta: "#a475f9",
  brightCyan: "#3192aa",
  brightWhite: "#8c959f",
};

/**
 * 毛玻璃下的半透明底色只画在外层容器上，xterm 本身全透明：
 * xterm 的视口是独立合成层，macOS WebKit 在透明窗口中会把其半透明色画错。
 */
const GLASS = { dark: "#18181b99", light: "#ffffff80" };

export const terminalTheme = computed<ITheme>(() => {
  const theme = isDark.value ? DARK : LIGHT;
  return VIBRANCY ? { ...theme, background: "#00000000" } : theme;
});

export const terminalSurface = computed(() => (VIBRANCY ? (isDark.value ? GLASS.dark : GLASS.light) : terminalTheme.value.background));
