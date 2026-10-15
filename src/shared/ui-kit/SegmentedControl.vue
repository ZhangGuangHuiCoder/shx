<script setup lang="ts" generic="T extends string">
// 分段选择，方向键切换（跳过禁用项）。tone="ok" 的选项选中时为绿色。
import { computed, type Component } from "vue";
import { segmentSlider } from "@/shared/ui-kit/segmentSlider";
import { capsule, focusRing, raisedChip } from "@/shared/ui-kit/styles";

export interface SegmentOption<V extends string = string> {
  value: V;
  label: string;
  icon?: Component;
  disabled?: boolean;
  tone?: "ok";
}

const props = defineProps<{ options: SegmentOption<T>[]; label?: string; size?: "small" }>();
const value = defineModel<T>({ required: true });

const selected = computed(() => props.options.find((option) => option.value === value.value));
const SIZE = {
  medium: { root: "grid", button: "h-7 min-w-0 px-3 text-sm", on: "font-semibold text-zinc-900 dark:text-white" },
  small: { root: "inline-grid shrink-0", button: "h-6 px-2.5 text-xs whitespace-nowrap", on: "font-medium text-zinc-800 dark:text-zinc-100" },
};
const size = computed(() => SIZE[props.size ?? "medium"]);
const OK_ON = "font-medium text-emerald-700 dark:text-emerald-300";

const slider = computed(() => segmentSlider(props.options.length, props.options.findIndex((option) => option.value === value.value)));

function move(step: number) {
  const list = props.options;
  let at = list.findIndex((option) => option.value === value.value);
  for (let i = 0; i < list.length; i++) {
    at = (at + step + list.length) % list.length;
    if (!list[at].disabled) break;
  }
  value.value = list[at].value;
  return at;
}

function onKey(event: KeyboardEvent) {
  const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
  if (!step) return;
  event.preventDefault();
  const at = move(step);
  ((event.currentTarget as HTMLElement).children[at] as HTMLElement | undefined)?.focus();
}
</script>

<template>
  <div :class="[capsule, size.root, 'relative isolate auto-cols-fr grid-flow-col gap-0.5 rounded-full p-0.5']" role="radiogroup" :aria-label="label" @keydown="onKey">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      role="radio"
      :aria-checked="option.value === value"
      :tabindex="option.value === value ? 0 : -1"
      :disabled="option.disabled"
      :class="[
        focusRing,
        'relative z-10 flex cursor-pointer items-center justify-center gap-1.5 rounded-full bg-transparent transition duration-200 ease-out disabled:cursor-not-allowed disabled:opacity-40',
        size.button,
        option.value === value
          ? option.tone === 'ok' ? OK_ON : size.on
          : 'text-zinc-500 enabled:hover:text-zinc-800 enabled:active:scale-97 dark:text-zinc-400 dark:enabled:hover:text-zinc-100',
      ]"
      @click="value = option.value"
    >
      <component :is="option.icon" v-if="option.icon" :size="14" :stroke-width="2" />
      <span class="truncate">{{ option.label }}</span>
    </button>
    <!-- 须放在按钮之后：方向键按 children[at] 定位按钮。 -->
    <span
      v-if="slider"
      :class="[
        selected?.tone === 'ok' ? 'bg-emerald-500/15 ring-1 ring-emerald-500/40' : raisedChip,
        'pointer-events-none absolute inset-y-0.5 left-0.5 rounded-full transition duration-300 ease-out motion-reduce:transition-none',
      ]"
      :style="slider"
      aria-hidden="true"
    />
  </div>
</template>
