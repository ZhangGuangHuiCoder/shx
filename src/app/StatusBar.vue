<script setup lang="ts">
import { FolderTree, HeartPulse, Moon, SquareTerminal, Sun, SunMoon } from "@lucide/vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { computed, ref, type Component } from "vue";
import { openAssistant } from "@/features/commands/commandAssistant";
import { slotOf } from "@/features/session/keepAlive";
import TransferPanel from "@/features/transfer/TransferPanel.vue";
import TransferStatus from "@/features/transfer/TransferStatus.vue";
import TunnelStatus from "@/features/tunnel/TunnelStatus.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { errorMessage } from "@/shared/api";
import { checkIcon, contextMenu, openContextMenu, type MenuEntry } from "@/shared/contextMenu";
import { t, unitGap } from "@/shared/i18n";
import { THEME_SETTINGS } from "@/shared/theme";
import { message } from "@/shared/ui";
import { statusDotColor } from "@/shared/util";
import { useApp } from "@/shared/store";
import { dim, glassBar, hoverFill, line, lineFill, okText, statusDot } from "@/shared/ui-kit/styles";

// 状态栏：左侧是当前标签页的连接状态，右侧是隧道、快捷操作和传输。
const app = useApp();
const transfersOpen = ref(false);

const tab = computed(() => app.tabs.find((item) => item.key === app.activeKey) ?? null);
/** “用户@主机[:端口]”，默认端口省略。 */
const address = computed(() => {
  const profile = app.profiles.find((item) => item.id === tab.value?.profileId);
  if (!profile) return "";
  return `${profile.username}@${profile.host}${profile.port === 22 ? "" : `:${profile.port}`}`;
});
/** 当前标签页所在的保活会话编号。 */
const slot = computed(() => (tab.value ? slotOf(tab.value.profileId, tab.value.keepAlive) : null));
const sessionCount = computed(() => app.tabs.filter((item) => item.status === "connected").length);

function copyAddress() {
  writeText(address.value).then(
    () => message.success(t("fm.copied")),
    (e) => message.error(errorMessage(e)),
  );
}

interface QuickAction {
  id: string;
  label: string;
  /** 比 label 更具体的悬停提示。 */
  title?: string;
  icon: Component;
  on?: boolean;
  /** 仅在会话页显示。 */
  sessionOnly?: boolean;
  run: (event: MouseEvent) => unknown;
}

const THEME_ICON = { light: Sun, dark: Moon, auto: SunMoon };

/** 按下时菜单已开着，这次点击只用来关闭菜单。 */
let menuWasOpen = false;
function rememberMenu() {
  menuWasOpen = contextMenu.show;
}

function openFilesMenu(event: MouseEvent) {
  if (menuWasOpen) return;
  const settings = app.settings;
  const position = (value: "bottom" | "right", label: string): MenuEntry => ({
    label,
    icon: checkIcon(settings.fileLayout === value),
    disabled: !settings.showFiles,
    action: () => (settings.fileLayout = value),
  });
  openContextMenu(event, [
    { heading: t("status.files") },
    { group: t("status.filesShow") },
    { label: t("menu.showFiles"), icon: checkIcon(settings.showFiles), action: () => (settings.showFiles = !settings.showFiles) },
    { label: t("menu.showHidden"), icon: checkIcon(settings.showHidden), action: () => (settings.showHidden = !settings.showHidden) },
    { group: t("status.filesPosition") },
    position("bottom", t("menu.fileBottom")),
    position("right", t("menu.fileRight")),
  ]);
}

const actions = computed<QuickAction[]>(() => {
  const settings = app.settings;
  return [
    { id: "files", label: t("status.files"), icon: FolderTree, on: settings.showFiles, sessionOnly: true, run: openFilesMenu },
    { id: "assistant", label: t("cmd.assistant"), icon: SquareTerminal, run: () => openAssistant() },
    {
      id: "theme",
      label: t("menu.theme"),
      title: t("theme.switch", { name: t(`theme.${settings.theme}`) }),
      icon: THEME_ICON[settings.theme],
      run: () => (settings.theme = THEME_SETTINGS[(THEME_SETTINGS.indexOf(settings.theme) + 1) % THEME_SETTINGS.length]),
    },
  ];
});
const shownActions = computed(() =>
  actions.value.filter((action) => app.settings.statusActions.includes(action.id) && (!action.sessionOnly || tab.value)),
);

/** 右键选择要显示的快捷操作。 */
function chooseActions(event: MouseEvent) {
  const chosen = app.settings.statusActions;
  openContextMenu(event, [
    { heading: t("status.actions"), detail: t("status.actionsHint") },
    ...actions.value.map(({ id, label }) => ({
      label,
      icon: checkIcon(chosen.includes(id)),
      action: () => (app.settings.statusActions = chosen.includes(id) ? chosen.filter((item) => item !== id) : [...chosen, id]),
    })),
  ]);
}
</script>

<template>
  <div :class="[glassBar, line, 'shrink-0 border-t']">
    <TransferPanel v-if="transfersOpen" />
    <div class="flex h-7 items-center text-xs" @contextmenu="chooseActions">
      <template v-if="tab">
        <span class="flex shrink-0 items-center gap-2 px-3"><i :class="[statusDot, statusDotColor(tab.status), tab.status === 'connecting' && 'motion-safe:animate-pulse']" />{{ t(`status.${tab.status}`) }}</span>
        <button v-if="address" type="button" :class="[dim, hoverFill, '-ml-1 flex h-full min-w-0 shrink cursor-pointer items-center gap-2 bg-transparent px-2 outline-none hover:text-zinc-800 focus-visible:ring-2 focus-visible:ring-blue-500/60 focus-visible:ring-inset dark:hover:text-zinc-100']" :title="t('status.copyAddress')" @click="copyAddress">
          <span class="truncate">{{ address }}</span>
        </button>
        <!-- 是否真正进入 tmux 会话，连上后才知道。 -->
        <span
          v-if="tab.keepAlive && tab.status === 'connected'"
          class="flex shrink-0 items-center gap-2 px-3"
          :class="tab.attached ? okText : dim"
          :title="t(tab.attached ? 'status.keepAliveOn' : 'status.keepAliveOff')"
        >
          <HeartPulse :size="13" />{{ t("profile.keepAlive") }}<template v-if="slot"> #{{ slot }}</template>
        </span>
      </template>
      <template v-else>
        <span :class="[dim, 'flex shrink-0 items-center gap-2 px-3']">{{ app.profiles.length }}{{ unitGap }}{{ t("home.stat.hosts") }}</span>
        <span v-if="sessionCount" class="flex shrink-0 items-center gap-2 px-3"><i :class="[statusDot, 'bg-emerald-500']" />{{ sessionCount }}{{ unitGap }}{{ t("home.stat.sessions") }}</span>
      </template>
      <span class="flex-1" />
      <TunnelStatus />
      <!-- 分组间的细线。 -->
      <i v-if="app.tunnel" :class="[lineFill, 'h-3 w-px shrink-0']" />
      <div v-if="shownActions.length" class="flex shrink-0 items-center gap-1 px-2">
        <IconButton
          v-for="action in shownActions"
          :key="action.id"
          size="tiny"
          :selected="action.on"
          :title="action.title ?? action.label"
          @mousedown="rememberMenu"
          @click="action.run"
        >
          <component :is="action.icon" :size="14" />
        </IconButton>
      </div>
      <i v-if="shownActions.length" :class="[lineFill, 'h-3 w-px shrink-0']" />
      <TransferStatus v-model:open="transfersOpen" />
    </div>
  </div>
</template>
