import { t } from "./i18n";

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${Math.round(bytes)} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unit]}`;
}

export function formatTime(epochSeconds: number | null): string {
  if (!epochSeconds) return "";
  const d = new Date(epochSeconds * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function formatDuration(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return "";
  if (seconds < 60) return t("duration.seconds", { s: Math.ceil(seconds) });
  if (seconds < 3600) return t("duration.minutes", { m: Math.floor(seconds / 60), s: Math.ceil(seconds % 60) });
  return t("duration.hours", { h: Math.floor(seconds / 3600), m: Math.floor((seconds % 3600) / 60) });
}

/** `ls -l` 风格的权限字符串，如 `drwxr-xr-x`，包含 setuid/setgid/sticky 位。 */
export function formatMode(mode: number | null, kind: string): string {
  if (mode === null) return "";
  const type = kind === "dir" ? "d" : kind === "link" ? "l" : "-";
  const special = [
    [0o4000, "s", "S"],
    [0o2000, "s", "S"],
    [0o1000, "t", "T"],
  ] as const;
  let out = type;
  for (let i = 0; i < 3; i++) {
    const bits = (mode >> ((2 - i) * 3)) & 7;
    const exec = bits & 1;
    const [flag, on, off] = special[i];
    out += bits & 4 ? "r" : "-";
    out += bits & 2 ? "w" : "-";
    out += mode & flag ? (exec ? on : off) : exec ? "x" : "-";
  }
  return out;
}

/** 八进制的权限数字，如 `755`；带 setuid/setgid/sticky 位时是四位，如 `4755`。 */
export function formatOctal(mode: number | null): string {
  return mode === null ? "" : (mode & 0o7777).toString(8).padStart(3, "0");
}

export function joinPath(dir: string, name: string): string {
  return dir.endsWith("/") ? dir + name : `${dir}/${name}`;
}

export function parentPath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const i = trimmed.lastIndexOf("/");
  return i <= 0 ? "/" : trimmed.slice(0, i);
}

export function baseName(path: string): string {
  return path.replace(/[/\\]+$/, "").split(/[/\\]/).pop() ?? path;
}

/** 把字符串引成一个 POSIX shell 单词。 */
export function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/**
 * 用户的“在编辑器中打开”规则，编译成判断文件名是否匹配其中某一条的函数。规则可以是
 * 扩展名（`.conf`）、通配符（`*.log`、`nginx*`）或完整文件名（`Dockerfile`）。
 * 文件列表的每一行每次绘制都要判断，先编译好，通配符规则只需转成正则一次。
 */
export function ruleMatcher(rules: string[]): (name: string) => boolean {
  const suffixes: string[] = [];
  const names = new Set<string>();
  const patterns: RegExp[] = [];
  for (const raw of rules) {
    const rule = raw.trim().toLowerCase();
    if (!rule) continue;
    if (rule.includes("*") || rule.includes("?")) {
      const pattern = rule.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
      patterns.push(new RegExp(`^${pattern}$`));
    } else if (rule.startsWith(".")) suffixes.push(rule);
    else names.add(rule);
  }
  return (name) => {
    const lower = name.toLowerCase();
    return names.has(lower) || suffixes.some((suffix) => lower.endsWith(suffix)) || patterns.some((pattern) => pattern.test(lower));
  };
}

const ARCHIVE_EXT = [".zip", ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".gz"];

export function isArchive(name: string): boolean {
  const lower = name.toLowerCase();
  return ARCHIVE_EXT.some((ext) => lower.endsWith(ext));
}
