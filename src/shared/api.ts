import { Channel, invoke } from "@tauri-apps/api/core";

export type AuthKind = "password" | "key" | "agent";

/** 保存的连接配置。记住的密码或口令另外存放。 */
export interface Profile {
  id: string;
  name: string;
  group: string;
  host: string;
  port: number;
  username: string;
  auth: AuthKind;
  /** 本地存储中私钥的 id。 */
  keyId: string | null;
  saveSecret: boolean;
  note: string;
  /** 置顶在首页顶部。 */
  pinned?: boolean;
  /** 内置图标名（lucide 的 kebab-case 写法）；未设置时显示名称的首字符。 */
  icon?: string;
  /** 头像色相（0-359）；未设置时由 id 推算出一个固定的色相。 */
  color?: number;
  /** 终端运行在服务器的 tmux 里，连接断开后 shell 仍然存活。 */
  keepAlive?: boolean;
  /** 文件管理器里收藏的目录（服务器上的绝对路径），按收藏的先后排列。 */
  favoriteDirs?: string[];
  /** 把这台主机当作隧道出口时的设置；没有设置过时用默认值。 */
  tunnel?: TunnelConfig;
}

/** 隧道（TUN 模式）的设置：本机的哪些流量经这台主机出去。隧道只管 IPv4。 */
export interface TunnelConfig {
  /** `all`：全部流量进隧道；`include`：只有 `cidrs` 里的网段进隧道。 */
  route: "all" | "include";
  /** 分流时进隧道的网段。 */
  cidrs: string[];
  /** 不进隧道的网段。 */
  bypass: string[];
}

/** 后端报告的隧道状态。 */
export interface TunnelStatus {
  /** `reconnecting`：承载隧道的 SSH 连接断了，流量仍被引进隧道，只是哪儿也去不了。 */
  state: "running" | "reconnecting" | "stopped";
  /** 虚拟网卡的名字。 */
  device: string;
  /** SSH 服务器的地址；重连时必须连回同一个地址，只有它绕开了隧道。 */
  server: string;
  warning: string | null;
  /** 隧道意外中断的原因。 */
  error: string | null;
  connections: number;
  /** 应用发出去和收到的字节数。 */
  up: number;
  down: number;
}

/** 服务器上留着的一个保持会话。 */
export interface KeptSession {
  name: string;
  /** 创建时间和最后一次有输出的时间（Unix 秒）。 */
  created: number;
  activity: number;
  /** 此刻有终端接在上面。 */
  attached: boolean;
  /** 里面正在运行的程序，空闲时是 shell 本身。 */
  command: string;
  /** 它所在的目录。 */
  path: string;
}

/** 服务器是否已为保持会话的终端做好准备。 */
export interface KeepAliveState {
  /** tmux 已安装且会话存在。 */
  ready: boolean;
  /** 会话是刚创建的：是新 shell，而不是保持下来的旧 shell。 */
  fresh: boolean;
  /** 没有 tmux 时的安装命令；仅在识别出包管理器时提供。 */
  install: string | null;
}

/** 保存在本地存储里的私钥：私钥文本，以及展示所需的信息。 */
export interface StoredKey {
  id: string;
  name: string;
  content: string;
  /** 使用该私钥是否需要口令。 */
  encrypted: boolean;
}

export type ConnectAuth =
  | { type: "password"; password: string }
  | { type: "key"; content: string; passphrase: string | null }
  | { type: "agent" };

/** 后端建立连接所需的全部信息；后端自己不保存任何东西。 */
export interface ConnectRequest {
  host: string;
  port: number;
  username: string;
  auth: ConnectAuth;
  /** 已信任的该服务器主机密钥：密钥算法 → SHA-256 指纹。 */
  knownHostKeys: Record<string, string>;
}

export interface FileEntry {
  name: string;
  path: string;
  kind: "file" | "dir" | "link" | "other";
  /** 目录，或指向目录的符号链接。 */
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

/** 内置编辑器打开的文件。 */
export interface TextFile {
  content: string;
  /** 当前用户能否把修改写回去；不能时以只读方式打开。 */
  writable: boolean;
}

export type Direction = "upload" | "download";
export type Conflict = "overwrite" | "skip" | "rename" | "resume";
/** `packing`：打包下载，压缩包还在服务器上生成。 */
export type TransferState = "queued" | "packing" | "running" | "paused" | "done" | "failed" | "cancelled";

export interface Transfer {
  id: string;
  sessionId: string;
  direction: Direction;
  name: string;
  source: string;
  destDir: string;
  /** 下载到了本机的哪里（文件或文件夹）；上传没有。 */
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

/** 所有命令失败时的错误结构；界面根据 `kind` 判断能否补救。 */
export interface AppError {
  kind: "error" | "hostKeyUnknown" | "hostKeyChanged" | "passphraseRequired" | "authFailed" | "noSession" | "cancelled";
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
  /** 把顶层菜单截断到前 `keep` 项，去掉 macOS 追加的部分。 */
  menuTrim: (title: string, keep: number) => invoke<void>("menu_trim", { title, keep }),
  /** 告诉后端用哪种语言给出错误信息。 */
  setLocale: (locale: string) => invoke<void>("set_locale", { locale }),
  /** 建立连接并返回会话 id。 */
  connect: (request: ConnectRequest) => invoke<string>("ssh_connect", { request }),
  /** 校验私钥并返回其文本：`path` 是选择的文件，`content` 是粘贴的文本。 */
  keyImport: (source: { path?: string; content?: string; name?: string }) =>
    invoke<Omit<StoredKey, "id">>("key_import", source),
  disconnect: (id: string) => invoke<void>("ssh_disconnect", { id }),
  /** 启动 shell；指定 `keepAlive` 时改为接入同名的 tmux 会话。 */
  termOpen: (id: string, cols: number, rows: number, onData: (bytes: Uint8Array) => void, keepAlive: string | null = null) => {
    const channel = new Channel<ArrayBuffer | number[]>();
    channel.onmessage = (data) => onData(new Uint8Array(data));
    return invoke<void>("term_open", { id, cols, rows, onData: channel, keepAlive });
  },
  /** 检查服务器是否有 tmux，并确保保持会话 `name` 存在。 */
  keepAlivePrepare: (id: string, name: string, cols: number, rows: number) =>
    invoke<KeepAliveState>("keepalive_prepare", { id, name, cols, rows }),
  /** 服务器上留着的保持会话，所有主机配置的都在内。 */
  keepAliveList: (id: string) => invoke<KeptSession[]>("keepalive_list", { id }),
  /** 结束服务器上的保持会话 `name` 及其中运行的一切。 */
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
  /**
   * 把 `dir` 下的条目打包成同目录的 `archive`，格式由扩展名决定。
   * `level` 为 0–9，null 表示默认级别；`removeSource` 表示打包后删除源条目。
   */
  compress: (id: string, dir: string, names: string[], archive: string, level: number | null = null, removeSource = false) =>
    invoke<void>("sftp_compress", { id, dir, names, archive, level, removeSource }),
  extract: (id: string, path: string, destDir: string) => invoke<void>("sftp_extract", { id, path, destDir }),
  search: (id: string, dir: string, pattern: string) => invoke<FileEntry[]>("sftp_search", { id, dir, pattern }),

  transferStart: (sessionId: string, direction: Direction, sources: string[], destDir: string, conflict: Conflict) =>
    invoke<Transfer[]>("transfer_start", { sessionId, direction, sources, destDir, conflict }),
  /**
   * 把远程文件夹 `dir` 下的条目打成名为 `archive` 的压缩包后下载，先在服务器上打包。
   * 服务器上的压缩包由传输任务自己管理：保留以备重试，下载完成或放弃后删除。
   */
  transferPack: (sessionId: string, dir: string, names: string[], archive: string, level: number | null, destDir: string, conflict: Conflict) =>
    invoke<Transfer>("transfer_pack", { sessionId, dir, names, archive, level, destDir, conflict }),
  /** 把断开连接上的传输任务移交给接替它的连接，以便重试。 */
  transferRebind: (from: string, to: string) => invoke<void>("transfer_rebind", { from, to }),
  /** 在连接彻底关闭前调用：清理其传输任务留在服务器上的东西。 */
  transferRelease: (sessionId: string) => invoke<void>("transfer_release", { sessionId }),
  transferControl: (id: string, action: "pause" | "resume" | "cancel") =>
    invoke<void>("transfer_control", { id, action }),
  transferRetry: (id: string) => invoke<void>("transfer_retry", { id }),
  transferList: () => invoke<Transfer[]>("transfer_list"),
  transferClear: () => invoke<void>("transfer_clear"),
  localExisting: (dir: string, names: string[]) => invoke<string[]>("local_existing", { dir, names }),
  defaultDownloadDir: () => invoke<string | null>("default_download_dir"),
  /** 写出导出文件，仅文件所有者可读。 */
  configExport: (path: string, content: string) => invoke<void>("config_export", { path, content }),
  configImport: (path: string) => invoke<string>("config_import", { path }),

  /** 这个系统上能不能用隧道。 */
  tunnelSupported: () => invoke<boolean>("tunnel_supported"),
  /** 隧道助手是否已装进系统（装了之后开隧道不用再输密码）；没有这回事的系统上是 null。 */
  tunnelHelper: () => invoke<boolean | null>("tunnel_helper"),
  /** 把隧道助手装进系统（请用户授权一次，不同意时以 `cancelled` 失败），或者从系统里删掉。 */
  tunnelHelperSet: (installed: boolean) => invoke<void>("tunnel_helper_set", { installed }),
  /** 开启隧道，流量经会话 `sessionId` 的连接出去。会请用户授予管理员权限，不同意时以 `cancelled` 失败。 */
  tunnelStart: (sessionId: string, config: TunnelConfig) => invoke<TunnelStatus>("tunnel_start", { sessionId, config }),
  /** 承载隧道的连接断开后，把隧道接到重连出来的新会话上。 */
  tunnelAttach: (sessionId: string) => invoke<TunnelStatus>("tunnel_attach", { sessionId }),
  /** 关闭隧道；返回时虚拟网卡和路由已经撤掉。 */
  tunnelStop: () => invoke<void>("tunnel_stop"),
  tunnelStatus: () => invoke<TunnelStatus | null>("tunnel_status"),
};
