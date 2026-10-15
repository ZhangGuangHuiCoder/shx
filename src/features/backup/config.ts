// 备份的构建与解析（纯函数）；写盘前由后端加密。
import type { Profile, StoredKey } from "@/shared/api.ts";
import { parseCustomCommand, type CustomCommand } from "@/features/commands/commands.ts";
import { parseTunnelConfig } from "@/features/tunnel/tunnel.ts";

export interface ConfigData {
  profiles: Profile[];
  /** 键为配置 id。 */
  secrets: Record<string, string>;
  keys: StoredKey[];
  /** `host:port` → 密钥算法 → 指纹。 */
  knownHosts: Record<string, Record<string, string>>;
  settings: Record<string, unknown>;
  commands: CustomCommand[];
}

export interface ExportFile extends ConfigData {
  app: "shx";
  version: 1;
  exportedAt: string;
}

export interface ParsedImport {
  data: ConfigData;
  /** 因格式错误丢弃的条目数。 */
  skipped: number;
}

/** 错误码由调用方转为提示文字。 */
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
  const tunnel = parseTunnelConfig(v.tunnel);
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
    ...(tunnel ? { tunnel } : {}),
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

/** 只保留字符串值，其余计入跳过数。 */
function stringMap(v: unknown, skip: () => void): Record<string, string> {
  const out: Record<string, string> = {};
  if (!isRecord(v)) return out;
  for (const [key, value] of Object.entries(v)) {
    if (typeof value === "string") out[key] = value;
    else skip();
  }
  return out;
}

/** 校验解密后的备份，出错抛出 ImportError。 */
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
    // 旧版备份可能没有此项。
    commands: list(file.commands, parseCustomCommand),
  };
  return { data, skipped };
}

/** 合并备份：同 id 以备份为准，不删除已有内容。 */
export function applyImport(current: ConfigData, incoming: ConfigData): ConfigData {
  const byId = <T extends { id: string }>(base: T[], extra: T[]) => [
    ...new Map([...base, ...extra].map((item) => [item.id, item])).values(),
  ];
  const knownHosts = { ...current.knownHosts };
  for (const [host, hostKeys] of Object.entries(incoming.knownHosts)) {
    knownHosts[host] = { ...knownHosts[host], ...hostKeys };
  }
  const profiles = byId(current.profiles, incoming.profiles);
  // 丢弃无对应配置的密码。
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

/** 只取 defaults 中存在且类型一致的设置项。 */
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
