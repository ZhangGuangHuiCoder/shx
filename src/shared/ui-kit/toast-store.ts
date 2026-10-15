// 命令式消息条，由 Toaster.vue 渲染。
import { reactive } from "vue";

export type ToastType = "success" | "error" | "warning" | "info" | "loading";

export interface Toast {
  id: number;
  type: ToastType;
  content: string;
  /** 0 表示不自动关闭。 */
  duration: number;
  closable: boolean;
}

export interface ToastHandle {
  content: string;
  destroy(): void;
}

export interface ToastOptions {
  duration?: number;
  closable?: boolean;
}

export const toasts = reactive<Toast[]>([]);

let seq = 1;
const timers = new Map<number, ReturnType<typeof setTimeout>>();
const MAX = 5;

export function destroy(id: number) {
  const t = timers.get(id);
  if (t) {
    clearTimeout(t);
    timers.delete(id);
  }
  const i = toasts.findIndex((x) => x.id === id);
  if (i >= 0) toasts.splice(i, 1);
}

export function pushToast(type: ToastType, content: string, opts?: ToastOptions): ToastHandle {
  const id = seq++;
  const duration = opts?.duration ?? (type === "loading" ? 0 : 3000);
  const item: Toast = { id, type, content, duration, closable: opts?.closable ?? false };
  toasts.push(item);
  while (toasts.length > MAX) destroy(toasts[0].id);
  if (duration > 0) timers.set(id, setTimeout(() => destroy(id), duration));
  return {
    get content() {
      return item.content;
    },
    set content(v: string) {
      item.content = v;
    },
    destroy() {
      destroy(id);
    },
  };
}

const show = (type: ToastType) => (content: string, opts?: ToastOptions) => pushToast(type, content, opts);

/** loading 默认不自动关闭，需调用 destroy。 */
export const message = {
  success: show("success"),
  error: show("error"),
  warning: show("warning"),
  info: show("info"),
  loading: show("loading"),
};
