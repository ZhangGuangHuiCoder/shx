<script setup lang="ts">
// 文本输入框，支持自动撑高的 textarea；用 v-model:value 绑定。
import { Eye, EyeOff, X } from "@lucide/vue";
import { computed, nextTick, ref, watch } from "vue";
import { field, fieldIcon, fieldInput } from "@/shared/ui-kit/styles";

const props = withDefaults(
  defineProps<{
    value?: string | null;
    type?: string; // text | password | textarea
    placeholder?: string;
    disabled?: boolean;
    status?: "error" | "warning" | "success";
    /** textarea 按内容自动撑高的行数范围。 */
    autosize?: { minRows?: number; maxRows?: number };
    showPasswordOn?: "click" | "mousedown";
    rows?: number;
    clearable?: boolean;
    round?: boolean;
    size?: "small" | "medium" | "large";
    autofocus?: boolean;
    /** 不带外壳，由外部容器提供。 */
    bare?: boolean;
  }>(),
  { type: "text", value: "", size: "medium" },
);

const emit = defineEmits<{ "update:value": [value: string] }>();

const SIZE = { small: "min-h-7 text-xs", medium: "min-h-9 text-sm", large: "min-h-10 text-sm" } as const;

const inner = computed({
  get: () => props.value ?? "",
  set: (v: string) => emit("update:value", v),
});

const reveal = ref(false);
const inputType = computed(() => (props.type === "password" ? (reveal.value ? "text" : "password") : props.type));

const ta = ref<HTMLTextAreaElement | null>(null);
const minRows = props.autosize?.minRows ?? props.rows ?? 2;
const maxRows = props.autosize?.maxRows ?? minRows;
const taStyle = ref<Record<string, string>>({});

function resize() {
  const el = ta.value;
  if (!el || props.type !== "textarea") return;
  const lh = Number.parseFloat(getComputedStyle(el).lineHeight) || 18;
  el.style.height = "auto";
  taStyle.value = {
    height: `${Math.min(Math.max(el.scrollHeight, lh * minRows), lh * maxRows)}px`,
    resize: "none",
  };
}

// 行数固定时高度与内容无关，只在挂载时量一次。
if (maxRows !== minRows) {
  watch(
    () => props.value,
    () => nextTick(resize),
  );
}
watch(
  () => props.type,
  async () => {
    await nextTick();
    resize();
  },
  { immediate: true },
);

function togglePassword() {
  if (props.showPasswordOn === "mousedown") return;
  reveal.value = !reveal.value;
}

function clear() {
  inner.value = "";
  ta.value?.focus();
}

function focus() {
  const el = ta.value ?? rootEl.value?.querySelector("input");
  el?.focus();
}

function select() {
  rootEl.value?.querySelector<HTMLInputElement>("input,textarea")?.select();
}

const rootEl = ref<HTMLElement | null>(null);

defineExpose({ focus, select });
</script>

<template>
  <div
    ref="rootEl"
    :class="
      bare
        ? ['flex w-full items-center gap-2', disabled && 'cursor-not-allowed opacity-50']
        : [
            field,
            SIZE[size],
            disabled && 'cursor-not-allowed opacity-50',
            type === 'textarea' ? 'items-stretch py-2' : 'items-center',
            round ? 'rounded-full' : 'rounded-xl',
            // 仅 error 有独立样式。
            status === 'error' && 'border-red-500! focus-within:ring-red-500/25! dark:border-red-400!',
          ]
    "
  >
    <span v-if="$slots.prefix || $slots.icon" class="shrink-0"><slot name="prefix" /><slot name="icon" /></span>
    <textarea
      v-if="type === 'textarea'"
      ref="ta"
      v-model="inner"
      :placeholder="placeholder"
      :disabled="disabled"
      :rows="minRows"
      :autofocus="autofocus || undefined"
      :class="[fieldInput, 'resize-none leading-5']"
      :style="taStyle"
    />
    <input
      v-else
      v-model="inner"
      :type="inputType"
      :placeholder="placeholder"
      :disabled="disabled"
      :autofocus="autofocus || undefined"
      :class="fieldInput"
    />
    <button
      v-if="clearable && inner && !disabled"
      type="button"
      :class="fieldIcon"
      aria-label="Clear"
      @mousedown.prevent
      @click="clear"
    >
      <X :size="13" />
    </button>
    <button
      v-if="type === 'password' && showPasswordOn === 'click' && inner"
      type="button"
      :class="fieldIcon"
      :aria-label="reveal ? 'Hide password' : 'Show password'"
      @click="togglePassword"
    >
      <EyeOff v-if="reveal" :size="14" />
      <Eye v-else :size="14" />
    </button>
    <span v-if="$slots.suffix" class="shrink-0"><slot name="suffix" /></span>
  </div>
</template>
