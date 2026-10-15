<script setup lang="ts">
import type { Component } from "vue";
import { line, well } from "@/shared/ui-kit/styles";

export interface NavGroup {
  key: string;
  title: string;
  count: number;
  icon: Component;
}

defineProps<{ groups: NavGroup[] }>();
const selected = defineModel<string>({ required: true });
</script>

<template>
  <nav :class="[line, well, 'flex w-52 shrink-0 flex-col gap-1 overflow-y-auto border-r px-2 py-3']">
    <button
      v-for="group in groups"
      :key="group.key"
      type="button"
      class="flex h-8 shrink-0 cursor-pointer items-center gap-2 rounded-lg pr-2 pl-3 text-left text-sm transition-colors duration-150 first:mb-2"
      :class="group.key === selected ? 'bg-blue-500/12 font-semibold text-blue-600 dark:bg-blue-400/15 dark:text-blue-300' : 'bg-transparent text-zinc-700 hover:bg-zinc-900/5 dark:text-zinc-200 dark:hover:bg-white/10'"
      :title="group.title"
      @click="selected = group.key"
    >
      <component :is="group.icon" :size="15" :stroke-width="1.9" class="shrink-0" :class="group.key === selected ? 'text-blue-500' : 'text-zinc-400 dark:text-zinc-500'" />
      <span class="flex-1 truncate">{{ group.title }}</span>
      <span class="min-w-6 shrink-0 rounded-full px-2 py-px text-center text-xs tabular-nums" :class="group.key === selected ? 'bg-blue-500/15 font-medium text-blue-600 dark:text-blue-300' : 'text-zinc-400 dark:text-zinc-500'">{{ group.count }}</span>
    </button>
  </nav>
</template>
