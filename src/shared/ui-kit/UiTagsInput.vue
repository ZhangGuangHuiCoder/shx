<script setup lang="ts">
// 标签输入：回车或逗号添加，Backspace 删除末项。
import { TagsInputInput, TagsInputItem, TagsInputItemDelete, TagsInputItemText, TagsInputRoot } from "reka-ui";
import { X } from "@lucide/vue";
import { computed } from "vue";
import { field, fieldInput, hoverFill } from "@/shared/ui-kit/styles";

const props = withDefaults(defineProps<{ value?: string[]; placeholder?: string; disabled?: boolean }>(), {
  value: () => [],
  placeholder: "",
});
const emit = defineEmits<{ "update:value": [value: string[]] }>();

const model = computed<string[]>({
  get: () => props.value ?? [],
  set: (v) => emit("update:value", v),
});
</script>

<template>
  <TagsInputRoot
    v-model="model"
    :disabled="disabled"
    delimiter=","
    add-on-paste
    :class="[field, 'min-h-9 cursor-text flex-wrap items-center rounded-xl px-1.5! py-1! text-sm gap-1!', disabled && 'cursor-not-allowed opacity-50']"
  >
    <TagsInputItem
      v-for="(tag, i) in model"
      :key="`${tag}-${i}`"
      :value="tag"
      :index="i"
      class="inline-flex h-6 items-center gap-1 rounded-md border border-zinc-900/10 bg-white/80 pr-1 pl-2 text-xs leading-none text-zinc-700 shadow-xs select-none data-[state=active]:border-blue-500 dark:border-white/10 dark:bg-white/10 dark:text-zinc-200"
    >
      <TagsInputItemText>{{ tag }}</TagsInputItemText>
      <TagsInputItemDelete
        :class="[hoverFill, 'grid size-4 cursor-pointer place-items-center rounded-sm text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200']"
        :aria-label="`Remove ${tag}`"
      >
        <X :size="11" />
      </TagsInputItemDelete>
    </TagsInputItem>
    <TagsInputInput
      :class="[fieldInput, 'h-6 w-16 min-w-16 flex-1']"
      :placeholder="model.length ? '' : placeholder"
    />
  </TagsInputRoot>
</template>
