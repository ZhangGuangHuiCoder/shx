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
  /** 界面、终端和编辑器的字体：应用内置的等宽字体 JetBrains Mono，或系统自带的字体。 */
  font: "builtin" | "system";
}

export const FONT_MIN = 9;
export const FONT_MAX = 28;
export const FONT_DEFAULT = 13;

/**
 * 默认在编辑器里打开的文件：服务器上常见的文本文件。规则的写法见 `ruleMatcher`（util.ts）：
 * 以点开头的按结尾匹配（扩展名、点文件），带通配符的按通配符，其余的是完整文件名；都不区分大小写。
 * 只收文本：同一个扩展名常常是二进制文件的（`.out`、`.key`、`.bak`）不收。
 */
export const DEFAULT_EDIT_RULES = [
  // 文本和文档
  ".txt", ".md", ".markdown", ".rst", ".adoc", ".tex", ".log", ".err", ".csv", ".tsv", ".diff", ".patch", "nohup.out",
  "README", "LICENSE", "LICENCE", "COPYING", "NOTICE", "AUTHORS", "CHANGELOG", "CHANGES", "NEWS", "TODO", "VERSION", "INSTALL",
  // 配置和数据
  ".conf", ".cfg", ".cnf", ".ini", ".properties", ".toml", ".yaml", ".yml", ".json", ".jsonc", ".json5", ".ipynb",
  ".xml", ".xsl", ".xsd", ".svg", ".plist", ".config", ".lock", ".env", ".env*", ".example", ".sample", ".template", ".default", ".dist",
  ".tf", ".tfvars", ".hcl", ".nomad", ".gradle", ".cmake", ".mk", ".ovpn", ".pem", ".crt", ".csr", ".pub",
  // systemd 和包管理器
  ".service", ".socket", ".timer", ".target", ".mount", ".path", ".slice", ".network", ".netdev", ".desktop",
  ".repo", ".list", ".sources", ".rules",
  // 脚本
  ".sh", ".bash", ".zsh", ".fish", ".ksh", ".csh", ".ps1", ".bat", ".cmd", ".awk", ".sed",
  // 源代码
  ".py", ".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".vue", ".svelte", ".html", ".htm", ".css", ".scss", ".sass", ".less",
  ".go", ".rs", ".java", ".kt", ".scala", ".groovy", ".c", ".h", ".cc", ".cpp", ".hpp", ".cs", ".swift", ".m", ".dart", ".zig",
  ".php", ".rb", ".pl", ".pm", ".lua", ".tcl", ".r", ".jl", ".ex", ".exs", ".erl", ".hs", ".clj", ".lisp", ".el", ".vim", ".nix",
  ".sql", ".graphql", ".proto", ".asm", ".s",
  // 模板
  ".tpl", ".tmpl", ".j2", ".jinja", ".jinja2", ".erb", ".ejs", ".hbs", ".mustache", ".twig",
  // 有固定名字的工程文件
  "Dockerfile*", "Containerfile", "Makefile*", "GNUmakefile", "Caddyfile", "Vagrantfile", "Jenkinsfile", "Procfile", "Justfile",
  "Gemfile", "Rakefile", "Brewfile", "Pipfile",
  // 点文件：.*rc 和 .*ignore 各是一大类（.bashrc、.vimrc、.npmrc……；.gitignore、.dockerignore……）
  ".*rc", ".*ignore", ".rc", "bashrc", "bash.bashrc", "zshrc", "vimrc", "inputrc", "nanorc", "wgetrc", "screenrc",
  ".profile", ".bash_profile", ".bash_aliases", ".bash_logout", ".zshenv", ".zprofile",
  ".gitconfig", ".gitattributes", ".gitmodules", ".editorconfig", ".htaccess", ".htpasswd", ".pgpass", ".flake8",
  // 系统和服务的配置文件
  "hosts", "hosts.allow", "hosts.deny", "hostname", "fstab", "crontab", "anacrontab", "passwd", "group", "sudoers", "exports",
  "environment", "profile", "issue", "motd", "timezone", "shells", "services", "aliases", "modules", "interfaces", "*-release",
  "authorized_keys", "known_hosts", "sshd_config", "ssh_config", "config", "nginx*",
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
    font: "builtin",
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
