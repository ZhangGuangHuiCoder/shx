import { shallowReactive, type Component, type VNode } from "vue";

/** 右键菜单的一行。 */
export type MenuEntry =
  | {
      label: string;
      icon: Component;
      disabled?: boolean;
      /** 以警示色显示。 */
      danger?: boolean;
      /** 对应的键盘快捷键，显示在右侧。 */
      hint?: string;
      action: () => unknown;
    }
  /** 不是命令：在顶部说明菜单针对的对象。 */
  | { heading: string; detail?: string; icon?: () => VNode }
  /** 不是命令：分组标题，说明它下面的几项是哪一类。 */
  | { group: string }
  | "divider";

/** 每个窗口唯一的右键菜单，由 `ContextMenuHost` 渲染。 */
export const contextMenu = shallowReactive({ show: false, x: 0, y: 0, entries: [] as MenuEntry[] });

/** 在指针处显示 `entries`，替换已打开的菜单。 */
export function openContextMenu(event: MouseEvent, entries: MenuEntry[]) {
  event.preventDefault();
  Object.assign(contextMenu, { show: true, x: event.clientX, y: event.clientY, entries });
}

const MAC = /Macintosh|Mac OS X/.test(navigator.userAgent);

/** 快捷键在当前平台上的写法，用于菜单提示。 */
export const shortcut = {
  mod: (key: string) => (MAC ? `⌘${key}` : `Ctrl+${key}`),
  delete: MAC ? "⌘⌫" : "Del",
  enter: "↵",
};
