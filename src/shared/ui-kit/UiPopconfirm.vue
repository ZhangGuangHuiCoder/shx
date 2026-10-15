<script setup lang="ts">
// 非模态的气泡确认。
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiPopover from "@/shared/ui-kit/UiPopover.vue";
import { ref } from "vue";

withDefaults(
  defineProps<{
    positiveText?: string;
    negativeText?: string;
    /** 禁用时不弹确认，触发元素的点击照常生效。 */
    disabled?: boolean;
    placement?: string;
  }>(),
  { positiveText: "OK", negativeText: "Cancel", disabled: false, placement: "bottom" },
);
const emit = defineEmits<{ "positive-click": []; "negative-click": [] }>();

const open = ref(false);

function positive() {
  open.value = false;
  emit("positive-click");
}
function negative() {
  open.value = false;
  emit("negative-click");
}
</script>

<template>
  <slot v-if="disabled" name="trigger" />
  <UiPopover v-else v-model:show="open" :placement="placement">
    <template #trigger>
      <slot name="trigger" />
    </template>
    <div class="flex min-w-48 flex-col gap-3">
      <div class="text-sm/relaxed"><slot /></div>
      <div class="flex justify-end gap-2">
        <UiButton size="small" @click="negative">{{ negativeText }}</UiButton>
        <UiButton size="small" type="error" @click="positive">{{ positiveText }}</UiButton>
      </div>
    </div>
  </UiPopover>
</template>
