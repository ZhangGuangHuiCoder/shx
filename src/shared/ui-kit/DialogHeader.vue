<script setup lang="ts">
import type { Component } from "vue";
import { dim } from "@/shared/ui-kit/styles";

withDefaults(defineProps<{ icon?: Component; title: string; detail?: string; tone?: "accent" | "danger" }>(), {
  icon: undefined,
  detail: "",
  tone: "accent",
});
</script>

<template>
  <div class="flex min-w-0 items-center gap-3">
    <div
      class="grid size-9 shrink-0 place-items-center rounded-xl inset-shadow-xs"
      :class="tone === 'danger' ? 'bg-red-500/15 text-red-500 inset-shadow-red-500/20' : 'bg-blue-500/15 text-blue-500 inset-shadow-blue-500/20'"
    >
      <slot name="icon"><component :is="icon" :size="18" :stroke-width="1.9" /></slot>
    </div>
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-baseline gap-2.5">
        <div class="shrink-0 text-base/5 font-semibold">{{ title }}</div>
        <slot name="tagline" />
      </div>
      <div v-if="detail" :class="[dim, 'mt-px truncate text-xs/4 font-normal']" :title="detail">{{ detail }}</div>
    </div>
    <slot />
  </div>
</template>
