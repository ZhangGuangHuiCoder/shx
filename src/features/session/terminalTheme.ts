// 终端的配色，跟随应用的明暗主题。
import type { ITheme } from "@xterm/xterm";
import { computed } from "vue";
import { isDark } from "../../shared/theme";

/** 深色：ANSI 的 16 种颜色用 xterm 默认的那一套。 */
const DARK: ITheme = { background: "#16171b", foreground: "#d6d9e0", cursor: "#d6d9e0", selectionBackground: "#3a5a8c" };

/** 浅色：默认的 ANSI 颜色在白底上有几种看不清（黄、白），整套换成按浅色背景调校的。 */
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

export const terminalTheme = computed<ITheme>(() => (isDark.value ? DARK : LIGHT));
