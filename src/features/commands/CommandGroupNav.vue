<script setup lang="ts">
// 命令助手的左侧：分组列表，选中的分组显示在右侧。
import type { Component } from "vue";

export interface NavGroup {
  key: string;
  title: string;
  count: number;
  icon: Component;
}

defineProps<{ groups: NavGroup[] }>();
/** 当前显示的分组的键。 */
const selected = defineModel<string>({ required: true });
</script>

<template>
  <nav class="groups">
    <button
      v-for="group in groups"
      :key="group.key"
      type="button"
      class="group"
      :class="{ on: group.key === selected }"
      :title="group.title"
      @click="selected = group.key"
    >
      <component :is="group.icon" :size="15" :stroke-width="1.9" class="icon" />
      <span class="ellipsis title">{{ group.title }}</span>
      <span class="count">{{ group.count }}</span>
    </button>
  </nav>
</template>

<style scoped>
.groups {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 208px;
  flex-shrink: 0;
  padding: 10px 8px;
  border-right: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-raised));
  overflow-y: auto;
}

.group {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 32px;
  flex-shrink: 0;
  padding: 0 8px 0 10px;
  border: 0;
  border-radius: 8px;
  background: none;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: background-color 0.12s;
}

.group:hover {
  background: var(--fill);
}

.icon {
  flex-shrink: 0;
  color: var(--text-dim);
}

.title {
  flex: 1;
}

.count {
  flex-shrink: 0;
  min-width: 22px;
  padding: 1px 6px;
  border-radius: 9px;
  color: var(--text-dim);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  text-align: center;
}

.group.on {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}

.group.on .icon {
  color: var(--accent);
}

.group.on .count {
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 500;
}

/* 第一项是“全部命令”，和具体的分组隔开一点。 */
.group:first-child {
  margin-bottom: 7px;
}
</style>
