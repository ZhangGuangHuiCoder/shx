<script setup lang="ts">
// 通用按钮。type 只决定文字和图标颜色。
import { Loader2 } from "@lucide/vue";
import { computed } from "vue";
import { accentText, capsule, dangerText, fg, focusRing, raisedChip } from "@/shared/ui-kit/styles";

type Type = "default" | "primary" | "error";
type Size = "tiny" | "small" | "medium" | "large";

const props = withDefaults(
  defineProps<{
    type?: Type;
    size?: Size;
    ghost?: boolean;
    /** 开关处于打开状态。 */
    selected?: boolean;
    circle?: boolean;
    loading?: boolean;
    disabled?: boolean;
  }>(),
  { type: "default", size: "medium" },
);

const SIZE: Record<Size, string> = {
  tiny: "h-6 px-2.5 text-xs",
  small: "h-7 px-3 text-xs",
  medium: "h-9 px-4 text-sm",
  large: "h-10 px-5 text-sm",
};

// 圆形钮单独定义：与 SIZE 的内边距类混用时优先级不确定。
const CIRCLE: Record<Size, string> = {
  tiny: "size-6 text-xs",
  small: "size-7 text-xs",
  medium: "size-9 text-sm",
  large: "size-10 text-sm",
};

const HOVER = "enabled:hover:bg-white/45 dark:enabled:hover:bg-white/12";
const GLASS: Record<Type, string> = {
  default: `${capsule} ${HOVER} ${fg} font-medium`,
  primary: `${capsule} ${HOVER} ${accentText} font-semibold`,
  error: `${capsule} ${HOVER} ${dangerText} font-medium`,
};

const GHOST_HOVER = "border-transparent bg-transparent enabled:hover:bg-zinc-900/6 dark:enabled:hover:bg-white/10";
const GHOST: Record<Type, string> = {
  default: `${GHOST_HOVER} ${fg} font-medium`,
  primary: `${GHOST_HOVER} ${accentText} font-medium`,
  error: `${GHOST_HOVER} ${dangerText} font-medium`,
};

const SELECTED = `border-transparent ${raisedChip} ${accentText} font-medium`;

const look = computed(() => (props.selected ? SELECTED : (props.ghost ? GHOST : GLASS)[props.type]));

function onClick(event: MouseEvent) {
  if (props.loading || props.disabled) {
    event.preventDefault();
    event.stopPropagation();
  }
}
</script>

<template>
  <button
    :class="[
      'm-0 inline-flex cursor-pointer items-center justify-center gap-1.5 border leading-none whitespace-nowrap select-none',
      'transition duration-150 ease-out enabled:active:scale-97 disabled:cursor-not-allowed disabled:opacity-40',
      focusRing,
      'rounded-full',
      circle ? ['shrink-0 p-0', CIRCLE[size]] : SIZE[size],
      look,
      loading && 'pointer-events-none',
    ]"
    :disabled="disabled || undefined"
    :aria-busy="loading || undefined"
    @click="onClick"
  >
    <Loader2 v-if="loading" class="size-4 shrink-0 animate-spin" />
    <slot v-else-if="$slots.icon" name="icon" />
    <span v-if="$slots.default" class="min-w-0 truncate"><slot /></span>
  </button>
</template>
