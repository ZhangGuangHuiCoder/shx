<script setup lang="ts">
import type { CommandGroup } from "@/features/commands/commands";
import { groupIcon } from "@/features/commands/groupIcons";
import { sectionLabel } from "@/shared/ui-kit/styles";

defineProps<{ group: CommandGroup }>();
</script>

<template>
  <h4 :class="[sectionLabel, 'm-0 flex h-10 w-full items-center gap-2 px-3 pt-3 pb-2 after:ml-1 after:h-px after:flex-1 after:bg-zinc-900/10 dark:after:bg-white/10']">
    <component :is="groupIcon(group.key)" :size="13" :stroke-width="2" />
    {{ group.title }}
    <span class="font-normal tabular-nums opacity-75">{{ group.items.length }}</span>
  </h4>
</template>
