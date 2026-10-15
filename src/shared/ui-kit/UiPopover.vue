<script setup lang="ts">
// 点击弹出的气泡浮层。
import { PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { computed, useAttrs } from "vue";
import { glassPop, popLayer } from "@/shared/ui-kit/styles";
import PopPanel from "@/shared/ui-kit/PopPanel.vue";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    show?: boolean;
    /** 形如 bottom-end。 */
    placement?: string;
  }>(),
  { show: false, placement: "bottom-start" },
);
const emit = defineEmits<{ "update:show": [open: boolean] }>();
const attrs = useAttrs();

const [side, alignRaw] = props.placement.split("-");
const align = computed(() => (alignRaw === "start" || alignRaw === "end" ? alignRaw : "center"));
</script>

<template>
  <PopoverRoot :open="show" @update:open="(v: boolean) => emit('update:show', v)">
    <PopoverTrigger as-child>
      <slot name="trigger" />
    </PopoverTrigger>
    <PopoverPortal>
      <PopTransition>
        <PopoverContent
          :class="popLayer"
          :side="side as 'top' | 'right' | 'bottom' | 'left'"
          :align="align as 'start' | 'center' | 'end'"
          :side-offset="6"
          :align-offset="0"
        >
          <PopPanel :class="[glassPop, 'rounded-2xl p-3 text-zinc-800 dark:text-zinc-100', (attrs.class as string) ?? '']">
            <slot />
          </PopPanel>
        </PopoverContent>
      </PopTransition>
    </PopoverPortal>
  </PopoverRoot>
</template>
