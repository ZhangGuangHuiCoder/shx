<script setup lang="ts">
// 通用小对话框：确认、输入、单选或直接选择操作，由 ui.ts 打开。
import { computed, onMounted, ref, type Component } from "vue";
import type { FileKind } from "@/shared/fileIcons";
import { t } from "@/shared/i18n";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import { dangerText, dim, fg, hoverFill, line } from "@/shared/ui-kit/styles";

export interface DialogItem {
  name: string;
  isDir?: boolean;
  kind?: FileKind;
}

export interface DialogOption {
  value: string;
  label: string;
  hint?: string;
}

/** 底栏按钮，点击即回答。 */
export interface DialogAction {
  value: string;
  label: string;
  /** primary 为回车默认项；danger 单独放在左侧。 */
  tone?: "primary" | "danger";
}

export interface DialogInput {
  initial?: string;
  placeholder?: string;
  password?: boolean;
  /** 预选前 N 个字符（如不含扩展名的文件名）。 */
  selectTo?: number;
  /** 返回错误信息，合法时返回 null。 */
  validate?: (value: string) => string | null;
}

export interface DialogResult {
  value: string;
  checked: boolean;
}

const props = withDefaults(
  defineProps<{
    icon: Component;
    title: string;
    detail?: string;
    danger?: boolean;
    /** 正文，保留换行。 */
    text?: string;
    items?: DialogItem[];
    input?: DialogInput;
    checkbox?: string;
    options?: DialogOption[];
    choice?: string;
    /** 提供时底栏为“取消”加各操作按钮，替代“确定”。 */
    actions?: DialogAction[];
    ok?: string;
  }>(),
  { detail: "", text: "", ok: "" },
);
const emit = defineEmits<{ done: [result: DialogResult | null] }>();

/** 最多显示的条目数，其余汇总为一行。 */
const ITEM_LIMIT = 6;

const value = ref(props.input?.initial ?? "");
const checked = ref(false);
const choice = ref(props.choice ?? props.options?.[0]?.value ?? "");
const body = ref<HTMLElement>();

const shown = computed(() => props.items?.slice(0, ITEM_LIMIT) ?? []);
/** 空输入不报错。 */
const problem = computed(() => (props.input && value.value ? props.input.validate?.(value.value) ?? null : null));
const ready = computed(() => !props.input || (!!value.value && !problem.value));

function submit() {
  if (ready.value) emit("done", { value: props.options ? choice.value : value.value, checked: checked.value });
}

function act(action: DialogAction) {
  emit("done", { value: action.value, checked: checked.value });
}

const risky = computed(() => props.actions?.filter((action) => action.tone === "danger") ?? []);
const safe = computed(() => props.actions?.filter((action) => action.tone !== "danger") ?? []);

/** 聚焦输入框；否则聚焦默认按钮，危险操作时聚焦“取消”。 */
function focusDefault() {
  const input = body.value?.querySelector("input");
  if (input) {
    input.focus();
    if (props.input?.selectTo !== undefined) input.setSelectionRange(0, props.input.selectTo);
    else input.select();
    return;
  }
  const safe = body.value?.querySelector<HTMLElement>(".ok");
  (props.danger || !safe ? body.value?.querySelector<HTMLElement>(".cancel") : safe)?.focus();
}

// 对话框焦点锁定会在此之后聚焦第一个按钮（可能是危险操作），延后一拍再设焦点。
onMounted(() => setTimeout(focusDefault));
</script>

<template>
  <div ref="body" class="flex flex-col gap-4 p-5">
    <DialogHeader :icon="icon" :title="title" :detail="detail" :tone="danger ? 'danger' : 'accent'" />

    <div v-if="text" :class="[fg, 'text-sm/relaxed break-words whitespace-pre-wrap select-text']">{{ text }}</div>

    <!-- 高度上限按 ITEM_LIMIT + 1 行（7 × 28px）留足，避免多余滚动条。 -->
    <div v-if="items?.length" :class="[line, 'max-h-52 overflow-y-auto rounded-xl border bg-zinc-900/5 p-1 dark:bg-black/20']">
      <div v-for="item in shown" :key="item.name" class="flex h-7 items-center gap-2 px-2 text-sm" :title="item.name">
        <FileIcon :name="item.name" :is-dir="item.isDir" :kind="item.kind ?? (item.isDir ? 'dir' : 'file')" :size="15" class="shrink-0" />
        <span class="truncate">{{ item.name }}</span>
      </div>
      <div v-if="items.length > shown.length" :class="[dim, 'flex h-7 items-center gap-2 px-2 text-xs']">{{ t("fm.moreItems", { count: items.length }) }}</div>
    </div>

    <div v-if="input" class="flex flex-col gap-1.5">
      <UiInput
        v-model:value="value"
        :type="input.password ? 'password' : 'text'"
        :show-password-on="input.password ? 'click' : undefined"
        :placeholder="input.placeholder ?? ''"
        :status="problem ? 'error' : undefined"
        @keydown.enter="!$event.isComposing && submit()"
      />
      <div v-if="problem" :class="[dangerText, 'text-xs']">{{ problem }}</div>
    </div>

    <div v-if="options" class="flex flex-col gap-1.5" role="radiogroup">
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        role="radio"
        :class="[
          fg,
          'group flex cursor-pointer items-start gap-2.5 rounded-xl border px-3 py-2 text-left text-sm transition-colors duration-150',
          option.value === choice ? 'border-blue-500/50 bg-blue-500/10 dark:bg-blue-400/15' : [line, hoverFill, 'bg-transparent'],
        ]"
        :aria-checked="option.value === choice"
        @click="choice = option.value"
        @dblclick="submit"
      >
        <span class="mt-0.5 size-3.5 shrink-0 rounded-full" :class="option.value === choice ? 'border-4 border-blue-500' : 'border-2 border-zinc-400 dark:border-zinc-500'" />
        <span class="flex min-w-0 flex-col gap-px">
          <span>{{ option.label }}</span>
          <span v-if="option.hint" :class="[dim, 'text-xs']">{{ option.hint }}</span>
        </span>
      </button>
    </div>

    <UiCheckbox v-if="checkbox" v-model:checked="checked">{{ checkbox }}</UiCheckbox>

    <footer v-if="actions" class="mt-1 flex justify-end gap-2">
      <UiButton v-for="action in risky" :key="action.value" class="min-w-20" type="error" @click="act(action)">{{ action.label }}</UiButton>
      <span class="flex-1" />
      <UiButton class="cancel min-w-20" @click="emit('done', null)">{{ t("common.cancel") }}</UiButton>
      <UiButton
        v-for="action in safe"
        :key="action.value"
        :class="['min-w-20', { ok: action.tone === 'primary' }]"
        :type="action.tone === 'primary' ? 'primary' : 'default'"
        @click="act(action)"
      >
        {{ action.label }}
      </UiButton>
    </footer>
    <footer v-else class="mt-1 flex justify-end gap-2">
      <UiButton class="cancel min-w-20" @click="emit('done', null)">{{ t("common.cancel") }}</UiButton>
      <UiButton class="ok min-w-20" :type="danger ? 'error' : 'primary'" :disabled="!ready" @click="submit">{{ ok || t("common.ok") }}</UiButton>
    </footer>
  </div>
</template>
