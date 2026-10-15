<script setup lang="ts">
// 命令列表项：高亮搜索命中和占位符。
import { computed } from "vue";
import { commandParts, markHits } from "@/features/commands/commands";
import { dim } from "@/shared/ui-kit/styles";

const hit = "rounded-sm bg-amber-300/50 text-inherit dark:bg-amber-400/30";

const props = withDefaults(defineProps<{ name: string; command: string; /** 小写搜索词。 */ words?: string[] }>(), { words: () => [] });
const nameParts = computed(() => markHits(props.name, props.words));
const parts = computed(() => commandParts(props.command, props.words));
</script>

<template>
  <div class="min-w-0 flex-1">
    <div class="truncate leading-5 font-medium">
      <template v-for="(part, i) in nameParts" :key="i"><mark v-if="part.hit" :class="hit">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template>
    </div>
    <div :class="[dim, 'mt-1 truncate font-mono text-xs/4']"><span class="opacity-55">$ </span>
      <template v-for="(part, i) in parts" :key="i"
        ><span v-if="part.placeholder" class="rounded bg-blue-500/12 px-1 py-px text-blue-600 dark:bg-blue-400/15 dark:text-blue-300">{{ part.text }}</span
        ><mark v-else-if="part.hit" :class="hit">{{ part.text }}</mark
        ><template v-else>{{ part.text }}</template></template
      >
    </div>
  </div>
</template>
