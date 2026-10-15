// 命令助手的全局状态，任何地方都可打开。
import { shallowReactive } from "vue";

/** run 为 true 表示直接执行。 */
export type UseCommand = (command: string, run: boolean) => void;

export const assistant = shallowReactive({
  open: false,
  /** 命令接收方；无终端时为 null，只能浏览和编辑。 */
  use: null as UseCommand | null,
});

let current: UseCommand | null = null;

/** 终端显示/隐藏时登记或注销；只注销自己，避免切换标签时的时序问题。 */
export function setCurrentTerminal(use: UseCommand, shown: boolean) {
  if (shown) current = use;
  else if (current === use) current = null;
}

/** 默认填入当前显示的终端。 */
export function openAssistant(use: UseCommand | null = current) {
  assistant.use = use;
  assistant.open = true;
}
