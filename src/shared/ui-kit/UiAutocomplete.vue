<script setup lang="ts">
// 可补全文本框：自由输入，聚焦展开建议，点选替换文本。
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxItemIndicator,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxViewport,
} from "reka-ui";
import { Check, X } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { field, fieldMd, fieldIcon, fieldInput, popCheck, popItem, popLayer, popPanel } from "@/shared/ui-kit/styles";
import PopPanel from "@/shared/ui-kit/PopPanel.vue";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

type Option = string | { label: string; value: string };

const props = withDefaults(
  defineProps<{
    value?: string | null;
    options?: Option[];
    placeholder?: string;
    clearable?: boolean;
    disabled?: boolean;
  }>(),
  { options: () => [] },
);
const emit = defineEmits<{ "update:value": [value: string] }>();

const text = ref(props.value ?? "");
// 须与文本同步，否则 ComboboxInput 打开时会把已有值清空。
const selected = ref<string | undefined>(props.value || undefined);
const open = ref(false);

watch(
  () => props.value,
  (v) => {
    text.value = v ?? "";
    selected.value = v || undefined;
  },
);

const normalized = computed(() =>
  props.options.map((o) => (typeof o === "string" ? { label: o, value: o } : o)),
);

function onText(v: string) {
  text.value = v;
  emit("update:value", v);
}

function onSelect(v: string | undefined) {
  // 再次点选已选项时 reka 会返回 undefined，保留文本。
  if (v == null) {
    selected.value = text.value || undefined;
    return;
  }
  selected.value = v;
  text.value = v;
  emit("update:value", v);
}

function clear() {
  text.value = "";
  selected.value = undefined;
  emit("update:value", "");
}
</script>

<template>
  <ComboboxRoot
    v-model="selected"
    v-model:open="open"
    :disabled="disabled"
    open-on-focus
    open-on-click
    :reset-search-term-on-blur="false"
    @update:model-value="(v: string | undefined) => onSelect(v)"
  >
    <ComboboxAnchor as-child>
      <div :class="[field, fieldMd, disabled && 'cursor-not-allowed opacity-50']">
        <ComboboxInput
          :model-value="text"
          :placeholder="placeholder"
          :disabled="disabled"
          :class="fieldInput"
          @update:model-value="(v: string) => onText(v)"
        />
        <button
          v-if="clearable && text && !disabled"
          type="button"
          :class="fieldIcon"
          tabindex="-1"
          aria-label="Clear"
          @mousedown.prevent
          @click="clear"
        >
          <X :size="13" />
        </button>
      </div>
    </ComboboxAnchor>
    <ComboboxPortal v-if="normalized.length">
      <PopTransition>
        <ComboboxContent :class="[popLayer, 'min-w-(--reka-combobox-trigger-width)']" position="popper" :side-offset="6">
          <PopPanel :class="popPanel">
            <ComboboxViewport class="min-h-0">
              <ComboboxItem v-for="opt in normalized" :key="opt.value" :value="opt.value" :class="popItem">
                <ComboboxItemIndicator :class="popCheck">
                  <Check :size="14" :stroke-width="2.5" />
                </ComboboxItemIndicator>
                <span class="min-w-0 flex-1 truncate">{{ opt.label }}</span>
              </ComboboxItem>
            </ComboboxViewport>
          </PopPanel>
        </ComboboxContent>
      </PopTransition>
    </ComboboxPortal>
  </ComboboxRoot>
</template>
