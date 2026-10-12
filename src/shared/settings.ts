// 应用的设置，以及保存它们的 localStorage 键名。不依赖框架。
import { systemLocale, type Locale } from "./i18n";

export interface Settings {
  /** 匹配其中任一规则的文件，双击时在编辑器中打开。 */
  editRules: string[];
  /** 双击不匹配任何规则的文件时的行为。 */
  otherDoubleClick: "download" | "edit" | "none";
  showHidden: boolean;
  /** 会话里是否显示文件管理器；隐藏后终端占满整个会话区域。 */
  showFiles: boolean;
  /** 文件管理器相对于终端的位置。 */
  fileLayout: "bottom" | "right";
  /** 是否显示窗口底部的状态栏（连接状态、传输）。 */
  showStatusBar: boolean;
  /** 状态栏上显示的快捷操作（各操作的 id）；显示的顺序是固定的，与这里的顺序无关。 */
  statusActions: string[];
  terminalFontSize: number;
  editorFontSize: number;
  language: Locale;
  /** 命令助手里收藏的命令（各命令的 id），按收藏的先后排列。 */
  favoriteCommands: string[];
  /** 界面配色；`auto` 跟随系统的深浅外观。 */
  theme: "dark" | "light" | "auto";
}

export const FONT_MIN = 9;
export const FONT_MAX = 28;
export const FONT_DEFAULT = 13;

export const DEFAULT_EDIT_RULES = [
  ".txt", ".md", ".log", ".conf", ".cfg", ".ini", ".env", ".properties", ".toml", ".yaml", ".yml",
  ".json", ".xml", ".csv", ".sh", ".bash", ".zsh", ".py", ".js", ".ts", ".vue", ".html", ".css",
  ".go", ".rs", ".java", ".c", ".h", ".cpp", ".php", ".rb", ".lua", ".sql", ".service",
  "Dockerfile", "Makefile", ".gitignore", ".bashrc", ".zshrc", ".profile", ".vimrc",
  "hosts", "fstab", "crontab", "authorized_keys", "nginx*",
];

export function defaultSettings(): Settings {
  return {
    editRules: [...DEFAULT_EDIT_RULES],
    otherDoubleClick: "download",
    showHidden: false,
    showFiles: true,
    fileLayout: "bottom",
    showStatusBar: true,
    statusActions: ["files", "assistant"],
    terminalFontSize: FONT_DEFAULT,
    editorFontSize: FONT_DEFAULT,
    favoriteCommands: [],
    language: systemLocale(),
    theme: "dark",
  };
}

/** 应用所存数据在 localStorage 中的键名。 */
export const STORAGE = {
  profiles: "shx.profiles",
  secrets: "shx.secrets",
  keys: "shx.keys",
  knownHosts: "shx.knownHosts",
  settings: "shx.settings",
  sidebar: "shx.sidebar",
  fileTree: "shx.fileTree",
  commands: "shx.commands",
} as const;

/** 读取保存的值；不存在或无法解析时返回默认值。 */
export function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
