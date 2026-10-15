<script lang="ts">
export default { name: "AppMenuContent" };
</script>

<script setup lang="ts">
// 标题栏菜单的递归内容。
import {
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "reka-ui";
import { Check, ChevronRight } from "@lucide/vue";
import type { MenuNode } from "@/app/menu";
import { formatAccelerator } from "@/app/shortcuts";
import { menuDivider, menuItem, menuPanel, popLayer } from "@/shared/ui-kit/styles";
import PopPanel from "@/shared/ui-kit/PopPanel.vue";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

/** 子菜单展开时其触发项保持浅底。 */
const itemClass = [menuItem, "not-data-highlighted:data-[state=open]:bg-zinc-900/5 dark:not-data-highlighted:data-[state=open]:bg-white/10"];

defineProps<{ items: MenuNode[]; path: string }>();
const emit = defineEmits<{ select: [key: string] }>();
</script>

<template>
  <template v-for="(node, index) in items" :key="`${path}.${index}`">
    <DropdownMenuSeparator v-if="node.type === 'separator'" :class="menuDivider" />
    <DropdownMenuSub v-else-if="node.type === 'submenu'">
      <DropdownMenuSubTrigger :class="itemClass" :disabled="node.enabled?.() === false">
        <span class="flex min-w-0 flex-1 items-center justify-between gap-7">
          <span class="truncate">{{ node.label }}</span>
          <ChevronRight :size="13" class="shrink-0 opacity-60" />
        </span>
      </DropdownMenuSubTrigger>
      <DropdownMenuPortal>
        <PopTransition>
          <DropdownMenuSubContent :class="popLayer" :side-offset="4" :align-offset="-4">
            <PopPanel :class="menuPanel">
              <AppMenuContent :items="node.items" :path="`${path}.${index}`" @select="emit('select', $event)" />
            </PopPanel>
          </DropdownMenuSubContent>
          </PopTransition>
      </DropdownMenuPortal>
    </DropdownMenuSub>
    <DropdownMenuItem v-else-if="node.type === 'item' || node.type === 'check'" :class="itemClass" @select="emit('select', `${path}.${index}`)">
      <Check v-if="node.type === 'check' && node.checked()" :size="15" :stroke-width="1.8" class="shrink-0" />
      <span v-else-if="node.type === 'check'" class="size-4 shrink-0" />
      <span class="flex min-w-0 flex-1 items-center justify-between gap-7">
        <span class="truncate">{{ node.label }}</span>
        <span v-if="node.type === 'item' && node.accelerator" class="min-w-0 truncate text-xs tracking-wide opacity-60">{{ formatAccelerator(node.accelerator) }}</span>
      </span>
    </DropdownMenuItem>
  </template>
</template>
