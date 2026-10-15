<script setup lang="ts">
// 浮层面板及其入场动画。
// 动画不能放在 reka 的定位层上：定位层用内联 transform 定位，加 transition 会让面板从左上角飘过来。
// 方向和缩放原点由 reka 的 data-side 决定；退场淡出由外层 PopTransition 负责。
import { useAttrs } from "vue";

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <Transition
    appear
    enter-active-class="transition duration-200 ease-out motion-reduce:transition-none"
    enter-from-class="scale-95 opacity-0 in-data-[side=bottom]:-translate-y-1 in-data-[side=top]:translate-y-1 in-data-[side=left]:translate-x-1 in-data-[side=right]:-translate-x-1"
  >
    <div
      :class="[
        'in-data-[side=bottom]:origin-top in-data-[side=top]:origin-bottom in-data-[side=left]:origin-right in-data-[side=right]:origin-left',
        (attrs.class as string) ?? '',
      ]"
    >
      <slot />
    </div>
  </Transition>
</template>
