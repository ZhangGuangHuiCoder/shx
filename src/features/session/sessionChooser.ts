// 打开“进入哪个保活会话”的对话框（SessionChooser.vue）。
import { h } from "vue";
import type { KeptSession } from "../../shared/api";
import { showDialog } from "../../shared/ui";
import SessionChooser from "./SessionChooser.vue";

/** 列在对话框里的一个会话：服务器报告的情况，加上它是这台主机的第几个。 */
export interface SessionChoice extends KeptSession {
  slot: number;
}

/** 让用户挑一个会话进入。返回会话名；空字符串表示新开一个，取消或关闭时返回 null。 */
export function chooseSession(host: string, sessions: SessionChoice[]): Promise<string | null> {
  return showDialog<string>((done) => h(SessionChooser, { host, sessions, onDone: done }), 500);
}
