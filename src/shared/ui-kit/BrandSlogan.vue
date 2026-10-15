<script setup lang="ts">
// 品牌标识，按主题选择深/浅色版本。内联 SVG：文字用系统字体，各平台宽度不一，内联可避免被裁剪。
import { computed } from "vue";
import onDark from "@/assets/slogan-dark.svg?raw";
import onLight from "@/assets/slogan-light.svg?raw";
import { isDark } from "@/shared/theme";

const props = defineProps<{ /** 固定用于深色底。 */ dark?: boolean }>();
const svg = computed(() => (props.dark || isDark.value ? onDark : onLight));
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- 内容为仓库内的静态图片。 -->
  <span class="inline-flex shrink-0 *:block *:h-full *:w-auto *:overflow-visible" role="img" aria-label="Shell Xplore" v-html="svg" />
</template>

