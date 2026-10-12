<script setup lang="ts">
import { Plus, X } from "@lucide/vue";
import { dateEnUS, dateZhCN, enUS, NConfigProvider, NSpin, zhCN } from "naive-ui";
import { computed, onMounted } from "vue";
import CommandAssistant from "../features/commands/CommandAssistant.vue";
import HomePage from "../features/home/HomePage.vue";
import RulesDialog from "../features/rules/RulesDialog.vue";
import SessionView from "../features/session/SessionView.vue";
import { errorMessage } from "../shared/api";
import ContextMenuHost from "../shared/components/ContextMenuHost.vue";
import { locale, t } from "../shared/i18n";
import { naiveTheme, themeOverrides } from "../shared/theme";
import { CUSTOM_TITLEBAR, OVERLAY_TITLEBAR } from "../shared/titlebar";
import { message } from "../shared/ui";
import { useApp } from "../stores/app";
import { installCloseGuard } from "./closeGuard";
import { installMenu } from "./menu";
import StatusBar from "./StatusBar.vue";
import TitleBar from "./TitleBar.vue";

const app = useApp();

// Naive UI 自带的文字（日期选择器、空状态等）跟随应用语言。
const naive = computed(() => (locale.value === "zh" ? { locale: zhCN, date: dateZhCN } : { locale: enUS, date: dateEnUS }));

onMounted(() => {
  app.init().catch((e) => message.error(errorMessage(e)));
  installCloseGuard().catch((e) => message.error(errorMessage(e)));
  installMenu().catch((e) => message.error(t("app.menuFailed", { error: errorMessage(e) })));
});
</script>

<template>
  <NConfigProvider :theme="naiveTheme" :theme-overrides="themeOverrides" :locale="naive.locale" :date-locale="naive.date" class="root">
    <main class="main">
      <TitleBar v-if="CUSTOM_TITLEBAR" />
      <!-- 没有原生标题栏时，标签栏的空白处用来拖动窗口。 -->
      <div v-if="app.tabs.length" class="tabbar" :data-tauri-drag-region="OVERLAY_TITLEBAR || undefined">
        <div
          v-for="tab in app.tabs"
          :key="tab.key"
          class="tab"
          :class="{ active: tab.key === app.activeKey }"
          @click="app.activeKey = tab.key"
          @mousedown.middle.prevent="app.closeTab(tab.key)"
        >
          <NSpin v-if="tab.status === 'connecting'" :size="12" />
          <span v-else class="dot" :class="tab.status" />
          <span class="ellipsis">{{ app.tabLabel(tab) }}</span>
          <button class="icon-btn close" :title="t('common.close')" @click.stop="app.closeTab(tab.key)"><X :size="13" /></button>
        </div>
        <button class="icon-btn new-tab" :class="{ active: app.activeKey === null }" :title="t('home.newTab')" @click="app.goHome()"><Plus :size="15" /></button>
      </div>
      <div class="views">
        <SessionView
          v-for="tab in app.tabs"
          v-show="tab.key === app.activeKey"
          :key="tab.key"
          :tab="tab"
          :active="tab.key === app.activeKey"
        />
        <!-- 在首页时盖住各会话；做成覆盖层，回首页就不会卸载活动会话
             （卸载会让服务器端重新打开 shell）。 -->
        <HomePage v-if="app.activeKey === null" class="view-layer" />
      </div>
      <StatusBar v-show="app.settings.showStatusBar" />
    </main>
    <ContextMenuHost />
    <CommandAssistant />
    <RulesDialog />
  </NConfigProvider>
</template>

<style scoped>
.root {
  display: flex;
  height: 100%;
}

.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.tabbar {
  display: flex;
  align-items: center;
  gap: 3px;
  height: 34px;
  padding: 0 6px 0 max(6px, var(--lights-width));
  background: var(--bg-raised);
  border-bottom: 1px solid var(--border);
  overflow-x: auto;
  flex-shrink: 0;
}

.tabbar::-webkit-scrollbar {
  display: none;
}

.tab {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 7px;
  max-width: 200px;
  min-width: 110px;
  height: 26px;
  padding: 0 3px 0 10px;
  border: 1px solid transparent;
  border-radius: 7px;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.tab:hover {
  background: var(--bg-hover);
  color: var(--text);
}

/* 带描边的实心胶囊：无论哪种主题、下面是什么，都能看出是“当前标签”。 */
.tab.active {
  background: var(--bg-active);
  border-color: var(--accent-border);
  color: var(--text);
  font-weight: 600;
}

.tab .close {
  margin-left: auto;
  height: 20px;
  min-width: 20px;
  padding: 0;
  visibility: hidden;
}

.tab:hover .close,
.tab.active .close {
  visibility: visible;
}

.tab.active .close:hover {
  background: var(--accent-soft-hover);
}

.new-tab {
  flex-shrink: 0;
  width: 26px;
  height: 26px;
  border-radius: 7px;
  color: var(--text-dim);
}

.new-tab:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.new-tab.active {
  background: var(--bg-active);
  color: var(--accent);
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--ok);
}

.dot.closed {
  background: var(--danger);
}

.views {
  flex: 1;
  min-height: 0;
  position: relative;
}

.view-layer {
  position: absolute;
  inset: 0;
  z-index: 10;
}
</style>
