<script setup lang="ts">
import { Folder, FolderOpen } from "@lucide/vue";
import { computed } from "vue";
import { fileIcon, type FileIconTarget } from "@/shared/fileIcons";

const props = withDefaults(
  defineProps<FileIconTarget & { size?: number; text?: boolean; /** 显示为打开的文件夹。 */ open?: boolean }>(),
  { size: 15, isDir: false, kind: "file", text: false, open: false },
);

const info = computed(() => fileIcon(props, { text: props.text }));
const component = computed(() => (props.open && info.value.component === Folder ? FolderOpen : info.value.component));
</script>

<template>
  <component :is="component" :size="size" :class="info.color" />
</template>
