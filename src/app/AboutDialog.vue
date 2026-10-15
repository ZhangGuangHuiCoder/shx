<script setup lang="ts">
import { CircleArrowUp, CodeXml, Scale, ScrollText } from "@lucide/vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onMounted, ref, type Component } from "vue";
import BrandSlogan from "@/shared/ui-kit/BrandSlogan.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import { t } from "@/shared/i18n";
import { dim, fg, line } from "@/shared/ui-kit/styles";

defineProps<{ version: string }>();
/** `update` 表示去检查更新，关闭时为 null。 */
const emit = defineEmits<{ done: [next: "update" | null] }>();

const REPO = "https://github.com/ZhangGuangHuiCoder/shx";
const LINKS: { icon: Component; label: () => string; url: string }[] = [
  { icon: CodeXml, label: () => t("about.source"), url: REPO },
  { icon: ScrollText, label: () => t("about.releases"), url: `${REPO}/releases` },
  { icon: Scale, label: () => "Apache-2.0", url: `${REPO}/blob/main/LICENSE` },
];

const body = ref<HTMLElement>();
// 对话框的焦点锁定会在此之后聚焦第一个按钮，延后一拍再设焦点。
onMounted(() => setTimeout(() => body.value?.querySelector<HTMLElement>(".close")?.focus()));
</script>

<template>
  <div ref="body" class="text-center">
    <!-- 标识文字是浅色，此区域始终深色。 -->
    <div class="relative isolate flex flex-col items-center gap-3 overflow-hidden bg-linear-135 from-slate-800 to-slate-950 px-6 pt-9 pb-7">
      <div class="absolute -top-32 left-1/2 -z-10 h-64 w-112 -translate-x-1/2 bg-radial from-indigo-500/50 to-transparent to-70%" aria-hidden="true" />
      <BrandSlogan class="h-13" dark />
      <div class="text-xs tracking-wider text-slate-400">Explore Beyond the Shell.</div>
    </div>

    <div class="flex flex-col items-center px-6 py-5">
      <div :class="[fg, 'text-sm']">{{ t("about.tagline") }}</div>
      <div :class="[dim, 'mt-3 rounded-full bg-zinc-900/5 px-3 py-1 text-xs tabular-nums select-text dark:bg-white/10']">{{ t("about.version", { version }) }}</div>

      <div :class="[line, 'mt-5 flex w-full flex-wrap justify-center gap-1 border-t pt-4']">
        <UiButton v-for="link in LINKS" :key="link.url" ghost size="tiny" :title="link.url" @click="openUrl(link.url)">
          <template #icon><component :is="link.icon" :size="13" /></template>
          {{ link.label() }}
        </UiButton>
      </div>

      <footer class="mt-4 flex justify-center gap-2">
        <UiButton class="min-w-24" @click="emit('done', 'update')">
          <template #icon><CircleArrowUp :size="15" /></template>
          {{ t("menu.checkUpdate") }}
        </UiButton>
        <UiButton class="close min-w-24" type="primary" @click="emit('done', null)">{{ t("common.close") }}</UiButton>
      </footer>
    </div>
  </div>
</template>
