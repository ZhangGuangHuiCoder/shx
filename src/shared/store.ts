import { listen } from "@tauri-apps/api/event";
import { HeartPulse } from "@lucide/vue";
import { defineStore } from "pinia";
import { reactive, ref, watch, type Ref } from "vue";
import { applyImport, pickSettings, type ConfigData } from "@/features/backup/config";
import { parseCustomCommand, removeCommand, restoreDefaults, toggleFavorite, type CustomCommand } from "@/features/commands/commands";
import { planSession, slotOf } from "@/features/session/keepAlive";
import { FILE_TREE_DEFAULT_WIDTH } from "@/features/session/layout";
import { chooseSession } from "@/features/session/sessionChooser";
import { defaultTunnelConfig, parseTunnelConfig, rate, tunnelRequest } from "@/features/tunnel/tunnel";
import { api, errorMessage, type AppError, type ConnectAuth, type Profile, type StoredKey, type Transfer, type TunnelConfig, type TunnelStatus } from "@/shared/api";
import { applyFont, isFontSetting } from "@/shared/fonts";
import { isLocale, locale, systemLocale, t } from "@/shared/i18n";
import { DEFAULT_EDIT_RULES, defaultSettings, readStored, STORAGE, type Settings } from "@/shared/settings";
import { applyTheme, isThemeSetting } from "@/shared/theme";
import { ask, confirm, message, prompt } from "@/shared/ui";

export type { Settings };

/** 侧边栏偏好，仅本机保存。 */
export interface SidebarPrefs {
  /** 已折叠的分组名；置顶分组用 PINNED_GROUP。 */
  collapsed: string[];
}

/** 目录树显示偏好，仅本机保存。 */
export interface FileTreePrefs {
  show: boolean;
  width: number;
}

export const FILE_TREE_WIDTH = { min: 120, max: 520, initial: FILE_TREE_DEFAULT_WIDTH };

/** 置顶主机的伪分组名。 */
export const PINNED_GROUP = "__pinned__";

export interface Tab {
  key: string;
  profileId: string;
  title: string;
  sessionId: string | null;
  status: "connecting" | "connected" | "closed";
  error: string | null;
  /** 保活主机所用的 tmux 会话名，连上后确定。 */
  keepAlive: string | null;
  /** 终端是否真正在 tmux 会话中。 */
  attached: boolean;
}

/** 连接结果；用户在询问中放弃时为 cancelled。 */
type Connected =
  | { outcome: "connected"; sessionId: string }
  | { outcome: "cancelled" | "failed"; reason: string; /** 需要用户回答但本次不允许询问。 */ needsUser?: boolean };

/** 当前隧道（含启动、关闭中），同一时刻至多一条。 */
export interface TunnelRun {
  profileId: string;
  /** 隧道专用的 SSH 会话，重连期间为 null。 */
  sessionId: string | null;
  phase: "starting" | "running" | "reconnecting" | "stopping";
  status: TunnelStatus | null;
  /** 每秒字节数。 */
  upRate: number;
  downRate: number;
}

/** 隧道重连间隔（秒），用尽后保持最后一个。 */
const TUNNEL_RETRY = [1, 3, 8, 15, 30, 60];

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** 与 localStorage 同步的 ref（数据均未加密）。 */
function persisted<T>(key: string, fallback: T): Ref<T> {
  const initial = readStored(key, fallback);
  const state = ref(initial) as Ref<T>;
  // 同步写入，防止关窗口丢数据。
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
  /** 记住的密码或口令，键为配置 id。 */
  const secrets = persisted<Record<string, string>>(STORAGE.secrets, {});
  const keys = persisted<StoredKey[]>(STORAGE.keys, []);
  /** `host:port` → 算法 → 指纹。 */
  const knownHosts = persisted<Record<string, Record<string, string>>>(STORAGE.knownHosts, {});
  const stored = persisted<Partial<Settings>>(STORAGE.settings, {});
  const settings = reactive<Settings>(pickSettings(defaultSettings(), stored.value));
  watch(settings, (value) => (stored.value = { ...value }), { deep: true, flush: "sync" });

  const sidebar = persisted<SidebarPrefs>(STORAGE.sidebar, { collapsed: [] });
  // 清理旧版本遗留的 sort 字段。
  if ("sort" in (sidebar.value as unknown as Record<string, unknown>) || !Array.isArray(sidebar.value.collapsed)) {
    sidebar.value = { collapsed: Array.isArray(sidebar.value.collapsed) ? sidebar.value.collapsed : [] };
  }

  const fileTree = persisted<FileTreePrefs>(STORAGE.fileTree, { show: true, width: FILE_TREE_WIDTH.initial });
  if (typeof fileTree.value?.show !== "boolean" || !Number.isFinite(fileTree.value.width)) {
    fileTree.value = { show: true, width: FILE_TREE_WIDTH.initial };
  }

  // 同步给后端，用于后端消息。
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
      // 非法值回退为默认。
      if (!isThemeSetting(theme)) return void (settings.theme = "dark");
      applyTheme(theme);
    },
    { immediate: true },
  );

  watch(
    () => settings.font,
    (font) => {
      // 非法值回退为默认。
      if (!isFontSetting(font)) return void (settings.font = "builtin");
      void applyFont(font);
    },
    { immediate: true },
  );

  /** 用户命令：新建项及对默认命令的修改、删除。 */
  const commands = persisted<CustomCommand[]>(STORAGE.commands, []);
  commands.value = (Array.isArray(commands.value) ? commands.value : []).flatMap((c) => parseCustomCommand(c) ?? []);

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

  /** 恢复默认命令，保留用户新建的命令。 */
  function restoreCommands() {
    commands.value = restoreDefaults(commands.value);
  }

  const tabs = ref<Tab[]>([]);
  /** 连接中的标签页，尚未显示。 */
  const connecting = ref<Tab[]>([]);
  const activeKey = ref<string | null>(null);
  const transfers = ref<Transfer[]>([]);
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

  /** 新建或更新配置；未传 secret 时保留已记住的密码。 */
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
    // 该主机是隧道出口，删除后无法重连，一并关闭隧道。
    if (tunnel.value?.profileId === id) void stopTunnel();
    profiles.value = profiles.value.filter((p) => p.id !== id);
    delete secrets.value[id];
  }

  /** 生成不重名的显示名，必要时追加序号。 */
  function uniqueProfileName(base: string, excludeId?: string): string {
    const taken = new Set(profiles.value.filter((p) => p.id !== excludeId).map((p) => p.name));
    if (!taken.has(base)) return base;
    for (let i = 2; ; i++) {
      const candidate = `${base} ${i}`;
      if (!taken.has(candidate)) return candidate;
    }
  }

  /** 复制主机，插在原配置之后。 */
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
    // 一并复制记住的密码。
    if (src.saveSecret && secrets.value[id]) secrets.value[copy.id] = secrets.value[id];
    return copy;
  }

  function togglePinned(id: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.pinned = !p.pinned;
  }

  function toggleKeepAlive(id: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.keepAlive = !p.keepAlive;
  }

  /** 切换目录收藏状态。 */
  function toggleFavoriteDir(id: string, dir: string) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.favoriteDirs = toggleFavorite(p.favoriteDirs ?? [], dir);
  }

  function toggleGroupCollapsed(name: string) {
    const set = new Set(sidebar.value.collapsed);
    if (!set.delete(name)) set.add(name);
    sidebar.value.collapsed = [...set];
  }

  /** 应用组内拖拽排序：只在这些配置原有的位置上重排，不影响其他分组。 */
  function reorderProfiles(ids: string[]) {
    if (ids.length < 2) return;
    const members = new Set(ids);
    const byId = new Map(profiles.value.map((p) => [p.id, p]));
    if (!ids.every((id) => byId.has(id))) return;
    let i = 0;
    profiles.value = profiles.value.map((p) => (members.has(p.id) ? byId.get(ids[i++])! : p));
  }

  /** 校验并保存私钥。 */
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
   * 连接主机，按需询问主机密钥信任和凭据。
   * ask 为 false（后台重连）时不询问，需要回答即失败；host 可覆盖实际连接地址，主机密钥仍按配置地址校验。
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
        const answerable =
          err.kind === "hostKeyUnknown" || err.kind === "hostKeyChanged" || err.kind === "passphraseRequired" || err.kind === "agentEmpty" || wrongPassword;
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
        } else if (err.kind === "agentEmpty") {
          // agent 为空时提议用 ssh-add 加载默认私钥，只尝试一次。
          if (!(await confirm(t("conn.agentEmptyTitle"), t("conn.agentEmptyBody"), { ok: t("conn.agentAdd") }))) return cancelled(t("common.cancelled"));
          try {
            await api.agentAddKeys();
          } catch (added) {
            return fail(errorMessage(added));
          }
        } else if (!(await askSecret(t("conn.wrongPassword")))) {
          return cancelled(t("common.cancelled"));
        }
      }
    }
  }

  /** 连接标签页，失败原因写入 tab.error。 */
  async function connectTab(tab: Tab): Promise<"connected" | "cancelled" | "failed"> {
    const profile = profiles.value.find((p) => p.id === tab.profileId);
    if (!profile) {
      // 配置已被删除。
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

  /** 标签名；保活主机多开或非 1 号会话时附带编号。 */
  function tabLabel(tab: Tab): string {
    const slot = slotOf(tab.profileId, tab.keepAlive);
    const several = tabs.value.filter((t) => t.profileId === tab.profileId).length > 1;
    return slot !== null && (several || slot > 1) ? `${tab.title} #${slot}` : tab.title;
  }

  /** 按主机串行安排会话，避免两个标签选中同一会话。 */
  const arranging = new Map<string, Promise<unknown>>();

  /** 为保活标签页选定会话（规则见 planSession），多个可选时让用户挑；返回 null 表示取消。 */
  function arrangeSession(profile: Profile, tab: Tab): Promise<string | null> {
    const run = async () => {
      const inUse = [...tabs.value, ...connecting.value].filter((t) => t !== tab && t.profileId === profile.id).map((t) => t.keepAlive);
      // 查询失败按无会话处理；无 tmux 的情况由终端打开时处理。
      const kept = await api.keepAliveList(tab.sessionId!).catch(() => []);
      const plan = planSession(profile.id, inUse, kept.map((session) => session.name));
      if ("enter" in plan) return (tab.keepAlive = plan.enter);

      const sessions = plan.choose.map((name) => ({ ...kept.find((session) => session.name === name)!, slot: slotOf(profile.id, name)! }));
      // 空串表示新建。
      const answer = await chooseSession(profile.name, sessions);
      return answer === null ? null : (tab.keepAlive = answer || plan.fresh);
    };
    const turn = (arranging.get(profile.id) ?? Promise.resolve()).then(run, run);
    arranging.set(profile.id, turn.catch(() => {}));
    return turn;
  }

  /** 连接成功后才打开标签页，失败只提示错误。 */
  async function openTab(profile: Profile) {
    // 正在连接中，忽略重复点击。
    if (connecting.value.some((t) => t.profileId === profile.id)) return;
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
    // 保活主机连上后才能决定进入哪个会话。
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

  function goHome() {
    activeKey.value = null;
  }

  async function reconnect(tab: Tab) {
    // 防止重复重连。
    if (tab.status === "connecting") return;
    tab.status = "connecting";
    const oldId = tab.sessionId;
    tab.sessionId = null;
    if (oldId) await api.disconnect(oldId).catch(() => {});
    await connectTab(tab).catch((e) => {
      tab.error = errorMessage(e);
      tab.status = "closed";
    });
    const newId = tab.sessionId;
    if (!newId) return;
    // 连接期间标签已关闭，断开新连接。
    if (!tabs.value.some((t) => t.key === tab.key)) return void api.disconnect(newId).catch(() => {});
    // 中断的传输移交给新连接，可续传。
    if (oldId) await api.transferRebind(oldId, newId).catch(() => {});
  }

  /** 正在询问中的标签页，避免重复询问。 */
  const closing = new Set<string>();

  /** 关闭保活标签页时询问是否同时结束服务器上的会话；返回 null 表示取消关闭。 */
  async function askEndSession(tab: Tab): Promise<boolean | null> {
    // 只有已进入 tmux 的活动终端才有会话可结束。
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
    await api.transferRelease(tab.sessionId).catch(() => {});
    await api.disconnect(tab.sessionId).catch(() => {});
  }


  function saveTunnelConfig(id: string, config: TunnelConfig) {
    const p = profiles.value.find((x) => x.id === id);
    if (p) p.tunnel = config;
  }

  /** 上次的流量计数，用于计算速率。 */
  let tunnelCounted: { up: number; down: number; at: number } | null = null;

  function setTunnelStatus(run: TunnelRun, status: TunnelStatus) {
    const at = Date.now();
    run.upRate = rate(tunnelCounted && { bytes: tunnelCounted.up, at: tunnelCounted.at }, status.up, at);
    run.downRate = rate(tunnelCounted && { bytes: tunnelCounted.down, at: tunnelCounted.at }, status.down, at);
    tunnelCounted = { up: status.up, down: status.down, at };
    run.status = status;
  }

  /** 启动时检测隧道支持。 */
  async function initTunnel() {
    tunnelSupported.value = await api.tunnelSupported().catch(() => false);
    // 页面重载后残留的隧道无人维护，关闭。
    if (tunnelSupported.value && (await api.tunnelStatus().catch(() => null))) await api.tunnelStop().catch(() => {});
  }

  /** 开启隧道。使用独立 SSH 连接，与终端互不影响。 */
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
      // 规范化设置，剔除旧版本遗留项。
      const status = await api.tunnelStart(connected.sessionId, tunnelRequest(parseTunnelConfig(profile.tunnel) ?? defaultTunnelConfig()));
      setTunnelStatus(run, status);
      run.phase = "running";
      if (status.warning) message.warning(status.warning, { duration: 15000, closable: true });
    } catch (e) {
      // 用户取消授权，不再提示。
      if ((e as AppError).kind !== "cancelled") failed(errorMessage(e));
    } finally {
      if (run.phase === "starting") {
        if (run.sessionId) await api.disconnect(run.sessionId).catch(() => {});
        if (tunnel.value === run) tunnel.value = null;
      }
    }
  }

  async function stopTunnel() {
    const run = tunnel.value;
    // 启动中（可能在等系统授权）无法中止；关闭中无需重复。
    if (!run || run.phase === "starting" || run.phase === "stopping") return;
    run.phase = "stopping";
    await api.tunnelStop().catch((e) => message.error(errorMessage(e)));
    if (run.sessionId) await api.disconnect(run.sessionId).catch(() => {});
    if (tunnel.value === run) tunnel.value = null;
  }

  function onTunnelUpdate(status: TunnelStatus) {
    const run = tunnel.value;
    // 启动、关闭过程中的状态由发起方处理。
    if (!run || run.phase === "starting" || run.phase === "stopping") return;
    if (status.state === "stopped") {
      // 隧道异常结束，释放其连接。
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

  /** 后台持续重连，直到成功或隧道关闭；期间流量不会改为直连。需要用户回答时关闭隧道。 */
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
      // 连回原地址：只有它绕开隧道，此时域名也可能无法解析。
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

  /** 合并备份，不删除已有内容。 */
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

// 开发时热更新会让新组件调用旧 store，改为整页刷新。
if (import.meta.hot) import.meta.hot.accept(() => location.reload());
