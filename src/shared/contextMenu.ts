import { shallowReactive, type Component, type VNode } from "vue";
import { Check } from "@lucide/vue";
import { IS_MAC } from "@/shared/platform";

export type MenuEntry =
  | {
      label: string;
      icon: Component;
      disabled?: boolean;
      danger?: boolean;
      /** 右侧显示的快捷键提示。 */
      hint?: string;
      action: () => unknown;
      /** 行尾的附加小按钮。 */
      aside?: { icon: Component; title: string; action: () => unknown };
    }
  /** 顶部标题，说明菜单作用对象。 */
  | { heading: string; detail?: string; icon?: () => VNode }
  /** 分组标题。 */
  | { group: string }
  | "divider";

/** 全局唯一的右键菜单，由 ContextMenuHost 渲染。anchor 标记打开菜单的对象，供其保持高亮。 */
export const contextMenu = shallowReactive({ show: false, x: 0, y: 0, entries: [] as MenuEntry[], anchor: null as unknown });

export function openContextMenu(event: MouseEvent, entries: MenuEntry[], anchor: unknown = null) {
  event.preventDefault();
  Object.assign(contextMenu, { show: true, x: event.clientX, y: event.clientY, entries, anchor });
}

/** 菜单中的快捷键提示文字。 */
export const shortcut = {
  mod: (key: string) => (IS_MAC ? `⌘${key}` : `Ctrl+${key}`),
  delete: IS_MAC ? "⌘⌫" : "Del",
  enter: "↵",
};

/** 空图标，保持未勾选项的文字对齐。 */
const Unchecked = () => null;
export const checkIcon = (checked: boolean): Component => (checked ? Check : Unchecked);
