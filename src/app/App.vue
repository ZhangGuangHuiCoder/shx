<script setup lang="ts">
import { Plus, X } from "@lucide/vue";
import { onMounted, watchEffect } from "vue";
import CommandAssistant from "@/features/commands/CommandAssistant.vue";
import HomePage from "@/features/home/HomePage.vue";
import RulesDialog from "@/features/rules/RulesDialog.vue";
import { windowMinWidth } from "@/features/session/layout";
import SessionView from "@/features/session/SessionView.vue";
import TunnelDialog from "@/features/tunnel/TunnelDialog.vue";
import { errorMessage } from "@/shared/api";
import ContextMenuHost from "@/shared/ui-kit/ContextMenuHost.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import DialogHost from "@/shared/ui-kit/DialogHost.vue";
import Toaster from "@/shared/ui-kit/Toaster.vue";
import UiSpin from "@/shared/ui-kit/UiSpin.vue";
import { t } from "@/shared/i18n";
import { CUSTOM_TITLEBAR, OVERLAY_TITLEBAR } from "@/shared/titlebar";
import { message } from "@/shared/ui";
import { statusDotColor } from "@/shared/util";
import { useApp } from "@/shared/store";
import { glassBar, line, raisedChip, statusDot } from "@/shared/ui-kit/styles";
import { installCloseGuard } from "@/app/closeGuard";
import { installMenu } from "@/app/menu";
import { installShortcuts } from "@/app/shortcuts";
import ShortcutsDialog from "@/features/shortcuts/ShortcutsDialog.vue";
import StatusBar from "@/app/StatusBar.vue";
import TitleBar from "@/app/TitleBar.vue";
import { checkForUpdatesOnLaunch } from "@/app/update";
import { setMinWidth } from "@/app/window";

const app = useApp();

// 最小窗口宽度随布局变化，保证文件列表放得下所有列。
watchEffect(() => {
  const view = { showFiles: app.settings.showFiles, fileLayout: app.settings.fileLayout, treeShown: app.fileTree.show };
  void setMinWidth(windowMinWidth(view, window.screen.availWidth));
});

// 必须在子组件挂载前注册，先于标题栏的快捷键监听，录制键位时才拦得住。
installShortcuts(useApp);

onMounted(() => {
  app.init().catch((e) => message.error(errorMessage(e)));
  installCloseGuard().catch((e) => message.error(errorMessage(e)));
  installMenu().catch((e) => message.error(t("app.menuFailed", { error: errorMessage(e) })));
  checkForUpdatesOnLaunch();
});
</script>

<template>
  <div class="relative isolate flex h-full">
    <main class="flex min-w-0 flex-1 flex-col">
      <TitleBar v-if="CUSTOM_TITLEBAR" />
      <!-- 无原生标题栏时，标签栏空白处可拖动窗口。 -->
      <div
        v-if="app.tabs.length"
        :class="[glassBar, line, 'flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-b pr-2 pl-1.5 lights:pl-20']"
        data-no-scrollbar
        :data-tauri-drag-region="OVERLAY_TITLEBAR || undefined">
        <div
          v-for="tab in app.tabs"
          :key="tab.key"
          class="group flex h-7 max-w-52 min-w-28 shrink-0 cursor-pointer items-center gap-2 rounded-full pr-1 pl-3 text-sm transition duration-200 ease-out"
          :class="tab.key === app.activeKey
            ? [raisedChip, 'font-semibold text-zinc-900 dark:text-white']
            : 'text-zinc-500 hover:bg-zinc-900/5 hover:text-zinc-800 dark:text-zinc-400 dark:hover:bg-white/10 dark:hover:text-zinc-100'"
          @click="app.activeKey = tab.key"
          @mousedown.middle.prevent="app.closeTab(tab.key)"
        >
          <UiSpin v-if="tab.status === 'connecting'" :size="12" />
          <span v-else :class="[statusDot, statusDotColor(tab.status)]" />
          <span class="truncate">{{ app.tabLabel(tab) }}</span>
          <IconButton class="ml-auto" :class="tab.key === app.activeKey ? 'visible' : 'invisible group-hover:visible'" size="tiny" :title="t('common.close')" @click.stop="app.closeTab(tab.key)"><X :size="13" /></IconButton>
        </div>
        <IconButton class="shrink-0" :selected="app.activeKey === null" :title="t('home.newTab')" @click="app.goHome()"><Plus :size="15" /></IconButton>
      </div>
      <div class="relative min-h-0 flex-1">
        <SessionView
          v-for="tab in app.tabs"
          v-show="tab.key === app.activeKey"
          :key="tab.key"
          :tab="tab"
          :active="tab.key === app.activeKey"
        />
        <!-- 首页做成覆盖层，避免卸载会话导致服务器端重开 shell。 -->
        <HomePage v-if="app.activeKey === null" class="absolute inset-0 z-10" />
      </div>
      <StatusBar v-show="app.settings.showStatusBar" />
    </main>
    <ContextMenuHost />
    <DialogHost />
    <Toaster />
    <CommandAssistant />
    <RulesDialog />
    <ShortcutsDialog />
    <TunnelDialog />
  </div>
</template>
