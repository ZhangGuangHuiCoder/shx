import { listen } from "@tauri-apps/api/event";
import { HeartPulse } from "@lucide/vue";
import { defineStore } from "pinia";
import { reactive, ref, watch, type Ref } from "vue";
import { applyImport, pickSettings, type ConfigData } from "../features/backup/config";
import { parseCustomCommand, removeCommand, restoreDefaults, toggleFavorite, type CustomCommand } from "../features/commands/commands";
import { planSession, slotOf } from "../features/session/keepAlive";
import { FILE_TREE_DEFAULT_WIDTH } from "../features/session/layout";
import { chooseSession } from "../features/session/sessionChooser";
import { defaultTunnelConfig, parseTunnelConfig, rate, tunnelRequest } from "../features/tunnel/tunnel";
import { api, errorMessage, type AppError, type ConnectAuth, type Profile, type StoredKey, type Transfer, type TunnelConfig, type TunnelStatus } from "../shared/api";
import { applyFont, isFontSetting } from "../shared/fonts";
import { isLocale, locale, systemLocale, t } from "../shared/i18n";
import { DEFAULT_EDIT_RULES, defaultSettings, readStored, STORAGE, type Settings } from "../shared/settings";
import { applyTheme, isThemeSetting } from "../shared/theme";
import { ask, confirm, message, prompt } from "../shared/ui";

export type { Settings };

/** 侧边栏的显示偏好，与同步的设置分开保存。 */
export interface SidebarPrefs {
  /** 已折叠的分组名；置顶分组用 {@link PINNED_GROUP} 表示。 */
  collapsed: string[];
}

/** 文件管理器目录树的显示方式；和侧边栏一样只属于本机。 */
export interface FileTreePrefs {
  show: boolean;
  /** 宽度（像素）。 */
  width: number;
}

export const FILE_TREE_WIDTH = { min: 120, max: 520, initial: FILE_TREE_DEFAULT_WIDTH };

/** 存放置顶会话的伪分组名，排在真实分组之上。 */
export const PINNED_GROUP = "__pinned__";

export interface Tab {
  key: string;
  profileId: string;
  title: string;
  sessionId: string | null;
  status: "connecting" | "connected" | "closed";
  error: string | null;
  /** 开启会话保持的主机：该标签页的终端所在的 tmux 会话。连上主机之后才确定。 */
  keepAlive: string | null;
  /** 终端此刻是否在该会话里（服务器没有 tmux 时不在）。 */
  attached: boolean;
}

/** 连接主机的结局。用户在某个询问中放弃时是 `cancelled`：用户自己清楚为什么没连上。 */
type Connected =
  | { outcome: "connected"; sessionId: string }
  | { outcome: "cancelled" | "failed"; reason: string; /** 得有用户的回答才能继续，而这次连接不许询问。 */ needsUser?: boolean };

/** 正开着（或正在开、正在关）的隧道。机器上同一时刻只有一条。 */
export interface TunnelRun {
  /** 作为出口的主机。 */
  profileId: string;
  /** 承载隧道的 SSH 会话，它是隧道专用的，不属于任何标签页；重连期间没有。 */
  sessionId: string | null;
  phase: "starting" | "running" | "reconnecting" | "stopping";
  status: TunnelStatus | null;
  /** 每秒的上行、下行流量（字节）。 */
  upRate: number;
  downRate: number;
}

/** 承载隧道的连接断开后，隔多久重连一次（秒）；用完之后一直按最后一个。 */
const TUNNEL_RETRY = [1, 3, 8, 15, 30, 60];

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * 与 localStorage 同步的 ref。应用的全部数据都存在那里：会话配置、记住的密码、
 * 私钥、已信任的主机密钥和设置，均未加密。
 */
function persisted<T>(key: string, fallback: T): Ref<T> {
  const initial = readStored(key, fallback);
  const state = ref(initial) as Ref<T>;
  // 同步写入，修改后立刻关窗口也不会丢。
  watch(
    state,
    (value) => {
      try {
        localStorage.setItem(key, JSON.stringify(value));
      } catch (e) {
        message.error(t("common.saveFailed", { error: errorMessage(e) }));
      }
    },
    { deep: true, flush: "sync" },
  );
  return state;
}

export const useApp = defineStore("app", () => {
  const profiles = persisted<Profile[]>(STORAGE.profiles, []);
  /** 记住的密码或私钥口令，以会话配置 id 为键。 */
  const secrets = persisted<Record<string, string>>(STORAGE.secrets, {});
  const keys = persisted<StoredKey[]>(STORAGE.keys, []);
  /** 已信任的主机密钥：`host:port` → 密钥算法 → 指纹。 */
  const knownHosts = persisted<Record<string, Record<string, string>>>(STORAGE.knownHosts, {});
  const stored = persisted<Partial<Settings>>(STORAGE.settings, {});
  const settings = reactive<Settings>(pickSettings(defaultSettings(), stored.value));
  watch(settings, (value) => (stored.value = { ...value }), { deep: true, flush: "sync" });

  const sidebar = persisted<SidebarPrefs>(STORAGE.sidebar, { collapsed: [] });
  // 丢弃旧版本遗留的 `sort` 偏好（该功能已移除）。
  if ("sort" in (sidebar.value as unknown as Record<string, unknown>) || !Array.isArray(sidebar.value.collapsed)) {
    sidebar.value = { collapsed: Array.isArray(sidebar.value.collapsed) ? sidebar.value.collapsed : [] };
  }

  const fileTree = persisted<FileTreePrefs>(STORAGE.fileTree, { show: true, width: FILE_TREE_WIDTH.initial });
  if (typeof fileTree.value?.show !== "boolean" || !Number.isFinite(fileTree.value.width)) {
    fileTree.value = { show: true, width: FILE_TREE_WIDTH.initial };
  }

  // 语言立即作用于界面，也作用于后端产生的消息。
  watch(
    () => settings.language,
    (language) => {
      if (!isLocale(language)) return void (settings.language = systemLocale());
      locale.value = language;
      document.documentElement.lang = language;
      api.setLocale(language).catch(() => {});
    },
    { immediate: true },
  );

  watch(
    () => settings.theme,
    (theme) => {
      // 存储或备份里的值不认识：退回默认的深色。
      if (!isThemeSetting(theme)) return void (settings.theme = "dark");
      applyTheme(theme);
    },
    { immediate: true },
  );

  watch(
    () => settings.font,
    (font) => {
      // 存储或备份里的值不认识：退回默认的内置字体。
      if (!isFontSetting(font)) return void (settings.font = "builtin");
      void applyFont(font);
    },
    { immediate: true },
  );

  /** 用户在命令助手里保存的命令：新建的，以及对默认命令的修改和删除。 */
  const commands = persisted<CustomCommand[]>(STORAGE.commands, []);
  commands.value = (Array.isArray(commands.value) ? commands.value : []).flatMap((c) => parseCustomCommand(c) ?? []);

  /** 添加命令；id 相同则替换。 */
  function saveCommand(command: CustomCommand): CustomCommand {
    const saved = { ...command, id: command.id || crypto.randomUUID() };
    const i = commands.value.findIndex((c) => c.id === saved.id);
    if (i >= 0) commands.value[i] = saved;
    else commands.value.push(saved);
    return saved;
  }

  function deleteCommand(id: string) {
    commands.value = removeCommand(commands.value, id);
    settings.favoriteCommands = settings.favoriteCommands.filter((item) => item !== id);
  }

  /** 撤销对默认命令的修改和删除；用户新建的命令保留。 */
  function restoreCommands() {
    commands.value = restoreDefaults(commands.value);
  }

  const tabs = ref<Tab[]>([]);
  /** 正在建立的连接，对应的标签页尚未出现。 */
  const connecting = ref<Tab[]>([]);
  const activeKey = ref<string | null>(null);
  const transfers = ref<Transfer[]>([]);
  /** 这个系统上能不能用隧道。 */
  const tunnelSupported = ref(false);
  const tunnel = ref<TunnelRun | null>(null);

  function resetEditRules() {
    settings.editRules = [...DEFAULT_EDIT_RULES];
  }

  async function init() {
    transfers.value = await api.transferList();
    await listen<Transfer>("transfer-update", ({ payload }) => {
      const i = transfers.value.findIndex((t) => t.id === payload.id);
      if (i >= 0) transfers.value[i] = payload;
      else transfers.value.push(payload);
    });
    await listen<{ id: string }>("session-closed", ({ payload }) => {
      const tab = tabs.value.find((t) => t.sessionId === payload.id);
      if (tab && tab.status === "connected") {
        tab.status = "closed";
        tab.error = t("conn.disconnected");
        void api.disconnect(payload.id);
      }
    });
    await listen<TunnelStatus>("tunnel-update", ({ payload }) => onTunnelUpdate(payload));
    await initTunnel();
  }

  /**
   * 新建或更新会话配置。`secret` 只在传入时写入，
   * 所以编辑配置而不重新输入密码时，记住的密码保持不变。
   */
  function saveProfile(profile: Profile, secret?: string): Profile {
    const saved = { ...profile, id: profile.id || crypto.randomUUID() };
    if (!saved.saveSecret) delete secrets.value[saved.id];
    else if (secret) secrets.value[saved.id] = secret;
    const i = profiles.value.findIndex((p) => p.id === saved.id);
    if (i >= 0) profiles.value[i] = saved;
    else profiles.value.push(saved);
    return saved;
  }

  function deleteProfile(id: string) {
    // 这台主机正当着隧道的出口：配置没了就没法重连，隧道一并关掉。
    if (tunnel.value?.profileId === id) void stopTunnel();
    profiles.value = profiles.value.filter((p) => p.id !== id);
    delete secrets.value[id];
  }

  /** 返回一个未被其他会话配置占用的显示名，必要时追加 " 2"、" 3"……。 */
  function uniqueProfileName(base: string, excludeId?: string): string {
    const taken = new Set(profiles.value.filter((p) => p.id !== excludeId).map((p) => p.name));
    if (!taken.has(base)) return base;
    for (let i = 2; ; i++) {
      const candidate = `${base} ${i}`;
      if (!taken.has(candidate)) return candidate;
    }
  }

  /** 复制一个主机，新配置紧跟在原配置之后。 */
  function duplicateProfile(id: string): Profile | null {
    const i = profiles.value.findIndex((p) => p.id === id);
    if (i < 0) return null;
    const src = profiles.value[i];
    const copy: Profile = {
      ...src,
      id: crypto.randomUUID(),
      name: uniqueProfileName(t("profile.copyName", { name: src.name })),
      pinned: false,
    };
    profiles.value.splice(i + 1, 0, copy);
    // 记住的密码以配置 id 为键保存，在本地一并复制过去。
    if (src.saveSecret && secrets.value[id]) secrets.value[copy.id] = secrets.value[id];
    return copy;
  }

  function togglePinned(id: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.pinned = !p.pinned;
  }

  /** 只对之后打开的标签页生效；已打开的保持原样。 */
  function toggleKeepAlive(id: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.keepAlive = !p.keepAlive;
  }

  /** 收藏主机上的一个目录，已收藏的则取消；新收藏的排在最后。 */
  function toggleFavoriteDir(id: string, dir: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.favoriteDirs = toggleFavorite(p.favoriteDirs ?? [], dir);
  }

  function toggleGroupCollapsed(name: string) {
    const set = new Set(sidebar.value.collapsed);
    if (!set.delete(name)) set.add(name);
    sidebar.value.collapsed = [...set];
  }

  /**
   * 应用某个分组内的拖拽排序：`ids` 是该分组的配置按新顺序排列的结果。
   * 只调整它们在全局数组中所占的位置，其他分组的条目相对顺序不变。
   */
  function reorderProfiles(ids: string[]) {
    if (ids.length < 2) return;
    const members = new Set(ids);
    const byId = new Map(profiles.value.map((p) => [p.id, p]));
    if (!ids.every((id) => byId.has(id))) return;
    let i = 0;
    profiles.value = profiles.value.map((p) => (members.has(p.id) ? byId.get(ids[i++])! : p));
  }

  /** 校验私钥并把内容保存到本地存储。 */
  async function importKey(source: { path?: string; content?: string; name?: string }): Promise<StoredKey> {
    const key = { id: crypto.randomUUID(), ...(await api.keyImport(source)) };
    keys.value.push(key);
    return key;
  }

  function deleteKey(id: string) {
    const users = profiles.value.filter((p) => p.keyId === id).map((p) => p.name);
    if (users.length) throw new Error(t("conn.keyInUse", { names: users.join(t("common.listSeparator")) }));
    keys.value = keys.value.filter((k) => k.id !== id);
  }

  /**
   * 连接主机，按服务器的要求依次询问主机密钥信任和凭据。`ask` 为 false 时不弹任何询问
   * （后台重连用）：需要用户回答的地方一律算失败。`host` 指定实际连到哪个地址，默认是配置里的；
   * 主机密钥仍按配置里的地址来认。
   */
  async function connectProfile(profile: Profile, opts: { ask?: boolean; host?: string } = {}): Promise<Connected> {
    const fail = (reason: string): Connected => ({ outcome: "failed", reason });
    const cancelled = (reason: string): Connected => ({ outcome: "cancelled", reason });
    const silent = opts.ask === false;
    const needsUser: Connected = { outcome: "failed", reason: t("conn.needsUser"), needsUser: true };

    const hostKey = `${profile.host}:${profile.port}`;
    const key = keys.value.find((k) => k.id === profile.keyId);
    if (profile.auth === "key" && !key) return fail(t("conn.noKey"));

    let secret = profile.saveSecret ? secrets.value[profile.id] : undefined;
    let remember = false;
    const askSecret = async (title: string) => {
      const answer = await prompt({
        title,
        placeholder: profile.auth === "key" ? key?.name ?? "" : `${profile.username}@${profile.host}`,
        password: true,
        checkbox: t("conn.remember"),
      });
      secret = answer?.value;
      remember = answer?.checked ?? false;
      return answer !== null;
    };
    if (profile.auth === "password" && !secret) {
      if (silent) return needsUser;
      if (!(await askSecret(t("conn.enterPassword")))) return cancelled(t("common.cancelled"));
    }

    for (;;) {
      const auth: ConnectAuth =
        profile.auth === "password"
          ? { type: "password", password: secret ?? "" }
          : profile.auth === "key"
            ? { type: "key", content: key!.content, passphrase: secret ?? null }
            : { type: "agent" };
      try {
        const sessionId = await api.connect({
          host: opts.host ?? profile.host,
          port: profile.port,
          username: profile.username,
          auth,
          knownHostKeys: knownHosts.value[hostKey] ?? {},
        });
        if (remember && secret) saveProfile({ ...profile, saveSecret: true }, secret);
        return { outcome: "connected", sessionId };
      } catch (e) {
        const err = e as AppError;
        const trust = () => {
          knownHosts.value[hostKey] = { ...knownHosts.value[hostKey], [err.algorithm!]: err.fingerprint! };
        };
        const wrongPassword = err.kind === "authFailed" && profile.auth === "password";
        const answerable = err.kind === "hostKeyUnknown" || err.kind === "hostKeyChanged" || err.kind === "passphraseRequired" || wrongPassword;
        if (!answerable) return fail(errorMessage(e));
        if (silent) return needsUser;
        if (err.kind === "hostKeyUnknown") {
          const ok = await confirm(
            t("conn.unknownHostTitle"),
            t("conn.unknownHostBody", { host: profile.host, algorithm: err.algorithm!, fingerprint: err.fingerprint! }),
            { ok: t("conn.trustAndConnect") },
          );
          if (!ok) return cancelled(t("conn.notTrusted"));
          trust();
        } else if (err.kind === "hostKeyChanged") {
          const ok = await confirm(
            t("conn.hostChangedTitle"),
            t("conn.hostChangedBody", { host: profile.host, fingerprint: err.fingerprint! }),
            { danger: true, ok: t("conn.replaceAndConnect") },
          );
          if (!ok) return cancelled(t("conn.hostChangedCancelled"));
          trust();
        } else if (err.kind === "passphraseRequired") {
          if (!(await askSecret(t("conn.enterPassphrase")))) return cancelled(t("common.cancelled"));
        } else if (!(await askSecret(t("conn.wrongPassword")))) {
          return cancelled(t("common.cancelled"));
        }
      }
    }
  }

  /** 连接标签页。返回连接的结局；失败原因留在 `tab.error` 里。 */
  async function connectTab(tab: Tab): Promise<"connected" | "cancelled" | "failed"> {
    const profile = profiles.value.find((p) => p.id === tab.profileId);
    if (!profile) {
      // 会话配置在标签页打开期间被删除了：没有可以用来连接的信息。
      tab.status = "closed";
      return "cancelled";
    }
    tab.status = "connecting";
    tab.error = null;
    const result = await connectProfile(profile);
    if (result.outcome === "connected") {
      tab.sessionId = result.sessionId;
      tab.status = "connected";
    } else {
      tab.error = result.reason;
      tab.status = "closed";
    }
    return result.outcome;
  }

  /** 标签页显示的名字。保活主机同时开着几个会话，或者进的不是 1 号时，带上会话的编号。 */
  function tabLabel(tab: Tab): string {
    const slot = slotOf(tab.profileId, tab.keepAlive);
    const several = tabs.value.filter((t) => t.profileId === tab.profileId).length > 1;
    return slot !== null && (several || slot > 1) ? `${tab.title} #${slot}` : tab.title;
  }

  /** 各主机正在进行的会话安排：同一台主机同时开两个标签页时一个一个来，免得两边选中同一个会话。 */
  const arranging = new Map<string, Promise<unknown>>();

  /**
   * 为刚连上的保活主机标签页安排会话（规则见 `planSession`）并记到标签页上。服务器上留着
   * 不止一个会话时列出来让用户挑；返回 null 表示用户取消了，这个标签页不开了。
   */
  function arrangeSession(profile: Profile, tab: Tab): Promise<string | null> {
    const run = async () => {
      const inUse = [...tabs.value, ...connecting.value].filter((t) => t !== tab && t.profileId === profile.id).map((t) => t.keepAlive);
      // 查不出来就当没有留着的，照常新开；服务器没有 tmux 的情况由终端在打开时处理。
      const kept = await api.keepAliveList(tab.sessionId!).catch(() => []);
      const plan = planSession(profile.id, inUse, kept.map((session) => session.name));
      if ("enter" in plan) return (tab.keepAlive = plan.enter);

      const sessions = plan.choose.map((name) => ({ ...kept.find((session) => session.name === name)!, slot: slotOf(profile.id, name)! }));
      // 空字符串表示新开一个。
      const answer = await chooseSession(profile.name, sessions);
      return answer === null ? null : (tab.keepAlive = answer || plan.fresh);
    };
    const turn = (arranging.get(profile.id) ?? Promise.resolve()).then(run, run);
    arranging.set(profile.id, turn.catch(() => {}));
    return turn;
  }

  /**
   * 连接主机，连接建立后才打开标签页。在此之前，连接过程只显示在主机卡片上；
   * 连接失败不会产生标签页，只提示错误。
   */
  async function openTab(profile: Profile) {
    const tab: Tab = reactive({
      key: crypto.randomUUID(),
      profileId: profile.id,
      title: profile.name,
      sessionId: null,
      status: "connecting",
      error: null,
      keepAlive: null,
      attached: false,
    });
    connecting.value.push(tab);
    let outcome = await connectTab(tab).catch((e) => {
      tab.error = errorMessage(e);
      return "failed" as const;
    });
    // 保活主机：连上之后才看得到服务器上留着哪些会话，这时再决定进哪个。
    if (outcome === "connected" && profile.keepAlive && (await arrangeSession(profile, tab)) === null) {
      await api.disconnect(tab.sessionId!).catch(() => {});
      outcome = "cancelled";
    }
    connecting.value = connecting.value.filter((t) => t !== tab);
    if (outcome === "connected") {
      tabs.value.push(tab);
      activeKey.value = tab.key;
    } else if (outcome === "failed") {
      message.error(t("conn.failed", { name: profile.name, error: tab.error ?? "" }), { duration: 10000, closable: true });
    }
  }

  /** 离开会话标签页，回到首页。 */
  function goHome() {
    activeKey.value = null;
  }

  async function reconnect(tab: Tab) {
    // 连点两次不能连出两个连接。
    if (tab.status === "connecting") return;
    tab.status = "connecting";
    // 先释放后端的旧会话，再打开新的。
    const oldId = tab.sessionId;
    tab.sessionId = null;
    if (oldId) await api.disconnect(oldId).catch(() => {});
    await connectTab(tab).catch((e) => {
      tab.error = errorMessage(e);
      tab.status = "closed";
    });
    const newId = tab.sessionId;
    if (!newId) return;
    // 连接期间标签页被关掉了：新连接已经没有人用，也没有人会去断开它。
    if (!tabs.value.some((t) => t.key === tab.key)) return void api.disconnect(newId).catch(() => {});
    // 因连接丢失而中断的传输改归新连接所有，重试时从中断处继续。
    if (oldId) await api.transferRebind(oldId, newId).catch(() => {});
  }

  /** 正在显示下面这个询问的标签页，避免再次点击时重复询问。 */
  const closing = new Set<string>();

  /**
   * 关闭开启了会话保持的标签页时，shell 仍留在服务器上运行。这正是会话保持的用意，
   * 但未必是用户关闭标签页的本意，所以交给用户决定：返回是否同时结束会话；
   * 返回 null 表示标签页不关了。
   */
  async function askEndSession(tab: Tab): Promise<boolean | null> {
    // 只有 tmux 里的活动终端，才有可以从这里结束的会话。
    if (!tab.keepAlive || !tab.attached || tab.status !== "connected") return false;
    const answer = await ask({
      title: t("keepAlive.closeTitle"),
      detail: tabLabel(tab),
      content: t("keepAlive.closeBody"),
      icon: HeartPulse,
      actions: [
        { value: "end", label: t("keepAlive.closeEnd"), tone: "danger" },
        { value: "keep", label: t("keepAlive.closeKeep"), tone: "primary" },
      ],
    });
    return answer === null ? null : answer === "end";
  }

  async function closeTab(key: string) {
    const tab = tabs.value.find((t) => t.key === key);
    if (!tab || closing.has(key)) return;
    closing.add(key);
    const end = await askEndSession(tab).finally(() => closing.delete(key));
    if (end === null) return;

    const i = tabs.value.indexOf(tab);
    if (i < 0) return;
    tabs.value.splice(i, 1);
    if (activeKey.value === key) {
      activeKey.value = tabs.value[Math.min(i, tabs.value.length - 1)]?.key ?? null;
    }
    if (!tab.sessionId) return;
    if (end && tab.keepAlive) {
      await api.keepAliveEnd(tab.sessionId, tab.keepAlive).catch((e) => message.error(errorMessage(e)));
    }
    // 趁连接还在时清理。
    await api.transferRelease(tab.sessionId).catch(() => {});
    await api.disconnect(tab.sessionId).catch(() => {});
  }

  // ---- 隧道 ----------------------------------------------------------------

  function saveTunnelConfig(id: string, config: TunnelConfig) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.tunnel = config;
  }

  /** 上一次状态里的流量计数，用来算速率。 */
  let tunnelCounted: { up: number; down: number; at: number } | null = null;

  function setTunnelStatus(run: TunnelRun, status: TunnelStatus) {
    const at = Date.now();
    run.upRate = rate(tunnelCounted && { bytes: tunnelCounted.up, at: tunnelCounted.at }, status.up, at);
    run.downRate = rate(tunnelCounted && { bytes: tunnelCounted.down, at: tunnelCounted.at }, status.down, at);
    tunnelCounted = { up: status.up, down: status.down, at };
    run.status = status;
  }

  /** 启动时：问清这个系统能不能用隧道。 */
  async function initTunnel() {
    tunnelSupported.value = await api.tunnelSupported().catch(() => false);
    // 界面重新加载之后，后端里还开着的隧道已经没有人管它重连了：关掉。
    if (tunnelSupported.value && (await api.tunnelStatus().catch(() => null))) await api.tunnelStop().catch(() => {});
  }

  /**
   * 开启隧道，以 `profile` 为出口。隧道用自己的一条 SSH 连接，不借用标签页的：
   * 关掉终端不该让隧道跟着断，隧道里的大流量也不该拖慢终端。
   */
  async function startTunnel(profile: Profile) {
    if (tunnel.value) return;
    const run: TunnelRun = reactive({ profileId: profile.id, sessionId: null, phase: "starting", status: null, upRate: 0, downRate: 0 });
    tunnel.value = run;
    tunnelCounted = null;
    const failed = (error: string) => message.error(t("tunnel.startFailed", { error }), { duration: 10000, closable: true });
    try {
      const connected = await connectProfile(profile);
      if (connected.outcome !== "connected") {
        if (connected.outcome === "failed") failed(connected.reason);
        return;
      }
      run.sessionId = connected.sessionId;
      // 存下来的设置过一遍校验：旧版本留下的、现在已经没有的项不交给后端。
      const status = await api.tunnelStart(connected.sessionId, tunnelRequest(parseTunnelConfig(profile.tunnel) ?? defaultTunnelConfig()));
      setTunnelStatus(run, status);
      run.phase = "running";
      if (status.warning) message.warning(status.warning, { duration: 15000, closable: true });
    } catch (e) {
      // 用户在系统的授权框里点了取消：不必再告诉他一遍。
      if ((e as AppError).kind !== "cancelled") failed(errorMessage(e));
    } finally {
      if (run.phase === "starting") {
        if (run.sessionId) await api.disconnect(run.sessionId).catch(() => {});
        if (tunnel.value === run) tunnel.value = null;
      }
    }
  }

  /** 关闭隧道；返回时虚拟网卡和路由已经撤掉。 */
  async function stopTunnel() {
    const run = tunnel.value;
    // 正在启动的拦不住（可能正等着用户输系统密码）；正在关的不用再关一次。
    if (!run || run.phase === "starting" || run.phase === "stopping") return;
    run.phase = "stopping";
    await api.tunnelStop().catch((e) => message.error(errorMessage(e)));
    if (run.sessionId) await api.disconnect(run.sessionId).catch(() => {});
    if (tunnel.value === run) tunnel.value = null;
  }

  function onTunnelUpdate(status: TunnelStatus) {
    const run = tunnel.value;
    // 启动和关闭过程中的状态由发起的那一方自己处理。
    if (!run || run.phase === "starting" || run.phase === "stopping") return;
    if (status.state === "stopped") {
      // 隧道自己没了（助手进程退出之类）：承载它的连接也不必留着。
      tunnel.value = null;
      if (run.sessionId) void api.disconnect(run.sessionId).catch(() => {});
      if (status.error) message.error(status.error, { duration: 15000, closable: true });
      return;
    }
    setTunnelStatus(run, status);
    if (status.state === "reconnecting" && run.phase === "running") {
      run.phase = "reconnecting";
      void reconnectTunnel(run);
    }
  }

  /**
   * 承载隧道的连接断了：在后台一直重连，直到连上或者隧道被关掉。这期间流量仍被引进隧道
   * （哪儿也去不了），不会悄悄改成直连。重连不弹任何询问；非得用户回答不可时关掉隧道。
   */
  async function reconnectTunnel(run: TunnelRun) {
    const waiting = () => tunnel.value === run && run.phase === "reconnecting";
    const old = run.sessionId;
    run.sessionId = null;
    if (old) await api.disconnect(old).catch(() => {});
    for (let attempt = 0; waiting(); attempt++) {
      await sleep(TUNNEL_RETRY[Math.min(attempt, TUNNEL_RETRY.length - 1)] * 1000);
      if (!waiting()) return;
      const profile = profiles.value.find((p) => p.id === run.profileId);
      if (!profile) return void stopTunnel();
      // 连回原来那个地址：只有它绕开了隧道，主机名这时也未必解析得了。
      const connected = await connectProfile(profile, { ask: false, host: run.status?.server }).catch((e): Connected => ({ outcome: "failed", reason: errorMessage(e) }));
      if (connected.outcome !== "connected") {
        if (!connected.needsUser || !waiting()) continue;
        message.error(t("tunnel.reconnectNeedsUser", { reason: connected.reason }), { duration: 0, closable: true });
        return void stopTunnel();
      }
      if (!waiting()) return void api.disconnect(connected.sessionId).catch(() => {});
      try {
        setTunnelStatus(run, await api.tunnelAttach(connected.sessionId));
        run.sessionId = connected.sessionId;
        run.phase = "running";
      } catch (e) {
        await api.disconnect(connected.sessionId).catch(() => {});
        message.error(errorMessage(e), { duration: 15000, closable: true });
        return void stopTunnel();
      }
    }
  }

  const snapshot = (): ConfigData => ({
    profiles: profiles.value,
    secrets: secrets.value,
    keys: keys.value,
    knownHosts: knownHosts.value,
    settings: { ...settings },
    commands: commands.value,
  });

  /** 把备份合并进当前配置，不删除任何已保存的内容。 */
  function importConfig(incoming: ConfigData) {
    const next = applyImport(snapshot(), incoming);
    profiles.value = next.profiles;
    secrets.value = next.secrets;
    keys.value = next.keys;
    knownHosts.value = next.knownHosts;
    commands.value = next.commands;
    Object.assign(settings, pickSettings({ ...settings }, next.settings));
  }

  async function clearTransfers() {
    await api.transferClear();
    transfers.value = await api.transferList();
  }

  return {
    profiles, keys, commands, saveCommand, deleteCommand, restoreCommands, tabs, tabLabel, connecting, activeKey, transfers, settings, sidebar, fileTree,
    init, saveProfile, deleteProfile, duplicateProfile, togglePinned, toggleKeepAlive, toggleFavoriteDir, toggleGroupCollapsed, reorderProfiles,
    importKey, deleteKey, importConfig, openTab, goHome, reconnect, closeTab, clearTransfers, resetEditRules,
    tunnelSupported, tunnel, saveTunnelConfig, startTunnel, stopTunnel,
  };
});

// 本模块（及其导入的模块）热更新后 store 实例仍然存活，热替换后的组件会带着新参数
// 调用旧的 store 代码，所以改为整页刷新。仅在开发时生效。
if (import.meta.hot) import.meta.hot.accept(() => location.reload());
