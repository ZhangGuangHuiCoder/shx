<script setup lang="ts">
import { Folder, FolderOpen } from "@lucide/vue";
import { computed } from "vue";
import { fileIcon, type FileIconTarget } from "../fileIcons";

/** 根据条目类型和名称得出的文件 / 文件夹图标，所有文件视图共用。 */
const props = withDefaults(
  defineProps<FileIconTarget & { size?: number; text?: boolean; /** 把普通文件夹画成打开的样子。 */ open?: boolean }>(),
  { size: 15, isDir: false, kind: "file", text: false, open: false },
);

const info = computed(() => fileIcon(props, { text: props.text }));
const component = computed(() => (props.open && info.value.component === Folder ? FolderOpen : info.value.component));
</script>

<template>
  <component :is="component" :size="size" :style="{ color: info.color }" />
</template>
