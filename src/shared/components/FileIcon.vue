<script setup lang="ts">
import { Folder, FolderOpen } from "@lucide/vue";
import { computed } from "vue";
import { fileIcon, type FileIconTarget } from "../fileIcons";
import { isDark } from "../theme";

/** 根据条目类型和名称得出的文件 / 文件夹图标，所有文件视图共用。 */
const props = withDefaults(
  defineProps<FileIconTarget & { size?: number; text?: boolean; /** 把普通文件夹画成打开的样子。 */ open?: boolean }>(),
  { size: 15, isDir: false, kind: "file", text: false, open: false },
);

const info = computed(() => fileIcon(props, { text: props.text }));
const component = computed(() => (props.open && info.value.component === Folder ? FolderOpen : info.value.component));
// 配色按深色背景调校；在白底上稍微加深。
const style = computed(() => ({
  color: info.value.color,
  filter: isDark.value ? undefined : "brightness(0.85) saturate(1.05)",
}));
</script>

<template>
  <component :is="component" :size="size" :style="style" />
</template>
