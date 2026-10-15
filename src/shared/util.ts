import { t } from "@/shared/i18n";

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

/** 类似 `ls -l` 的短时间：今年显示“月-日 时:分”，往年显示“年-月-日”。 */
export function formatShortTime(epochSeconds: number | null, now = new Date()): string {
  if (!epochSeconds) return "";
  const d = new Date(epochSeconds * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  const day = `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return d.getFullYear() === now.getFullYear() ? `${day} ${pad(d.getHours())}:${pad(d.getMinutes())}` : `${d.getFullYear()}-${day}`;
}

export function formatDuration(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return "";
  if (seconds < 60) return t("duration.seconds", { s: Math.ceil(seconds) });
  if (seconds < 3600) return t("duration.minutes", { m: Math.floor(seconds / 60), s: Math.ceil(seconds % 60) });
  return t("duration.hours", { h: Math.floor(seconds / 3600), m: Math.floor((seconds % 3600) / 60) });
}

/** `ls -l` 风格权限串，含 setuid/setgid/sticky 位。 */
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

/** 八进制权限，如 755；含特殊位时为四位。 */
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

/** POSIX shell 单引号转义。 */
export function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/**
 * 把“用编辑器打开”规则编译成匹配函数（不区分大小写）。
 * 规则：以点开头按后缀匹配，含通配符转正则，其余按完整文件名匹配。
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

export function statusDotColor(status: string): string {
  return status === "connected" ? "bg-emerald-500" : status === "closed" ? "bg-red-500" : "bg-amber-500";
}
