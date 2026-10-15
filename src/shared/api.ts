import { Channel, invoke } from "@tauri-apps/api/core";

export type AuthKind = "password" | "key" | "agent";

/** 连接配置；密码和口令另行存放。 */
export interface Profile {
  id: string;
  name: string;
  group: string;
  host: string;
  port: number;
  username: string;
  auth: AuthKind;
  keyId: string | null;
  saveSecret: boolean;
  note: string;
  pinned?: boolean;
  /** lucide 图标名（kebab-case）；未设置时显示名称首字符。 */
  icon?: string;
  /** 头像色相 0-359；未设置时由 id 推算。 */
  color?: number;
  /** 终端运行在服务器 tmux 中，断线后 shell 仍存活。 */
  keepAlive?: boolean;
  /** 收藏的远程目录，按收藏先后排列。 */
  favoriteDirs?: string[];
  /** 作为隧道出口时的设置。 */
  tunnel?: TunnelConfig;
}

export type TunnelVia = "tunnel" | "direct";

/** 分流规则：发往 net（IPv4 CIDR）的流量走向。 */
export interface TunnelRule {
  net: string;
  via: TunnelVia;
  disabled?: boolean;
}

/** 隧道（TUN，仅 IPv4）设置。地址由最具体（前缀最长）的规则决定，与顺序无关。 */
export interface TunnelConfig {
  /** 未匹配任何规则时的走向。 */
  default: TunnelVia;
  rules: TunnelRule[];
  /** 本机网段的走向，网段自动探测；null 表示不特殊处理。 */
  lan: TunnelVia | null;
}

/** 系统隧道助手状态；outdated 表示需要更新，下次开启时要再授权一次。 */
export type TunnelHelper = "missing" | "outdated" | "installed";

export interface TunnelStatus {
  /** reconnecting：SSH 连接已断，流量仍被导入隧道但无法送出。 */
  state: "running" | "reconnecting" | "stopped";
  /** 虚拟网卡名。 */
  device: string;
  /** SSH 服务器地址；只有它绕开隧道，重连须连同一地址。 */
  server: string;
  warning: string | null;
  error: string | null;
  connections: number;
  /** 上行/下行字节数。 */
  up: number;
  down: number;
}

/** 服务器上的保活会话。 */
export interface KeptSession {
  name: string;
  /** Unix 秒。 */
  created: number;
  activity: number;
  attached: boolean;
  /** 正在运行的程序，空闲时为 shell。 */
  command: string;
  path: string;
}

export interface KeepAliveState {
  ready: boolean;
  /** 会话为新建（新 shell）。 */
  fresh: boolean;
  /** 缺少 tmux 时的安装命令，识别不出包管理器时为 null。 */
  install: string | null;
}

/** 本地保存的私钥。 */
export interface StoredKey {
  id: string;
  name: string;
  content: string;
  encrypted: boolean;
}

export type ConnectAuth =
  | { type: "password"; password: string }
  | { type: "key"; content: string; passphrase: string | null }
  | { type: "agent" };

/** 连接所需的全部信息，后端不保存任何东西。 */
export interface ConnectRequest {
  host: string;
  port: number;
  username: string;
  auth: ConnectAuth;
  /** 已信任的主机密钥：算法 → SHA-256 指纹。 */
  knownHostKeys: Record<string, string>;
}

export interface FileEntry {
  name: string;
  path: string;
  kind: "file" | "dir" | "link" | "other";
  /** 目录或指向目录的链接。 */
  isDir: boolean;
  size: number;
  mtime: number | null;
  mode: number | null;
  uid: number | null;
  gid: number | null;
  owner: string | null;
  group: string | null;
  linkTarget: string | null;
}

export interface TextFile {
  content: string;
  writable: boolean;
}

export type Direction = "upload" | "download";
export type Conflict = "overwrite" | "skip" | "rename" | "resume";
/** packing：打包下载时服务器端正在生成压缩包。 */
export type TransferState = "queued" | "packing" | "running" | "paused" | "done" | "failed" | "cancelled";

export interface Transfer {
  id: string;
  sessionId: string;
  direction: Direction;
  name: string;
  source: string;
  destDir: string;
  /** 下载的本地路径；上传为 null。 */
  local: string | null;
  state: TransferState;
  total: number;
  transferred: number;
  filesTotal: number;
  filesDone: number;
  speed: number;
  current: string;
  error: string | null;
}

/** 后端命令的错误结构，界面据 kind 决定如何补救。 */
export interface AppError {
  kind: "error" | "hostKeyUnknown" | "hostKeyChanged" | "passphraseRequired" | "authFailed" | "agentEmpty" | "noSession" | "cancelled";
  message: string;
  fingerprint?: string;
  algorithm?: string;
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as AppError).message);
  return String(e);
}

export const api = {
  /** 截断顶层菜单，去掉 macOS 追加的项。 */
  menuTrim: (title: string, keep: number) => invoke<void>("menu_trim", { title, keep }),
  /** 由系统自动决定菜单项可用性（macOS）。 */
  menuAutoenable: (title: string) => invoke<void>("menu_autoenable", { title }),
  setLocale: (locale: string) => invoke<void>("set_locale", { locale }),
  connect: (request: ConnectRequest) => invoke<string>("ssh_connect", { request }),
  /** 校验私钥并返回文本，来源为文件路径或粘贴内容。 */
  keyImport: (source: { path?: string; content?: string; name?: string }) =>
    invoke<Omit<StoredKey, "id">>("key_import", source),
  disconnect: (id: string) => invoke<void>("ssh_disconnect", { id }),
  /** 调用 ssh-add 加载默认私钥（本程序不读私钥文件），返回 agent 中的密钥数。 */
  agentAddKeys: () => invoke<number>("agent_add_keys"),
  /** 启动 shell；指定 keepAlive 时接入同名 tmux 会话。 */
  termOpen: (id: string, cols: number, rows: number, onData: (bytes: Uint8Array) => void, keepAlive: string | null = null) => {
    const channel = new Channel<ArrayBuffer | number[]>();
    channel.onmessage = (data) => onData(new Uint8Array(data));
    return invoke<void>("term_open", { id, cols, rows, onData: channel, keepAlive });
  },
  /** 检查 tmux 并确保保活会话存在。 */
  keepAlivePrepare: (id: string, name: string, cols: number, rows: number) =>
    invoke<KeepAliveState>("keepalive_prepare", { id, name, cols, rows }),
  /** 服务器上所有保活会话。 */
  keepAliveList: (id: string) => invoke<KeptSession[]>("keepalive_list", { id }),
  /** 结束保活会话及其中的进程。 */
  keepAliveEnd: (id: string, name: string) => invoke<void>("keepalive_end", { id, name }),
  termWrite: (id: string, data: string) => invoke<void>("term_write", { id, data }),
  termResize: (id: string, cols: number, rows: number) => invoke<void>("term_resize", { id, cols, rows }),

  list: (id: string, path: string) => invoke<{ path: string; entries: FileEntry[] }>("sftp_list", { id, path }),
  stat: (id: string, path: string) => invoke<FileEntry>("sftp_stat", { id, path }),
  mkdir: (id: string, path: string) => invoke<void>("sftp_mkdir", { id, path }),
  createFile: (id: string, path: string) => invoke<void>("sftp_create_file", { id, path }),
  rename: (id: string, from: string, to: string) => invoke<void>("sftp_rename", { id, from, to }),
  remove: (id: string, paths: string[]) => invoke<void>("sftp_remove", { id, paths }),
  move: (id: string, paths: string[], destDir: string) => invoke<void>("sftp_move", { id, paths, destDir }),
  copy: (id: string, paths: string[], destDir: string) => invoke<void>("sftp_copy", { id, paths, destDir }),
  chmod: (id: string, paths: string[], mode: number, recursive: boolean) =>
    invoke<void>("sftp_chmod", { id, paths, mode, recursive }),
  chown: (id: string, paths: string[], owner: string, group: string, recursive: boolean) =>
    invoke<void>("sftp_chown", { id, paths, owner, group, recursive }),
  readText: (id: string, path: string) => invoke<TextFile>("sftp_read_text", { id, path }),
  writeText: (id: string, path: string, content: string) => invoke<void>("sftp_write_text", { id, path, content }),
  du: (id: string, path: string) => invoke<number>("sftp_du", { id, path }),
  /** 打包为同目录下的 archive，格式由扩展名决定；level 0–9，null 为默认。 */
  compress: (id: string, dir: string, names: string[], archive: string, level: number | null = null, removeSource = false) =>
    invoke<void>("sftp_compress", { id, dir, names, archive, level, removeSource }),
  extract: (id: string, path: string, destDir: string) => invoke<void>("sftp_extract", { id, path, destDir }),
  search: (id: string, dir: string, pattern: string) => invoke<FileEntry[]>("sftp_search", { id, dir, pattern }),

  transferStart: (sessionId: string, direction: Direction, sources: string[], destDir: string, conflict: Conflict) =>
    invoke<Transfer[]>("transfer_start", { sessionId, direction, sources, destDir, conflict }),
  /** 服务器端打包后下载；压缩包保留以备重试，完成或放弃后删除。 */
  transferPack: (sessionId: string, dir: string, names: string[], archive: string, level: number | null, destDir: string, conflict: Conflict) =>
    invoke<Transfer>("transfer_pack", { sessionId, dir, names, archive, level, destDir, conflict }),
  /** 把断开连接的传输任务移交给新连接。 */
  transferRebind: (from: string, to: string) => invoke<void>("transfer_rebind", { from, to }),
  /** 连接关闭前清理传输任务在服务器上的残留。 */
  transferRelease: (sessionId: string) => invoke<void>("transfer_release", { sessionId }),
  transferControl: (id: string, action: "pause" | "resume" | "cancel") =>
    invoke<void>("transfer_control", { id, action }),
  transferRetry: (id: string) => invoke<void>("transfer_retry", { id }),
  transferList: () => invoke<Transfer[]>("transfer_list"),
  transferClear: () => invoke<void>("transfer_clear"),
  localExisting: (dir: string, names: string[]) => invoke<string[]>("local_existing", { dir, names }),
  defaultDownloadDir: () => invoke<string | null>("default_download_dir"),
  /** 导出文件仅所有者可读。 */
  configExport: (path: string, content: string) => invoke<void>("config_export", { path, content }),
  configImport: (path: string) => invoke<string>("config_import", { path }),

  tunnelSupported: () => invoke<boolean>("tunnel_supported"),
  /** 不需要助手的系统返回 null。 */
  tunnelHelper: () => invoke<TunnelHelper | null>("tunnel_helper"),
  /** 安装（需授权，拒绝时以 cancelled 失败）或卸载隧道助手。 */
  tunnelHelperSet: (installed: boolean) => invoke<void>("tunnel_helper_set", { installed }),
  /** 本机出口网段。 */
  tunnelLocal: () => invoke<string[]>("tunnel_local"),
  /** 开启隧道，需管理员权限，拒绝时以 cancelled 失败。 */
  tunnelStart: (sessionId: string, config: TunnelConfig) => invoke<TunnelStatus>("tunnel_start", { sessionId, config }),
  /** 断线重连后把隧道接到新会话。 */
  tunnelAttach: (sessionId: string) => invoke<TunnelStatus>("tunnel_attach", { sessionId }),
  /** 返回时网卡和路由已撤除。 */
  tunnelStop: () => invoke<void>("tunnel_stop"),
  tunnelStatus: () => invoke<TunnelStatus | null>("tunnel_status"),
};
