<script setup lang="ts">
// 应用所有对话框共用的头部：带底色的图标、标题，以及标题下方的一行说明。
import type { Component } from "vue";

withDefaults(defineProps<{ /** `icon` 插槽自己绘制图标时不需要。 */ icon?: Component; title: string; detail?: string; tone?: "accent" | "danger" }>(), {
  icon: undefined,
  detail: "",
  tone: "accent",
});
</script>

<template>
  <div class="dialog-header">
    <div class="tile" :class="tone">
      <slot name="icon"><component :is="icon" :size="18" :stroke-width="1.9" /></slot>
    </div>
    <div class="titles">
      <div class="title">{{ title }}</div>
      <div v-if="detail" class="detail" :title="detail">{{ detail }}</div>
    </div>
    <slot />
  </div>
</template>

<style scoped>
.dialog-header {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.tile {
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--accent-soft);
  color: var(--accent);
}

.tile.danger {
  background: var(--danger-soft);
  color: var(--danger);
}

.titles {
  flex: 1;
  min-width: 0;
}

.title {
  font-size: 15px;
  font-weight: 600;
  line-height: 20px;
}

.detail {
  margin-top: 1px;
  overflow: hidden;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 400;
  line-height: 16px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
