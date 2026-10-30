// 命令助手的全局状态，任何地方都可打开。
import { shallowReactive } from "vue";
import { IS_WINDOWS } from "@/shared/platform";
import type { HostOs } from "@/shared/api";

/** run 为 true 表示直接执行。 */
export type UseCommand = (command: string, run: boolean) => void;

/** 本机系统；本地 shell 与无终端上下文时按此选择命令集。 */
export const CLIENT_OS: HostOs = IS_WINDOWS ? "windows" : "linux";

export const assistant = shallowReactive({
  open: false,
  /** 命令接收方；无终端时为 null，只能浏览和编辑。 */
  use: null as UseCommand | null,
  /** 当前展示的默认命令集对应的系统；在助手内手动切换，默认本机系统。 */
  os: CLIENT_OS as HostOs,
});

let current: UseCommand | null = null;

/** 终端显示/隐藏时登记或注销；只注销自己，避免切换标签时的时序问题。 */
export function setCurrentTerminal(use: UseCommand, shown: boolean) {
  if (shown) current = use;
  else if (current === use) current = null;
}

/** 默认填入当前显示的终端；命令集沿用助手上次的选择。 */
export function openAssistant(use: UseCommand | null = current) {
  assistant.use = use;
  assistant.open = true;
}
