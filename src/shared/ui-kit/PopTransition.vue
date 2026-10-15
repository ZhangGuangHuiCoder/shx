<script setup lang="ts">
// 浮层过渡。不用 @starting-style：旧版 macOS WebKit 不支持。
// popper：reka 浮层，只做退场淡出（入场动画在 PopPanel 上）；menu：右键菜单；zoom：大面板。
withDefaults(defineProps<{ appear?: boolean; kind?: "popper" | "menu" | "zoom" }>(), { kind: "popper" });

const ENTER = {
  popper: { active: "", from: "" },
  menu: { active: "origin-top-left transition duration-200 ease-out motion-reduce:transition-none", from: "translate-y-1 scale-95 opacity-0" },
  zoom: { active: "transition duration-200 ease-out motion-reduce:transition-none", from: "scale-95 opacity-0" },
} as const;
</script>

<template>
  <Transition
    :appear="appear"
    :enter-active-class="ENTER[kind].active"
    :enter-from-class="ENTER[kind].from"
    leave-active-class="transition-opacity duration-100 ease-in motion-reduce:transition-none"
    leave-to-class="opacity-0"
  >
    <slot />
  </Transition>
</template>
