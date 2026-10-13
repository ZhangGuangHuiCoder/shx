<script setup lang="ts">
// 只有图标的按钮。样式全部来自 Naive 的按钮（quaternary + circle），底色、悬停和禁用的样子都跟着主题走；
// `disabled`、`title`、点击事件等照常写，会原样交给 NButton。
import { NButton } from "naive-ui";

withDefaults(
  defineProps<{
    size?: "tiny" | "small" | "medium";
    /** 有含义的按钮才给颜色（开关打开了、会删除东西……）；不给的是次要文字的颜色，不和内容抢眼。 */
    type?: "default" | "primary" | "info" | "success" | "warning" | "error";
  }>(),
  { size: "small", type: "default" },
);
</script>

<template>
  <NButton quaternary circle :size="size" :type="type === 'default' ? 'tertiary' : type">
    <template #icon><slot /></template>
  </NButton>
</template>
