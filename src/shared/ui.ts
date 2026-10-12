import { CircleHelp, ListChecks, TextCursorInput, TriangleAlert } from "@lucide/vue";
import { createDiscreteApi } from "naive-ui";
import { computed, h, type Component, type VNode } from "vue";
import AppDialog, { type DialogAction, type DialogInput, type DialogItem, type DialogOption, type DialogResult } from "./components/AppDialog.vue";
import { naiveTheme, themeOverrides } from "./theme";

export type { DialogItem };

export const { message, dialog } = createDiscreteApi(["message", "dialog"], {
  // 用 computed 让独立的对话框和消息跟随应用主题。
  configProviderProps: computed(() => ({ theme: naiveTheme.value, themeOverrides })),
});

type DialogProps = InstanceType<typeof AppDialog>["$props"];

/**
 * 显示一个对话框并返回结果。框内的内容由 `content` 画，它拿到的 `done` 用来交回结果；
 * 以任何方式关闭（取消、Esc、点击外部）时返回 null。
 */
export function showDialog<T>(content: (done: (result: T | null) => void) => VNode, width = 420): Promise<T | null> {
  return new Promise((resolve) => {
    let settled = false;
    const settle = (result: T | null) => {
      if (settled) return;
      settled = true;
      resolve(result);
      shown.destroy();
    };
    // 遮罩、焦点锁定和动画由 Naive 提供；框内的内容全部是应用自己的
    // （见 styles.css 中的 `.app-dialog`）。
    const shown = dialog.create({
      class: "app-dialog",
      style: { width: `${width}px` },
      showIcon: false,
      closable: false,
      content: () => content(settle),
      onClose: () => settle(null),
      onEsc: () => settle(null),
      onMaskClick: () => settle(null),
    });
  });
}

/** 显示应用通用的小对话框（AppDialog）并返回结果。 */
function open(props: Omit<DialogProps, "onDone">, width = 420): Promise<DialogResult | null> {
  return showDialog<DialogResult>((done) => h(AppDialog, { ...props, onDone: done }), width);
}

/** 询问是或否。`danger` 用于确认后会造成破坏的操作。 */
export async function confirm(
  title: string,
  content: string,
  opts: { danger?: boolean; ok?: string; icon?: Component; /** 询问所涉及的文件。 */ items?: DialogItem[] } = {},
): Promise<boolean> {
  const icon = opts.icon ?? (opts.danger ? TriangleAlert : CircleHelp);
  return (await open({ icon, title, text: content, danger: opts.danger, ok: opts.ok, items: opts.items })) !== null;
}

export type PromptResult = DialogResult;

/** 单个输入框的对话框。关闭时返回 null。 */
export function prompt(
  opts: DialogInput & { title: string; /** 标题下方的一行说明。 */ detail?: string; icon?: Component; checkbox?: string; ok?: string },
): Promise<PromptResult | null> {
  const { title, detail, icon, checkbox, ok, ...input } = opts;
  return open({ icon: icon ?? TextCursorInput, title, detail, input, checkbox, ok });
}

/** 让用户从几个选项中选一个。返回选中的值，关闭时返回 null。 */
export async function choose<T extends string>(opts: {
  title: string;
  content?: string;
  icon?: Component;
  items?: DialogItem[];
  options: (DialogOption & { value: T })[];
  initial: T;
}): Promise<T | null> {
  const result = await open(
    { icon: opts.icon ?? ListChecks, title: opts.title, text: opts.content, items: opts.items, options: opts.options, choice: opts.initial },
    460,
  );
  return result ? (result.value as T) : null;
}

/**
 * 让用户在几种做法里直接点一种：每种做法是底栏里的一个按钮，点了就算数，不用先选再确认。
 * 返回点中的值，取消或关闭时返回 null。
 */
export async function ask<T extends string>(opts: {
  title: string;
  /** 标题下方的一行说明。 */
  detail?: string;
  content?: string;
  icon?: Component;
  actions: (DialogAction & { value: T })[];
}): Promise<T | null> {
  const result = await open({ icon: opts.icon ?? CircleHelp, title: opts.title, detail: opts.detail, text: opts.content, actions: opts.actions });
  return result ? (result.value as T) : null;
}
