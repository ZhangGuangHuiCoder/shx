// 快捷键注册表：在 window 捕获阶段统一分发，抢在 xterm 和菜单之前。
// store 只以类型引入、运行时注入，测试时不会带上 store 的模块副作用。
import { computed } from "vue";
import { FONT_DEFAULT, stepFont } from "@/shared/settings";
import { IS_MAC } from "@/shared/platform";
import type { MessageKey } from "@/shared/i18n";
import type { useApp } from "@/shared/store";

type AppStore = ReturnType<typeof useApp>;

/** KeyboardEvent 的子集，便于测试构造。 */
export interface ComboKey {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/** 由当前活动的 TerminalPane 注册。 */
export interface TerminalOps {
  copy(): void;
  paste(): void;
  selectAll(): void;
  clear(): void;
  openSearch(): void;
}

export type ShortcutGroup = "term" | "font" | "tab";

export type ShortcutId =
  | "term.copy"
  | "term.paste"
  | "term.selectAll"
  | "term.clear"
  | "term.find"
  | "term.fontUp"
  | "term.fontDown"
  | "term.fontReset"
  | "app.newTab"
  | "app.closeTab"
  | "app.nextTab"
  | "app.prevTab";

export interface ShortcutAction {
  id: ShortcutId;
  labelKey: MessageKey;
  group: ShortcutGroup;
  /** Tauri accelerator 写法。 */
  defaultKey: string;
  /** 没有可见终端时 term 为 null。 */
  run: (term: TerminalOps | null) => void;
}

function resizeFont(app: AppStore, delta: number) {
  app.settings.terminalFontSize = stepFont(app.settings.terminalFontSize, delta);
}

function cycleTab(app: AppStore, step: number) {
  if (!app.tabs.length) return;
  const i = app.tabs.findIndex((tab) => tab.key === app.activeKey);
  app.activeKey = app.tabs[i < 0 ? 0 : (i + step + app.tabs.length) % app.tabs.length].key;
}

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  // 默认键位避开 Ctrl+C/K/A/V/F 等 shell 控制字符。
  { id: "term.copy", labelKey: "shortcut.action.copy", group: "term", defaultKey: "CmdOrCtrl+Shift+C", run: (term) => term?.copy() },
  { id: "term.paste", labelKey: "shortcut.action.paste", group: "term", defaultKey: "CmdOrCtrl+Shift+V", run: (term) => term?.paste() },
  { id: "term.selectAll", labelKey: "shortcut.action.selectAll", group: "term", defaultKey: "CmdOrCtrl+Shift+A", run: (term) => term?.selectAll() },
  { id: "term.clear", labelKey: "shortcut.action.clear", group: "term", defaultKey: "CmdOrCtrl+Shift+K", run: (term) => term?.clear() },
  { id: "term.find", labelKey: "shortcut.action.find", group: "term", defaultKey: IS_MAC ? "Cmd+F" : "Ctrl+Shift+F", run: (term) => term?.openSearch() },
  { id: "term.fontUp", labelKey: "shortcut.action.fontUp", group: "font", defaultKey: "CmdOrCtrl+=", run: () => withApp((app) => resizeFont(app, 1)) },
  { id: "term.fontDown", labelKey: "shortcut.action.fontDown", group: "font", defaultKey: "CmdOrCtrl+-", run: () => withApp((app) => resizeFont(app, -1)) },
  { id: "term.fontReset", labelKey: "shortcut.action.fontReset", group: "font", defaultKey: "CmdOrCtrl+0", run: () => withApp((app) => void (app.settings.terminalFontSize = FONT_DEFAULT)) },
  { id: "app.newTab", labelKey: "shortcut.action.newTab", group: "tab", defaultKey: "CmdOrCtrl+T", run: () => withApp((app) => void app.goHome()) },
  { id: "app.closeTab", labelKey: "shortcut.action.closeTab", group: "tab", defaultKey: IS_MAC ? "Cmd+Shift+W" : "Ctrl+W", run: () => withApp((app) => {
    if (app.activeKey) void app.closeTab(app.activeKey);
  }) },
  { id: "app.nextTab", labelKey: "shortcut.action.nextTab", group: "tab", defaultKey: "Ctrl+Tab", run: () => withApp((app) => cycleTab(app, 1)) },
  { id: "app.prevTab", labelKey: "shortcut.action.prevTab", group: "tab", defaultKey: "Ctrl+Shift+Tab", run: () => withApp((app) => cycleTab(app, -1)) },
];

const MODIFIERS = new Set(["CmdOrCtrl", "Ctrl", "Alt", "Shift", "Cmd", "Meta"]);

type ParsedAccelerator = { ctrl: boolean; alt: boolean; shift: boolean; meta: boolean; key: string };

/** 解析 accelerator；写法非法时返回 null。 */
export function parseAccelerator(accelerator: string): ParsedAccelerator | null {
  const parts = accelerator.split("+");
  const key = parts.pop();
  if (!key) return null;
  const modifiers = new Set(parts);
  if (parts.length !== modifiers.size || [...modifiers].some((m) => !MODIFIERS.has(m))) return null;
  return {
    // CmdOrCtrl：mac 上是 Cmd，其他平台是 Ctrl。
    ctrl: modifiers.has("Ctrl") || (!IS_MAC && modifiers.has("CmdOrCtrl")),
    meta: modifiers.has("Meta") || modifiers.has("Cmd") || (IS_MAC && modifiers.has("CmdOrCtrl")),
    alt: modifiers.has("Alt"),
    shift: modifiers.has("Shift"),
    key,
  };
}

export function matchAccelerator(event: ComboKey, accelerator: string): boolean {
  const parsed = parseAccelerator(accelerator);
  return !!parsed && matchParsed(event, parsed);
}

function matchParsed(event: ComboKey, parsed: ParsedAccelerator): boolean {
  return (
    event.ctrlKey === parsed.ctrl &&
    event.altKey === parsed.alt &&
    event.shiftKey === parsed.shift &&
    event.metaKey === parsed.meta &&
    event.key.toLowerCase() === parsed.key.toLowerCase()
  );
}

/** 按键事件转 accelerator；只按了修饰键时返回 null。 */
export function acceleratorOf(event: ComboKey): string | null {
  if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return null;
  const parts: string[] = [];
  if (IS_MAC) {
    if (event.metaKey) parts.push("Cmd");
    if (event.ctrlKey) parts.push("Ctrl");
  } else if (event.ctrlKey || event.metaKey) {
    parts.push("CmdOrCtrl");
  }
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  // 字母统一大写存储。
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  parts.push(key);
  return parts.join("+");
}

const MAC_KEY: Record<string, string> = { CmdOrCtrl: "⌘", Cmd: "⌘", Meta: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };

/** 拆成单个键的展示文字：mac 用符号，其他平台用文字。 */
export function acceleratorParts(accelerator: string): string[] {
  const parsed = parseAccelerator(accelerator);
  if (!parsed) return [accelerator];
  const modifiers: string[] = [];
  if (IS_MAC) {
    if (parsed.ctrl) modifiers.push(MAC_KEY.Ctrl);
    if (parsed.alt) modifiers.push(MAC_KEY.Alt);
    if (parsed.shift) modifiers.push(MAC_KEY.Shift);
    if (parsed.meta) modifiers.push(MAC_KEY.Cmd);
  } else {
    if (parsed.ctrl || parsed.meta) modifiers.push(parsed.meta ? "Meta" : "Ctrl");
    if (parsed.alt) modifiers.push("Alt");
    if (parsed.shift) modifiers.push("Shift");
  }
  return [...modifiers, parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key];
}

export function formatAccelerator(accelerator: string): string {
  return acceleratorParts(accelerator).join(IS_MAC ? "" : "+");
}

/** 未安装时动作不执行、键位取默认值，测试无需 Pinia。 */
let appStore: (() => AppStore) | null = null;

function withApp(run: (app: AppStore) => void) {
  const app = appStore?.();
  if (app) run(app);
}

/** 生效键位：合法的自定义值优先，否则用默认值。 */
export const bindings = computed<Record<ShortcutId, string>>(() => {
  const stored = appStore?.().settings.shortcuts ?? {};
  const out = {} as Record<ShortcutId, string>;
  for (const { id, defaultKey } of SHORTCUT_ACTIONS) {
    const custom = stored[id];
    out[id] = typeof custom === "string" && parseAccelerator(custom) ? custom : defaultKey;
  }
  return out;
});

/** 预先解析，避免每次按键重复解析。 */
const parsedBindings = computed(() =>
  SHORTCUT_ACTIONS.map((action) => ({ action, parsed: parseAccelerator(bindings.value[action.id])! })),
);

let terminal: TerminalOps | null = null;
/** 录制键位时的接收者，录制期间不分发动作。 */
let recordHandler: ((event: KeyboardEvent) => void) | null = null;

export function setShortcutTerminal(ops: TerminalOps, shown: boolean) {
  // 切换标签时旧终端的注销可能晚于新终端的登记，只注销自己。
  if (shown) terminal = ops;
  else if (terminal === ops) terminal = null;
}

/** 开始录制，返回结束录制的函数。 */
export function startRecording(handler: (event: KeyboardEvent) => void): () => void {
  recordHandler = handler;
  return () => void (recordHandler = null);
}

/** 焦点在输入框或编辑器中时不分发；xterm 的隐藏输入框除外。 */
function inEditable(target: EventTarget | null): boolean {
  const el = target instanceof HTMLElement ? target : null;
  if (!el) return false;
  return (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable) && !el.closest(".xterm");
}

function onKey(event: KeyboardEvent) {
  if (recordHandler) {
    event.preventDefault();
    event.stopImmediatePropagation();
    recordHandler(event);
    return;
  }
  if (inEditable(event.target)) return;
  for (const { action, parsed } of parsedBindings.value) {
    if (!matchParsed(event, parsed)) continue;
    event.preventDefault();
    event.stopImmediatePropagation();
    action.run(terminal);
    return;
  }
}

let installed = false;

/** 安装全局分发。须在子组件挂载前调用，先于标题栏的快捷键监听注册，才能拦截录制中的按键。 */
export function installShortcuts(getStore: () => AppStore) {
  appStore = getStore;
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", onKey, true);
}
