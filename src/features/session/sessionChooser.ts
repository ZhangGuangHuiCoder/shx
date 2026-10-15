import { h } from "vue";
import type { KeptSession } from "@/shared/api";
import { showDialog } from "@/shared/ui";
import SessionChooser from "@/features/session/SessionChooser.vue";

export interface SessionChoice extends KeptSession {
  slot: number;
}

/** 返回会话名；空串表示新开，取消时为 null。 */
export function chooseSession(host: string, sessions: SessionChoice[]): Promise<string | null> {
  return showDialog<string>((done) => h(SessionChooser, { host, sessions, onDone: done }), "w-lg");
}
