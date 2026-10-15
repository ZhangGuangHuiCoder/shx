// 命令式对话框队列，由 DialogHost.vue 渲染。
import { h, markRaw, reactive, type VNode } from "vue";
import AppDialog, { type DialogResult } from "@/shared/ui-kit/AppDialog.vue";

type AppDialogProps = Omit<InstanceType<typeof AppDialog>["$props"], "onDone">;

export type DialogWidth = "w-sm" | "w-md" | "w-lg" | "w-xl";

export interface DialogEntry {
  id: number;
  width: DialogWidth;
  open: boolean;
  render: (entry: DialogEntry) => VNode | null;
  resolve: (result: unknown) => void;
}

export const dialogs = reactive<DialogEntry[]>([]);

let seq = 1;

function open<T>(width: DialogWidth, render: (entry: DialogEntry, done: (result: T | null) => void) => VNode): Promise<T | null> {
  return new Promise((resolve) => {
    const entry: DialogEntry = {
      id: seq++,
      width,
      open: true,
      resolve: resolve as (result: unknown) => void,
      render: () => null,
    };
    entry.render = markRaw(() => render(entry, (result) => closeDialog(entry, result)));
    dialogs.push(entry);
  });
}

/** 先交还结果，退场动画后移出队列。 */
export function closeDialog(entry: DialogEntry, result: unknown) {
  if (!entry.open) return;
  entry.open = false;
  entry.resolve(result);
  setTimeout(() => {
    const i = dialogs.findIndex((d) => d.id === entry.id);
    if (i >= 0) dialogs.splice(i, 1);
  }, 190);
}

export function openCustom<T>(render: (done: (result: T | null) => void) => VNode, width: DialogWidth = "w-md"): Promise<T | null> {
  return open<T>(width, (_entry, done) => render(done));
}

export function openApp(props: AppDialogProps, width: DialogWidth = "w-md"): Promise<DialogResult | null> {
  return open<DialogResult>(width, (_entry, done) => h(AppDialog, { ...props, onDone: done }));
}
