<script setup lang="ts">
// 品牌标识（图标加 Shell × Files.），应用里凡是要显示它的地方都用这个组件。
// 图有两张：slogan-dark.svg 是浅色字，配深色底；slogan-light.svg 是深色字，配浅色底。
// 默认跟着应用的主题选；底色固定是深色的地方（“关于”的顶部）传 `dark`。
//
// 内联而不是当图片用：字是用系统字体画的，各平台宽度略有出入，内联以后不裁剪，宽一点也不会缺一块。
import { computed } from "vue";
import onDark from "../../assets/slogan-dark.svg?raw";
import onLight from "../../assets/slogan-light.svg?raw";
import { isDark } from "../theme";

const props = defineProps<{ /** 画在固定的深色底上，不跟主题走。 */ dark?: boolean }>();
const svg = computed(() => (props.dark || isDark.value ? onDark : onLight));
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- 内容是仓库里的两张图，不是外来的。 -->
  <span class="brand-slogan" role="img" aria-label="shx — Shell × Files." v-html="svg" />
</template>

<style scoped>
/* 高度由使用它的地方定，宽度按比例走。 */
.brand-slogan {
  display: inline-flex;
  flex-shrink: 0;
}

.brand-slogan :deep(svg) {
  display: block;
  width: auto;
  height: 100%;
  overflow: visible;
}
</style>
