<script setup lang="ts">
// 数字输入（无步进按钮）。空值为 null。
import { NumberFieldInput, NumberFieldRoot } from "reka-ui";
import { computed } from "vue";
import { field, fieldMd, fieldInput } from "@/shared/ui-kit/styles";

const props = withDefaults(
  defineProps<{
    value?: number | null;
    min?: number;
    max?: number;
    placeholder?: string;
    disabled?: boolean;
  }>(),
  { value: null },
);
const emit = defineEmits<{ "update:value": [value: number | null] }>();

const model = computed<number | undefined>({
  get: () => (props.value === null || props.value === undefined ? undefined : props.value),
  set: (v) => emit("update:value", v === undefined ? null : v),
});
</script>

<template>
  <NumberFieldRoot
    v-model="model"
    :min="min"
    :max="max"
    :disabled="disabled"
    :class="[field, fieldMd, disabled && 'cursor-not-allowed opacity-50']"
  >
    <NumberFieldInput :class="[fieldInput, 'tabular-nums']" :placeholder="placeholder" />
  </NumberFieldRoot>
</template>
