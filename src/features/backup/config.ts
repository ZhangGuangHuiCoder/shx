// 应用保存在 localStorage 里的全部数据的备份与恢复。结果在写盘前由后端加密。
// 都是纯函数，不依赖框架，可以直接用 `node --test` 做单元测试。
import type { Profile, StoredKey } from "../../shared/api.ts";
import { parseCustomCommand, type CustomCommand } from "../commands/commands.ts";

export interface ConfigData {
  profiles: Profile[];
  /** 记住的密码或私钥口令，以会话配置 id 为键。 */
  secrets: Record<string, string>;
  keys: StoredKey[];
  /** `host:port` → 密钥算法 → 指纹。 */
  knownHosts: Record<string, Record<string, string>>;
  settings: Record<string, unknown>;
  /** 用户在命令助手里保存的命令：新建的，以及对默认命令的修改和删除。 */
  commands: CustomCommand[];
}

export interface ExportFile extends ConfigData {
  app: "shx";
  version: 1;
  exportedAt: string;
}

export interface ParsedImport {
  data: ConfigData;
  /** 因格式不正确而被丢弃的条目数。 */
  skipped: number;
}

/** 备份内容被拒绝的原因。由调用方把错误码转成提示文字。 */
export class ImportError extends Error {
  readonly code: "invalidJson" | "notBackup" | "newerVersion";

  constructor(code: ImportError["code"]) {
    super(code);
    this.code = code;
  }
}

export function buildExport(data: ConfigData, now = new Date()): ExportFile {
  return { app: "shx", version: 1, exportedAt: now.toISOString(), ...data };
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const text = (v: unknown, fallback = "") => (typeof v === "string" ? v : fallback);

function parseProfile(v: unknown): Profile | null {
  if (!isRecord(v)) return null;
  const { id, host, port, username, auth } = v;
  if (typeof id !== "string" || !id || typeof host !== "string" || !host.trim()) return null;
  if (typeof port !== "number" || !Number.isInteger(port) || port < 1 || port > 65535) return null;
  if (typeof username !== "string" || !username) return null;
  if (auth !== "password" && auth !== "key" && auth !== "agent") return null;
  const dirs = Array.isArray(v.favoriteDirs) ? v.favoriteDirs.filter((dir): dir is string => typeof dir === "string" && dir.startsWith("/")) : [];
  return {
    id,
    host,
    port,
    username,
    auth,
    name: text(v.name) || host,
    group: text(v.group),
    keyId: typeof v.keyId === "string" && v.keyId ? v.keyId : null,
    saveSecret: v.saveSecret === true,
    note: text(v.note),
    pinned: v.pinned === true,
    ...(v.keepAlive === true ? { keepAlive: true } : {}),
    ...(dirs.length ? { favoriteDirs: [...new Set(dirs)] } : {}),
    ...(typeof v.icon === "string" && v.icon ? { icon: v.icon } : {}),
    ...(typeof v.color === "number" && Number.isFinite(v.color) && v.color >= 0 && v.color < 360
      ? { color: Math.round(v.color) }
      : {}),
  };
}

function parseKey(v: unknown): StoredKey | null {
  if (!isRecord(v)) return null;
  const { id, content } = v;
  if (typeof id !== "string" || !id || typeof content !== "string" || !content.trim()) return null;
  return { id, content, name: text(v.name) || "key", encrypted: v.encrypted === true };
}

/** 保留对象中值为字符串的条目，其余的计入跳过数。 */
function stringMap(v: unknown, skip: () => void): Record<string, string> {
  const out: Record<string, string> = {};
  if (!isRecord(v)) return out;
  for (const [key, value] of Object.entries(v)) {
    if (typeof value === "string") out[key] = value;
    else skip();
  }
  return out;
}

/** 校验解密后的备份内容。出错时抛出 `ImportError`。 */
export function parseImport(json: string): ParsedImport {
  let file: unknown;
  try {
    file = JSON.parse(json);
  } catch {
    throw new ImportError("invalidJson");
  }
  if (!isRecord(file) || file.app !== "shx") throw new ImportError("notBackup");
  if (typeof file.version !== "number" || file.version > 1) throw new ImportError("newerVersion");

  let skipped = 0;
  const skip = () => void skipped++;
  const list = <T>(v: unknown, parse: (item: unknown) => T | null): T[] =>
    (Array.isArray(v) ? v : []).flatMap((item) => {
      const parsed = parse(item);
      if (!parsed) skip();
      return parsed ? [parsed] : [];
    });

  const knownHosts: ConfigData["knownHosts"] = {};
  for (const [host, keys] of Object.entries(isRecord(file.knownHosts) ? file.knownHosts : {})) {
    if (isRecord(keys)) knownHosts[host] = stringMap(keys, skip);
    else skip();
  }
  const data: ConfigData = {
    profiles: list(file.profiles, parseProfile),
    secrets: stringMap(file.secrets, skip),
    keys: list(file.keys, parseKey),
    knownHosts,
    settings: isRecord(file.settings) ? file.settings : {},
    // 命令助手出现之前的备份里没有这一项。
    commands: list(file.commands, parseCustomCommand),
  };
  return { data, skipped };
}

/**
 * 把备份合并进已有的数据：id 相同的条目以备份为准，其余保留。
 * 因此导入不会删除任何内容。
 */
export function applyImport(current: ConfigData, incoming: ConfigData): ConfigData {
  const byId = <T extends { id: string }>(base: T[], extra: T[]) => [
    ...new Map([...base, ...extra].map((item) => [item.id, item])).values(),
  ];
  const knownHosts = { ...current.knownHosts };
  for (const [host, hostKeys] of Object.entries(incoming.knownHosts)) {
    knownHosts[host] = { ...knownHosts[host], ...hostKeys };
  }
  const profiles = byId(current.profiles, incoming.profiles);
  // 没有对应会话配置的密码没有意义。
  const ids = new Set(profiles.map((p) => p.id));
  const secrets = { ...current.secrets, ...incoming.secrets };
  return {
    profiles,
    keys: byId(current.keys, incoming.keys),
    secrets: Object.fromEntries(Object.entries(secrets).filter(([id]) => ids.has(id))),
    knownHosts,
    settings: { ...current.settings, ...incoming.settings },
    commands: byId(current.commands, incoming.commands),
  };
}

/** 只从 `incoming` 中取出 `defaults` 里存在且类型相同的设置项。 */
export function pickSettings<T extends object>(defaults: T, incoming: Record<string, unknown>): T {
  const out = { ...defaults } as Record<string, unknown>;
  for (const [key, fallback] of Object.entries(defaults)) {
    const value = incoming[key];
    const sameShape = Array.isArray(fallback)
      ? Array.isArray(value) && value.every((item) => typeof item === "string")
      : typeof value === typeof fallback && !Array.isArray(value);
    if (sameShape) out[key] = value;
  }
  return out as T;
}
