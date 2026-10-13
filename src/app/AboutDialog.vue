<script setup lang="ts">
// “关于 shx”：上面一块深色的品牌区，放品牌标识（图标加 Shell × Files.）和
// One Window. Total Control.；下面是一句介绍、版本、几个去处和检查更新。由 about.ts 打开。
import { CircleArrowUp, CodeXml, Scale, ScrollText } from "@lucide/vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { NButton } from "naive-ui";
import { onMounted, ref, type Component } from "vue";
import BrandSlogan from "../shared/components/BrandSlogan.vue";
import { t } from "../shared/i18n";

defineProps<{ version: string }>();
/** 交回 `update` 表示要去检查更新；关闭时是 null。 */
const emit = defineEmits<{ done: [next: "update" | null] }>();

const REPO = "https://github.com/ZhangGuangHuiCoder/shx";
const LINKS: { icon: Component; label: () => string; url: string }[] = [
  { icon: CodeXml, label: () => t("about.source"), url: REPO },
  { icon: ScrollText, label: () => t("about.releases"), url: `${REPO}/releases` },
  { icon: Scale, label: () => "Apache-2.0", url: `${REPO}/blob/main/LICENSE` },
];

const body = ref<HTMLElement>();
// 外层对话框的焦点锁定在挂载时会把焦点放到第一个按钮上，而且发生在这里之后；等它做完再放。
onMounted(() => setTimeout(() => body.value?.querySelector<HTMLElement>(".close")?.focus()));
</script>

<template>
  <div ref="body" class="about">
    <!-- 标识里的字是浅色的，所以这一块不跟主题走，始终是深色。 -->
    <div class="hero">
      <BrandSlogan class="slogan" dark />
      <div class="sub">One Window. Total Control.</div>
    </div>

    <div class="details">
      <div class="tagline">{{ t("about.tagline") }}</div>
      <div class="version">shx · {{ t("about.version", { version }) }}</div>

      <div class="links">
        <NButton v-for="link in LINKS" :key="link.url" quaternary type="tertiary" size="tiny" :title="link.url" @click="openUrl(link.url)">
          <template #icon><component :is="link.icon" :size="13" /></template>
          {{ link.label() }}
        </NButton>
      </div>

      <footer>
        <NButton @click="emit('done', 'update')">
          <template #icon><CircleArrowUp :size="15" /></template>
          {{ t("menu.checkUpdate") }}
        </NButton>
        <NButton class="close" type="primary" @click="emit('done', null)">{{ t("common.close") }}</NButton>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.about {
  text-align: center;
}

/* 配色取自图标本身的底色，再加一抹和 × 同色的光。 */
.hero {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 34px 24px 26px;
  border-radius: 13px 13px 0 0;
  background:
    radial-gradient(260px 150px at 50% 0, rgb(83 120 255 / 30%), transparent 72%),
    linear-gradient(135deg, #242b3f, #0b1020);
}

.slogan {
  height: 50px;
}

.sub {
  color: #9aa9c2;
  font-size: 12.5px;
  letter-spacing: 0.6px;
}

.details {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 18px 24px 20px;
}

.tagline {
  color: var(--text);
  font-size: 13px;
}

.version {
  margin-top: 10px;
  padding: 3px 11px;
  border-radius: 11px;
  background: var(--fill);
  color: var(--text-dim);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  user-select: text;
  -webkit-user-select: text;
}

.links {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 4px;
  width: 100%;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}

footer {
  display: flex;
  justify-content: center;
  gap: 8px;
  margin-top: 16px;
}

footer :deep(.n-button) {
  min-width: 96px;
}
</style>
