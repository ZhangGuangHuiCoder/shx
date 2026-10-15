// 应用菜单：内容由 appMenu 统一描述，macOS 装成原生菜单栏，Windows 画在自绘标题栏里。
import { CheckMenuItem, Menu, PredefinedMenuItem, Submenu, type PredefinedMenuItemOptions } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { shallowRef, watch } from "vue";
import { exportBackup, importBackup } from "@/features/backup/backup";
import { openAssistant } from "@/features/commands/commandAssistant";
import { openRulesDialog } from "@/features/rules/rulesDialog";
import { openShortcutsDialog } from "@/features/shortcuts/shortcutsDialog";
import { api } from "@/shared/api";
import { locale, LOCALES, t, type MessageKey } from "@/shared/i18n";
import { FONT_DEFAULT, stepFont, type Settings } from "@/shared/settings";
import { CUSTOM_TITLEBAR } from "@/shared/titlebar";
import { useApp } from "@/shared/store";
import { openAbout } from "@/app/about";
import { quit } from "@/app/closeGuard";
import { checkForUpdates } from "@/app/update";
import { revealWindow } from "@/app/window";

export type MenuNode =
  | {
      type: "item";
      label: string;
      /** Tauri accelerator 写法。 */
      accelerator?: string;
      /** 为 false 时执行前不把主窗口带到前台。 */
      reveal?: boolean;
      action: () => unknown;
    }
  | { type: "check"; label: string; checked: () => boolean; action: () => unknown }
  | { type: "submenu"; label: string; enabled?: () => boolean; items: MenuNode[] }
  /** 系统预定义项，仅用于原生菜单。 */
  | { type: "native"; item: PredefinedMenuItemOptions["item"]; label?: string }
  | { type: "separator" };

export interface TopMenu {
  label: string;
  items: MenuNode[];
}

const DOUBLE_CLICK: [Settings["otherDoubleClick"], MessageKey][] = [
  ["download", "settings.dblDownload"],
  ["edit", "settings.dblEdit"],
  ["none", "settings.dblNone"],
];

const FILE_LAYOUTS: [Settings["fileLayout"], MessageKey][] = [
  ["bottom", "menu.fileBottom"],
  ["right", "menu.fileRight"],
];

const THEMES: [Settings["theme"], MessageKey][] = [
  ["light", "theme.light"],
  ["dark", "theme.dark"],
  ["auto", "theme.auto"],
];

const FONTS: [Settings["font"], MessageKey][] = [
  ["builtin", "font.builtin"],
  ["system", "font.system"],
];

const SEPARATOR: MenuNode = { type: "separator" };
const native = (item: PredefinedMenuItemOptions["item"], label?: string): MenuNode => ({ type: "native", item, label });

async function toggleFullscreen() {
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}

/** 按当前语言生成菜单。macOS 的复制粘贴依赖原生“编辑”菜单，自绘标题栏不需要这些项。 */
export function appMenu(): TopMenu[] {
  const settings = useApp().settings;
  /** 单选设置：当前值打勾。 */
  const choices = <K extends keyof Settings>(key: K, options: [Settings[K], string][]): MenuNode[] =>
    options.map(([value, label]) => ({ type: "check", label, checked: () => settings[key] === value, action: () => (settings[key] = value) }));
  const toggle = (key: "showHidden" | "showFiles" | "showStatusBar", label: MessageKey): MenuNode => ({
    type: "check",
    label: t(label),
    checked: () => settings[key],
    action: () => (settings[key] = !settings[key]),
  });

  const aboutItem: MenuNode = { type: "item", label: t("menu.about"), action: () => void openAbout() };
  const checkUpdate: MenuNode = { type: "item", label: t("menu.checkUpdate"), action: () => void checkForUpdates() };
  // 原生退出项会直接结束进程，跳过会话询问，所以自己实现。
  const quitItem: MenuNode = { type: "item", label: t("menu.quit"), accelerator: "CmdOrCtrl+Q", reveal: false, action: () => void quit() };

  const application: TopMenu = {
    label: "Shell Xplore",
    items: [
      aboutItem,
      checkUpdate,
      SEPARATOR,
      native("Hide", t("menu.hide")),
      native("HideOthers", t("menu.hideOthers")),
      native("ShowAll", t("menu.showAll")),
      SEPARATOR,
      quitItem,
    ],
  };
  const file: TopMenu = {
    label: t("menu.file"),
    items: [
      { type: "item", label: t("menu.exportConfig"), action: exportBackup },
      { type: "item", label: t("menu.importConfig"), action: importBackup },
      SEPARATOR,
      CUSTOM_TITLEBAR ? quitItem : native("CloseWindow", t("menu.closeWindow")),
    ],
  };
  const edit: TopMenu = {
    label: t("menu.edit"),
    items: [
      native("Undo", t("menu.undo")),
      native("Redo", t("menu.redo")),
      SEPARATOR,
      native("Cut", t("common.cut")),
      native("Copy", t("common.copy")),
      native("Paste", t("common.paste")),
      native("SelectAll", t("common.selectAll")),
    ],
  };
  /** 快捷键由 shortcuts.ts 分发，这里不注册 accelerator。 */
  const termFontSize: MenuNode = {
    type: "submenu",
    label: t("menu.termFontSize"),
    items: [
      { type: "item", label: t("menu.fontUp"), action: () => (settings.terminalFontSize = stepFont(settings.terminalFontSize, 1)) },
      { type: "item", label: t("menu.fontDown"), action: () => (settings.terminalFontSize = stepFont(settings.terminalFontSize, -1)) },
      { type: "item", label: t("menu.fontReset"), action: () => (settings.terminalFontSize = FONT_DEFAULT) },
    ],
  };
  const view: TopMenu = {
    label: t("menu.view"),
    items: [
      toggle("showFiles", "menu.showFiles"),
      {
        // 文件管理器隐藏时停用。
        type: "submenu",
        label: t("menu.filePosition"),
        enabled: () => settings.showFiles,
        items: choices("fileLayout", FILE_LAYOUTS.map(([value, label]) => [value, t(label)])),
      },
      toggle("showHidden", "menu.showHidden"),
      SEPARATOR,
      toggle("showStatusBar", "menu.showStatusBar"),
      SEPARATOR,
      termFontSize,
      { type: "submenu", label: t("menu.font"), items: choices("font", FONTS.map(([value, label]) => [value, t(label)])) },
      SEPARATOR,
      { type: "submenu", label: t("menu.theme"), items: choices("theme", THEMES.map(([value, label]) => [value, t(label)])) },
      { type: "submenu", label: t("menu.language"), items: choices("language", LOCALES.map(({ value, label }) => [value, label])) },
      SEPARATOR,
      CUSTOM_TITLEBAR
        ? { type: "item", label: t("menu.fullscreen"), accelerator: "F11", action: () => void toggleFullscreen() }
        : native("Fullscreen", t("menu.fullscreen")),
    ],
  };
  const tools: TopMenu = {
    label: t("menu.tools"),
    items: [
      { type: "item", label: t("menu.commands"), action: () => openAssistant() },
      { type: "item", label: t("menu.shortcuts"), action: openShortcutsDialog },
      SEPARATOR,
      { type: "item", label: t("menu.editRules"), action: openRulesDialog },
      { type: "submenu", label: t("settings.otherDoubleClick"), items: choices("otherDoubleClick", DOUBLE_CLICK.map(([value, label]) => [value, t(label)])) },
    ],
  };
  if (!CUSTOM_TITLEBAR) return [application, file, edit, view, tools];
  const help: TopMenu = { label: t("menu.help"), items: [checkUpdate, SEPARATOR, aboutItem] };
  return [file, view, tools, help];
}

/** 先把主窗口带到前台再执行，确保用户看到结果。 */
const shown = (run: () => unknown) => () => {
  void revealWindow();
  run();
};

/** 上次安装的菜单，替换时释放。 */
let installed: Menu | undefined;
/** 随设置变化的菜单状态（勾选、可用）及其写回方法。 */
const states = shallowRef<{ read: () => boolean; write: (value: boolean) => unknown }[]>([]);
type States = typeof states.value;

/** 原生勾选项点击后会自行切换，因此每次点击或外部变更后都写回真实状态。 */
const syncChecks = () => states.value.forEach(({ read, write }) => void write(read()));

type NativeItem = PredefinedMenuItem | CheckMenuItem | Submenu | { text: string; accelerator?: string; action: () => void };

async function toNative(node: MenuNode, tracked: States): Promise<NativeItem> {
  switch (node.type) {
    case "separator":
      return PredefinedMenuItem.new({ item: "Separator" });
    case "native":
      return PredefinedMenuItem.new({ item: node.item, text: node.label });
    case "item":
      return { text: node.label, accelerator: node.accelerator, action: node.reveal === false ? () => void node.action() : shown(node.action) };
    case "check": {
      const item = await CheckMenuItem.new({
        text: node.label,
        checked: node.checked(),
        action: shown(() => {
          node.action();
          syncChecks();
        }),
      });
      tracked.push({ read: node.checked, write: (checked) => item.setChecked(checked) });
      return item;
    }
    case "submenu": {
      const { enabled } = node;
      const submenu = await toSubmenu(node, tracked, enabled?.() ?? true);
      if (enabled) tracked.push({ read: enabled, write: (value) => submenu.setEnabled(value) });
      return submenu;
    }
  }
}

async function toSubmenu(menu: TopMenu, tracked: States, enabled = true) {
  const items: NativeItem[] = [];
  for (const node of menu.items) items.push(await toNative(node, tracked));
  return Submenu.new({ text: menu.label, enabled, items });
}

async function buildMenu() {
  const tracked: States = [];
  const submenus: Submenu[] = [];
  for (const menu of appMenu()) submenus.push(await toSubmenu(menu, tracked));
  const menu = await Menu.new({ items: submenus });
  await menu.setAsAppMenu();
  // macOS 会往菜单里追加未翻译的系统项，截回构建时的项数。
  for (const submenu of submenus.slice(1)) {
    await api.menuTrim(await submenu.text(), (await submenu.items()).length).catch(() => {});
  }
  // 让系统自动判断“隐藏其他”等项是否可用。
  await api.menuAutoenable(await submenus[0].text()).catch(() => {});
  states.value = tracked;
  const replaced = installed;
  installed = menu;
  await replaced?.close().catch(() => {});
}

/** 安装原生菜单栏，语言变化时重建；自绘标题栏平台不做任何事。 */
export async function installMenu() {
  if (CUSTOM_TITLEBAR) return;
  // 设置可能在菜单外被修改，需同步。
  watch(() => states.value.map(({ read }) => read()), syncChecks, { flush: "post" });
  // 串行重建，避免交错时留下旧语言的菜单。
  let building = buildMenu();
  watch(locale, () => (building = building.then(buildMenu, buildMenu).catch(() => {})));
  await building;
}
