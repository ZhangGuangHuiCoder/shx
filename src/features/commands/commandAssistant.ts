// 命令助手当前的显示状态。对话框本身只在 App.vue 中挂载一次；
// 任何地方都可以从这里打开它：终端的右键菜单、应用菜单。
import { shallowReactive } from "vue";

/** 接收选中的命令；`run` 表示要求执行，而不只是输入。 */
export type UseCommand = (command: string, run: boolean) => void;

export const assistant = shallowReactive({
  open: false,
  /** 选中的命令交给它。没有终端可以填入时为 null，这时只能浏览和修改命令。 */
  use: null as UseCommand | null,
});

/** 当前显示的终端接收命令的入口（如果有）。 */
let current: UseCommand | null = null;

/** 终端显示或隐藏时登记、注销自己，从应用菜单打开命令助手时命令就填到它那里。 */
export function setCurrentTerminal(use: UseCommand, shown: boolean) {
  if (shown) current = use;
  else if (current === use) current = null;
}

/** 打开命令助手。不指定终端时，命令填入当前显示的终端。 */
export function openAssistant(use: UseCommand | null = current) {
  assistant.use = use;
  assistant.open = true;
}
