<script setup lang="ts">
// 应用各个小对话框的主体：一个询问、一个要输入的名称，一个要做的选择，或几种可以直接点的做法。
// 共用一个组件，外观和行为才能一致；由 `ui.ts` 打开。
import { NButton, NCheckbox, NInput } from "naive-ui";
import { computed, onMounted, ref, type Component } from "vue";
import type { FileKind } from "../fileIcons";
import { t } from "../i18n";
import DialogHeader from "./DialogHeader.vue";
import FileIcon from "./FileIcon.vue";

/** 对话框所涉及的文件或文件夹。 */
export interface DialogItem {
  name: string;
  isDir?: boolean;
  kind?: FileKind;
}

export interface DialogOption {
  value: string;
  label: string;
  /** 选项名称下方的一行说明，解释选它意味着什么。 */
  hint?: string;
}

/** 底栏里的一种做法：点它就是回答，不需要再确认。 */
export interface DialogAction {
  value: string;
  label: string;
  /** `primary`：推荐的做法，回车选它；`danger`：会造成破坏，单独放在左边。 */
  tone?: "primary" | "danger";
}

export interface DialogInput {
  initial?: string;
  placeholder?: string;
  password?: boolean;
  /** 预先选中开头的这么多个字符，例如不含扩展名的文件名。 */
  selectTo?: number;
  /** 返回值的问题所在；没有问题时返回 null。 */
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
    /** 标题下方的一行说明。 */
    detail?: string;
    danger?: boolean;
    /** 消息正文。保留换行。 */
    text?: string;
    items?: DialogItem[];
    input?: DialogInput;
    checkbox?: string;
    options?: DialogOption[];
    /** 初始选中的选项。 */
    choice?: string;
    /** 有它时，底栏不再是“取消 / 确定”，而是“取消”加上这些做法各一个按钮。 */
    actions?: DialogAction[];
    ok?: string;
  }>(),
  { detail: "", text: "", ok: "" },
);
const emit = defineEmits<{ done: [result: DialogResult | null] }>();

/** 长列表最多显示多少项；其余的用一行概括。 */
const ITEM_LIMIT = 6;

const value = ref(props.input?.initial ?? "");
const checked = ref(false);
const choice = ref(props.choice ?? props.options?.[0]?.value ?? "");
const body = ref<HTMLElement>();

const shown = computed(() => props.items?.slice(0, ITEM_LIMIT) ?? []);
/** 输入内容之后才校验：空输入框不需要报错。 */
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

/** 把焦点放到该按回车的地方：输入框；否则是默认的做法——除非它会造成破坏，那就是“取消”。 */
function focusDefault() {
  const input = body.value?.querySelector("input");
  if (input) {
    input.focus();
    if (props.input?.selectTo !== undefined) input.setSelectionRange(0, props.input.selectTo);
    else input.select();
    return;
  }
  // 这样按 Enter 就是回答：默认为“是”，除非“是”会造成破坏。
  const safe = body.value?.querySelector<HTMLElement>(".ok");
  (props.danger || !safe ? body.value?.querySelector<HTMLElement>(".cancel") : safe)?.focus();
}

// 外层对话框的焦点锁定在挂载时会把焦点放到第一个按钮上，而且发生在这里之后；
// 等它做完再放，否则落在最左边的按钮上的回车，可能正是那个会造成破坏的做法。
onMounted(() => setTimeout(focusDefault));
</script>

<template>
  <div ref="body" class="app-dialog-body">
    <DialogHeader :icon="icon" :title="title" :detail="detail" :tone="danger ? 'danger' : 'accent'" />

    <div v-if="text" class="text">{{ text }}</div>

    <div v-if="items?.length" class="items">
      <div v-for="item in shown" :key="item.name" class="item" :title="item.name">
        <FileIcon :name="item.name" :is-dir="item.isDir" :kind="item.kind ?? (item.isDir ? 'dir' : 'file')" :size="15" />
        <span class="ellipsis">{{ item.name }}</span>
      </div>
      <div v-if="items.length > shown.length" class="item more">{{ t("fm.moreItems", { count: items.length }) }}</div>
    </div>

    <div v-if="input" class="field">
      <NInput
        v-model:value="value"
        :type="input.password ? 'password' : 'text'"
        :show-password-on="input.password ? 'click' : undefined"
        :placeholder="input.placeholder ?? ''"
        :status="problem ? 'error' : undefined"
        @keydown.enter="!$event.isComposing && submit()"
      />
      <div v-if="problem" class="problem">{{ problem }}</div>
    </div>

    <div v-if="options" class="options" role="radiogroup">
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        role="radio"
        class="option"
        :class="{ on: option.value === choice }"
        :aria-checked="option.value === choice"
        @click="choice = option.value"
        @dblclick="submit"
      >
        <span class="mark" />
        <span class="words">
          <span class="label">{{ option.label }}</span>
          <span v-if="option.hint" class="hint">{{ option.hint }}</span>
        </span>
      </button>
    </div>

    <NCheckbox v-if="checkbox" v-model:checked="checked">{{ checkbox }}</NCheckbox>

    <footer v-if="actions">
      <NButton v-for="action in risky" :key="action.value" type="error" secondary @click="act(action)">{{ action.label }}</NButton>
      <span class="gap" />
      <NButton class="cancel" @click="emit('done', null)">{{ t("common.cancel") }}</NButton>
      <NButton
        v-for="action in safe"
        :key="action.value"
        :class="{ ok: action.tone === 'primary' }"
        :type="action.tone === 'primary' ? 'primary' : 'default'"
        @click="act(action)"
      >
        {{ action.label }}
      </NButton>
    </footer>
    <footer v-else>
      <NButton class="cancel" @click="emit('done', null)">{{ t("common.cancel") }}</NButton>
      <NButton class="ok" :type="danger ? 'error' : 'primary'" :disabled="!ready" @click="submit">{{ ok || t("common.ok") }}</NButton>
    </footer>
  </div>
</template>

<style scoped>
.app-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 20px;
}

.text {
  color: var(--text);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
  -webkit-user-select: text;
}

.items {
  max-height: 190px;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg);
}

.item {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  padding: 0 8px;
}

.item :deep(svg) {
  flex-shrink: 0;
}

.item.more {
  color: var(--text-dim);
  font-size: 12px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.problem {
  color: var(--danger);
  font-size: 12px;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.option {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: none;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s;
}

.option:hover {
  background: var(--bg-hover);
}

.option.on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.mark {
  flex-shrink: 0;
  width: 14px;
  height: 14px;
  margin-top: 3px;
  border: 1.5px solid var(--text-dim);
  border-radius: 50%;
}

.option.on .mark {
  border: 4px solid var(--accent);
}

.words {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.hint {
  color: var(--text-dim);
  font-size: 12px;
}

footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 4px;
}

footer :deep(.n-button) {
  min-width: 76px;
}

/* 把会造成破坏的做法推到左边，和其余按钮隔开。 */
.gap {
  flex: 1;
}
</style>
