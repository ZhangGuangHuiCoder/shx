import { CircleHelp, ListChecks, TextCursorInput, TriangleAlert } from "@lucide/vue";
import type { Component, VNode } from "vue";
import type { DialogAction, DialogInput, DialogItem, DialogOption, DialogResult } from "@/shared/ui-kit/AppDialog.vue";
import { message } from "@/shared/ui-kit/toast-store";
import { openApp, openCustom, type DialogWidth } from "@/shared/ui-kit/dialog-store";

export type { DialogItem };
export { message };

/** 显示自定义对话框，content 通过 done 交回结果；关闭时返回 null。 */
export function showDialog<T>(content: (done: (result: T | null) => void) => VNode, width: DialogWidth = "w-md"): Promise<T | null> {
  return openCustom<T>(content, width);
}

/** 是/否确认。 */
export async function confirm(
  title: string,
  content: string,
  opts: { danger?: boolean; ok?: string; icon?: Component; items?: DialogItem[] } = {},
): Promise<boolean> {
  const icon = opts.icon ?? (opts.danger ? TriangleAlert : CircleHelp);
  const result = await openApp({ icon, title, text: content, danger: opts.danger, ok: opts.ok, items: opts.items });
  return result !== null;
}

export type PromptResult = DialogResult;

/** 单输入框对话框，关闭时返回 null。 */
export function prompt(
  opts: DialogInput & { title: string; detail?: string; icon?: Component; checkbox?: string; ok?: string },
): Promise<PromptResult | null> {
  const { title, detail, icon, checkbox, ok, ...input } = opts;
  return openApp({ icon: icon ?? TextCursorInput, title, detail, input, checkbox, ok });
}

/** 单选对话框，关闭时返回 null。 */
export async function choose<T extends string>(opts: {
  title: string;
  content?: string;
  icon?: Component;
  items?: DialogItem[];
  options: (DialogOption & { value: T })[];
  initial: T;
}): Promise<T | null> {
  const result = await openApp(
    { icon: opts.icon ?? ListChecks, title: opts.title, text: opts.content, items: opts.items, options: opts.options, choice: opts.initial },
    "w-lg",
  );
  return result ? (result.value as T) : null;
}

/** 底栏按钮即选项，点击即返回；关闭时返回 null。 */
export async function ask<T extends string>(opts: {
  title: string;
  detail?: string;
  content?: string;
  icon?: Component;
  actions: (DialogAction & { value: T })[];
}): Promise<T | null> {
  const result = await openApp({ icon: opts.icon ?? CircleHelp, title: opts.title, detail: opts.detail, text: opts.content, actions: opts.actions });
  return result ? (result.value as T) : null;
}
