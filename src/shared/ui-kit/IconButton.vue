<script setup lang="ts">
// 纯图标按钮：透明底的圆形 UiButton。
import UiButton from "@/shared/ui-kit/UiButton.vue";

withDefaults(
  defineProps<{
    size?: "tiny" | "small" | "medium";
    type?: "default" | "primary" | "error";
    /** 开关处于打开状态。 */
    selected?: boolean;
  }>(),
  { size: "small", type: "default" },
);
</script>

<template>
  <UiButton ghost circle :size="size" :type="type" :selected="selected">
    <template #icon><slot /></template>
  </UiButton>
</template>
