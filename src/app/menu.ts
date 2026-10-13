// 应用菜单。菜单的内容只描述一次（`appMenu`），按平台画成两种样子：
// macOS 上是屏幕顶部的原生菜单栏（`installMenu`），Windows 上画在自绘的标题栏里（TitleBar.vue）。
import { CheckMenuItem, Menu, PredefinedMenuItem, Submenu, type PredefinedMenuItemOptions } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { shallowRef, watch } from "vue";
import { exportBackup, importBackup } from "../features/backup/backup";
import { openAssistant } from "../features/commands/commandAssistant";
import { openRulesDialog } from "../features/rules/rulesDialog";
import { api } from "../shared/api";
import { locale, LOCALES, t, type MessageKey } from "../shared/i18n";
import { FONT_DEFAULT, FONT_MAX, FONT_MIN, type Settings } from "../shared/settings";
import { CUSTOM_TITLEBAR } from "../shared/titlebar";
import { useApp } from "../stores/app";
import { openAbout } from "./about";
import { quit } from "./closeGuard";
import { checkForUpdates } from "./update";
import { revealWindow } from "./window";

/** 菜单里的一项，与平台无关。 */
export type MenuNode =
  | {
      type: "item";
      label: string;
      /** 快捷键，写法同 Tauri（`CmdOrCtrl+=`、`F11`）。 */
      accelerator?: string;
      /** 为 false 时执行前不必先让主窗口回到眼前。 */
      reveal?: boolean;
      action: () => unknown;
    }
  /** 带勾选标记的项；几项互斥时，各自的 `checked` 只有一个为真。 */
  | { type: "check"; label: string; checked: () => boolean; action: () => unknown }
  | { type: "submenu"; label: string; enabled?: () => boolean; items: MenuNode[] }
  /** 行为由系统提供的项（复制、隐藏等），只出现在原生菜单里；文字使用应用语言。 */
  | { type: "native"; item: PredefinedMenuItemOptions["item"]; label?: string }
  | { type: "separator" };

/** 菜单栏上的一个菜单。 */
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

/**
 * 用当前语言描述应用菜单。原生菜单栏上，标准菜单项保留原生行为；尤其是“编辑”菜单，
 * macOS 的终端和编辑器里复制粘贴靠它才能工作。自绘的标题栏里没有这些：
 * Windows 上复制粘贴不依赖菜单，“隐藏”之类是 macOS 才有的概念。
 */
export function appMenu(): TopMenu[] {
  const settings = useApp().settings;
  type FontSetting = "terminalFontSize" | "editorFontSize";
  const resize = (key: FontSetting, delta: number) => () => (settings[key] = Math.min(FONT_MAX, Math.max(FONT_MIN, settings[key] + delta)));
  const reset = (key: FontSetting) => () => (settings[key] = FONT_DEFAULT);
  /** 几选一的设置：每个选项一项，当前的那个带勾选标记。 */
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
  // 不用原生的退出项：它会立刻结束进程，不管有没有打开的会话。
  const quitItem: MenuNode = { type: "item", label: t("menu.quit"), accelerator: "CmdOrCtrl+Q", reveal: false, action: () => void quit() };

  const application: TopMenu = {
    label: "shx",
    items: [
      // 不用系统的“关于”面板：应用自己的那个带品牌标识，两个平台一致。
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
      // 双击文件的行为。文件类型列表需要编辑界面，所以那一项会打开一个窗口；
      // 其他文件的处理方式直接在这里选。
      { type: "item", label: t("menu.editRules"), action: openRulesDialog },
      { type: "item", label: t("menu.commands"), action: () => openAssistant() },
      { type: "submenu", label: t("settings.otherDoubleClick"), items: choices("otherDoubleClick", DOUBLE_CLICK.map(([value, label]) => [value, t(label)])) },
      SEPARATOR,
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
  const view: TopMenu = {
    label: t("menu.view"),
    items: [
      toggle("showHidden", "menu.showHidden"),
      SEPARATOR,
      { type: "item", label: t("menu.termFontUp"), accelerator: "CmdOrCtrl+=", action: resize("terminalFontSize", 1) },
      { type: "item", label: t("menu.termFontDown"), accelerator: "CmdOrCtrl+-", action: resize("terminalFontSize", -1) },
      { type: "item", label: t("menu.termFontReset"), accelerator: "CmdOrCtrl+0", action: reset("terminalFontSize") },
      SEPARATOR,
      { type: "item", label: t("menu.editorFontUp"), action: resize("editorFontSize", 1) },
      { type: "item", label: t("menu.editorFontDown"), action: resize("editorFontSize", -1) },
      { type: "item", label: t("menu.editorFontReset"), action: reset("editorFontSize") },
      { type: "submenu", label: t("menu.font"), items: choices("font", FONTS.map(([value, label]) => [value, t(label)])) },
      SEPARATOR,
      {
        // 布局：会话里要不要文件管理器、它放在哪一侧，以及窗口底部要不要状态栏。
        type: "submenu",
        label: t("menu.layout"),
        items: [
          toggle("showFiles", "menu.showFiles"),
          {
            // 文件管理器隐藏时，选位置看不到任何效果，所以停用。
            type: "submenu",
            label: t("menu.filePosition"),
            enabled: () => settings.showFiles,
            items: choices("fileLayout", FILE_LAYOUTS.map(([value, label]) => [value, t(label)])),
          },
          SEPARATOR,
          toggle("showStatusBar", "menu.showStatusBar"),
        ],
      },
      { type: "submenu", label: t("menu.language"), items: choices("language", LOCALES.map(({ value, label }) => [value, label])) },
      { type: "submenu", label: t("menu.theme"), items: choices("theme", THEMES.map(([value, label]) => [value, t(label)])) },
      SEPARATOR,
      CUSTOM_TITLEBAR
        ? { type: "item", label: t("menu.fullscreen"), accelerator: "F11", action: () => void toggleFullscreen() }
        : native("Fullscreen", t("menu.fullscreen")),
    ],
  };
  if (!CUSTOM_TITLEBAR) return [application, file, edit, view];
  const help: TopMenu = { label: t("menu.help"), items: [checkUpdate, SEPARATOR, aboutItem] };
  return [file, view, help];
}

// ---- 原生菜单栏 ------------------------------------------------------------

/**
 * 菜单项的动作。菜单栏在主窗口最小化或被别的窗口挡住时照样能点，所以先让主窗口
 * 回到眼前，再执行：用户点了菜单，就该看到它的结果。
 */
const shown = (run: () => unknown) => () => {
  void revealWindow();
  run();
};

/** 本模块上次安装的菜单，留着以便替换时释放。 */
let installed: Menu | undefined;
/** 已安装菜单里随设置变化的状态（勾选、可用），以及把它们写回菜单的办法。 */
const states = shallowRef<{ read: () => boolean; write: (value: boolean) => unknown }[]>([]);
type States = typeof states.value;

/**
 * 把当前设置同步到已安装菜单上。原生勾选项一点击就会自行切换，即使它本来就是当前选项；
 * 所以每次点击和每次外部变更之后，都要把真实状态写回去。
 */
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

/** 用当前语言构建原生菜单栏并安装。 */
async function buildMenu() {
  const tracked: States = [];
  const submenus: Submenu[] = [];
  for (const menu of appMenu()) submenus.push(await toSubmenu(menu, tracked));
  const menu = await Menu.new({ items: submenus });
  await menu.setAsAppMenu();
  // macOS 会往这些菜单里追加自己的项：“编辑”菜单的“自动填充”，以及“全部关闭”
  // 之类未翻译的备选项。把应用菜单之外的每个菜单截回构建时的项数。
  for (const submenu of submenus.slice(1)) {
    await api.menuTrim(await submenu.text(), (await submenu.items()).length).catch(() => {});
  }
  states.value = tracked;
  const replaced = installed;
  installed = menu;
  await replaced?.close().catch(() => {});
}

/** 安装原生菜单栏，并在语言变化时重建。自绘标题栏的平台上菜单画在标题栏里，这里什么都不做。 */
export async function installMenu() {
  if (CUSTOM_TITLEBAR) return;
  // 设置也会在菜单之外被修改：文件工具栏、首页的主题按钮、恢复备份。
  watch(() => states.value.map(({ read }) => read()), syncChecks, { flush: "post" });
  // 菜单文字在创建时就固定了，换语言只能重建菜单。一次只建一个：
  // 两次构建交错的话，留下来的可能是先开始的那个，也就是旧语言的菜单。
  let building = buildMenu();
  watch(locale, () => (building = building.then(buildMenu, buildMenu).catch(() => {})));
  await building;
}
