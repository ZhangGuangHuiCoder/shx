<script setup lang="ts">
// 复选框，v-model 名为 checked。
import { CheckboxIndicator, CheckboxRoot } from "reka-ui";
import { Check } from "@lucide/vue";

withDefaults(
  defineProps<{
    checked?: boolean;
    disabled?: boolean;
  }>(),
  { checked: false },
);
const emit = defineEmits<{ "update:checked": [value: boolean] }>();
</script>

<template>
    <label class="group inline-flex cursor-pointer items-center gap-2 select-none" :class="disabled && 'cursor-not-allowed opacity-50'">
    <CheckboxRoot
      :model-value="checked"
      :disabled="disabled"
      class="inline-grid size-4 shrink-0 place-items-center rounded-sm border border-zinc-900/20 bg-white/70 text-white shadow-xs transition duration-150 ease-out outline-none hover:border-blue-500 focus-visible:ring-3 focus-visible:ring-blue-500/30 data-disabled:hover:border-zinc-900/20 data-[state=checked]:border-blue-500 data-[state=checked]:bg-blue-500 dark:border-white/20 dark:bg-white/5 dark:data-[state=checked]:border-blue-500 dark:data-[state=checked]:bg-blue-500"
      @update:model-value="(v: boolean | 'indeterminate') => emit('update:checked', v === true)"
    >
      <CheckboxIndicator class="grid place-items-center size-full">
        <Check :size="12" :stroke-width="3" />
      </CheckboxIndicator>
    </CheckboxRoot>
    <span v-if="$slots.default" class="leading-none"><slot /></span>
  </label>
</template>
