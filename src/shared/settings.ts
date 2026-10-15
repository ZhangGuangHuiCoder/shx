import { systemLocale, type Locale } from "@/shared/i18n";

export interface Settings {
  /** 匹配任一规则的文件双击时用编辑器打开。 */
  editRules: string[];
  otherDoubleClick: "download" | "edit" | "none";
  showHidden: boolean;
  showFiles: boolean;
  fileLayout: "bottom" | "right";
  showStatusBar: boolean;
  /** 状态栏显示的快捷操作 id；显示顺序固定，与数组顺序无关。 */
  statusActions: string[];
  terminalFontSize: number;
  editorFontSize: number;
  editorWordWrap: boolean;
  editorWhitespace: boolean;
  language: Locale;
  /** 收藏的命令 id，按收藏先后排列。 */
  favoriteCommands: string[];
  theme: "dark" | "light" | "auto";
  font: "builtin" | "system";
  /** 自定义快捷键：动作 id → accelerator，只存改过的项。 */
  shortcuts: Record<string, string>;
}

export const FONT_MIN = 9;
export const FONT_MAX = 28;
export const stepFont = (size: number, delta: number) => Math.min(FONT_MAX, Math.max(FONT_MIN, size + delta));
export const FONT_DEFAULT = 13;

/** 默认用编辑器打开的文本文件规则（写法见 util.ts 的 ruleMatcher）。常为二进制的扩展名（.out、.key、.bak）不收录。 */
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
  // 点文件
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
    editorWordWrap: false,
    editorWhitespace: false,
    favoriteCommands: [],
    language: systemLocale(),
    theme: "dark",
    font: "builtin",
    shortcuts: {},
  };
}

/** localStorage 键名。 */
export const STORAGE = {
  profiles: "shx.profiles",
  secrets: "shx.secrets",
  keys: "shx.keys",
  knownHosts: "shx.knownHosts",
  settings: "shx.settings",
  sidebar: "shx.sidebar",
  fileTree: "shx.fileTree",
  commands: "shx.commands",
  /** 跳过的更新版本，仅本机，不进备份。 */
  skippedUpdate: "shx.skippedUpdate",
} as const;

/** 读取存储值，缺失或解析失败时返回默认值。 */
export function readStored<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
