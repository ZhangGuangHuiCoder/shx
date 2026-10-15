<script setup lang="ts">
import {
  SelectContent,
  SelectIcon,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from "reka-ui";
import { Check, ChevronDown } from "@lucide/vue";
import { computed, useAttrs } from "vue";
import { t } from "@/shared/i18n";
import { dim, field, fieldMd, popCheck, popEmpty, popItem, popLayer, popPanel } from "@/shared/ui-kit/styles";
import PopPanel from "@/shared/ui-kit/PopPanel.vue";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

defineOptions({ inheritAttrs: false });

export type UiSelectOption = { label: string; value: string | number; disabled?: boolean };

const props = withDefaults(
  defineProps<{
    value?: string | number | null;
    options?: UiSelectOption[];
    placeholder?: string;
    /** 空列表提示。 */
    emptyText?: string;
    disabled?: boolean;
  }>(),
  { options: () => [] },
);
const emit = defineEmits<{ "update:value": [value: string | number] }>();
const attrs = useAttrs();

const model = computed<string | number | undefined>({
  get: () => (props.value === null || props.value === undefined ? undefined : props.value),
  set: (v) => {
    if (v !== undefined) emit("update:value", v);
  },
});
</script>

<template>
  <SelectRoot v-model="model" :disabled="disabled">
    <SelectTrigger
      :class="[field, fieldMd, 'group cursor-pointer appearance-none justify-between text-left outline-none focus-visible:border-blue-500 focus-visible:ring-3 focus-visible:ring-blue-500/20 disabled:cursor-not-allowed disabled:opacity-50', (attrs.class as string) ?? '']"
      :disabled="disabled"
    >
      <SelectValue
        class="min-w-0 flex-1 truncate text-zinc-800 data-placeholder:text-zinc-400 dark:text-zinc-100 dark:data-placeholder:text-zinc-500"
        :placeholder="placeholder"
      />
      <SelectIcon :class="[dim, 'grid shrink-0 place-items-center transition-transform duration-200 ease-out group-data-[state=open]:rotate-180']">
        <ChevronDown :size="14" />
      </SelectIcon>
    </SelectTrigger>
    <SelectPortal>
      <PopTransition>
        <SelectContent :class="[popLayer, 'min-w-(--reka-select-trigger-width)']" position="popper" :side-offset="6">
          <PopPanel :class="popPanel">
            <SelectViewport class="min-h-0">
              <div v-if="!options.length" :class="popEmpty">{{ emptyText ?? t("common.noOptions") }}</div>
              <SelectItem v-for="opt in options" :key="opt.value" :value="opt.value" :disabled="opt.disabled" :class="popItem">
                <SelectItemIndicator :class="popCheck">
                  <Check :size="14" :stroke-width="2.5" />
                </SelectItemIndicator>
                <SelectItemText class="min-w-0 flex-1 truncate">{{ opt.label }}</SelectItemText>
              </SelectItem>
            </SelectViewport>
          </PopPanel>
        </SelectContent>
      </PopTransition>
    </SelectPortal>
  </SelectRoot>
</template>
