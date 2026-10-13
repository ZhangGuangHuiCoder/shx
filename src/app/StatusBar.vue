<script setup lang="ts">
import { Check, FolderTree, HeartPulse, Moon, SquareTerminal, Sun, SunMoon } from "@lucide/vue";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { computed, ref, type Component } from "vue";
import { openAssistant } from "../features/commands/commandAssistant";
import { slotOf } from "../features/session/keepAlive";
import TransferPanel from "../features/transfer/TransferPanel.vue";
import TransferStatus from "../features/transfer/TransferStatus.vue";
import TunnelStatus from "../features/tunnel/TunnelStatus.vue";
import IconButton from "../shared/components/IconButton.vue";
import { errorMessage } from "../shared/api";
import { contextMenu, openContextMenu, type MenuEntry } from "../shared/contextMenu";
import { t } from "../shared/i18n";
import { THEME_SETTINGS } from "../shared/theme";
import { message } from "../shared/ui";
import { useApp } from "../stores/app";

// 窗口底部的状态栏：左边是当前标签页的连接状态，右边是整个应用的事——隧道、快捷操作、传输，
// 三组之间用细线隔开。
const app = useApp();
const transfersOpen = ref(false);

/** 当前标签页；在首页时没有。 */
const tab = computed(() => app.tabs.find((item) => item.key === app.activeKey) ?? null);
/** 当前标签页连接的地址，写成“用户@主机”，不是默认端口时带上端口。 */
const address = computed(() => {
  const profile = app.profiles.find((item) => item.id === tab.value?.profileId);
  if (!profile) return "";
  return `${profile.username}@${profile.host}${profile.port === 22 ? "" : `:${profile.port}`}`;
});
/** 当前标签页进的是这台主机的第几个保活会话。 */
const slot = computed(() => (tab.value ? slotOf(tab.value.profileId, tab.value.keepAlive) : null));
const sessionCount = computed(() => app.tabs.filter((item) => item.status === "connected").length);

function copyAddress() {
  writeText(address.value).then(
    () => message.success(t("fm.copied")),
    (e) => message.error(errorMessage(e)),
  );
}

// ---- 快捷操作 --------------------------------------------------------------

/** 状态栏上的一个快捷操作：点一下就执行的按钮。 */
interface QuickAction {
  id: string;
  /** 在右键菜单里的名字，也是按钮的悬停提示。 */
  label: string;
  /** 悬停提示需要比名字说得更具体时用它。 */
  title?: string;
  icon: Component;
  /** 开关类操作当前是否打开。 */
  on?: boolean;
  run: (event: MouseEvent) => unknown;
}

const THEME_ICON = { light: Sun, dark: Moon, auto: SunMoon };

/** 菜单里没勾选的项：占住图标的位置，各项的文字才对得齐。 */
const Unchecked = () => null;
const check = (checked: boolean) => (checked ? Check : Unchecked);

/** 按下鼠标时菜单是否开着：开着的话这一下是用来关它的，不该接着又打开。 */
let menuWasOpen = false;
function rememberMenu() {
  menuWasOpen = contextMenu.show;
}

/** 文件管理器的各项控制收在一个按钮里：显示与否、放在哪一侧、是否列出隐藏文件。 */
function openFilesMenu(event: MouseEvent) {
  if (menuWasOpen) return;
  const settings = app.settings;
  const position = (value: "bottom" | "right", label: string): MenuEntry => ({
    label,
    icon: check(settings.fileLayout === value),
    disabled: !settings.showFiles,
    action: () => (settings.fileLayout = value),
  });
  openContextMenu(event, [
    { heading: t("status.files") },
    { group: t("status.filesShow") },
    { label: t("menu.showFiles"), icon: check(settings.showFiles), action: () => (settings.showFiles = !settings.showFiles) },
    { label: t("menu.showHidden"), icon: check(settings.showHidden), action: () => (settings.showHidden = !settings.showHidden) },
    { group: t("status.filesPosition") },
    position("bottom", t("menu.fileBottom")),
    position("right", t("menu.fileRight")),
  ]);
}

/** 全部可选的快捷操作，按显示顺序排列。 */
const actions = computed<QuickAction[]>(() => {
  const settings = app.settings;
  return [
    { id: "files", label: t("status.files"), icon: FolderTree, on: settings.showFiles, run: openFilesMenu },
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
const shownActions = computed(() => actions.value.filter((action) => app.settings.statusActions.includes(action.id)));

/** 右键状态栏：勾选要显示哪些快捷操作。 */
function chooseActions(event: MouseEvent) {
  const chosen = app.settings.statusActions;
  openContextMenu(event, [
    { heading: t("status.actions"), detail: t("status.actionsHint") },
    ...actions.value.map(({ id, label }) => ({
      label,
      icon: check(chosen.includes(id)),
      action: () => (app.settings.statusActions = chosen.includes(id) ? chosen.filter((item) => item !== id) : [...chosen, id]),
    })),
  ]);
}
</script>

<template>
  <div class="status">
    <TransferPanel v-if="transfersOpen" />
    <div class="bar" @contextmenu="chooseActions">
      <template v-if="tab">
        <span class="item"><i class="status-dot" :class="tab.status" />{{ t(`status.${tab.status}`) }}</span>
        <button v-if="address" type="button" class="item dim address" :title="t('status.copyAddress')" @click="copyAddress">
          <span class="ellipsis">{{ address }}</span>
        </button>
        <!-- 开了保活的主机：终端是否真的进了 tmux 会话，连上之后才知道。 -->
        <span
          v-if="tab.keepAlive && tab.status === 'connected'"
          class="item"
          :class="tab.attached ? 'on' : 'dim'"
          :title="t(tab.attached ? 'status.keepAliveOn' : 'status.keepAliveOff')"
        >
          <HeartPulse :size="13" />{{ t("profile.keepAlive") }}<template v-if="slot"> #{{ slot }}</template>
        </span>
      </template>
      <template v-else>
        <span class="item dim">{{ app.profiles.length }} {{ t("home.stat.hosts") }}</span>
        <span v-if="sessionCount" class="item"><i class="status-dot connected" />{{ sessionCount }} {{ t("home.stat.sessions") }}</span>
      </template>
      <span class="spacer" />
      <!-- 隧道是整台机器的事，不属于哪个标签页：在哪一页都显示。 -->
      <TunnelStatus />
      <i v-if="app.tunnel" class="divider" />
      <div v-if="shownActions.length" class="actions">
        <IconButton
          v-for="action in shownActions"
          :key="action.id"
          size="tiny"
          :type="action.on ? 'primary' : 'default'"
          :title="action.title ?? action.label"
          @mousedown="rememberMenu"
          @click="action.run"
        >
          <component :is="action.icon" :size="14" />
        </IconButton>
      </div>
      <i v-if="shownActions.length" class="divider" />
      <TransferStatus v-model:open="transfersOpen" />
    </div>
  </div>
</template>

<style scoped>
.status {
  flex-shrink: 0;
  border-top: 1px solid var(--border);
  background: var(--bg-raised);
}

.bar {
  display: flex;
  align-items: center;
  height: 26px;
  font-size: 12px;
}

.item {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
}

/* 地址点一下就复制：是个按钮，但长得和旁边的文字一样，指针移上去才看得出来。 */
.item.address {
  flex-shrink: 1;
  min-width: 0;
  height: 100%;
  margin-left: -4px;
  padding: 0 6px;
  border: 0;
  background: none;
  font: inherit;
  cursor: pointer;
}

.item.address:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.item.address:focus-visible {
  outline: 2px solid var(--accent-border);
  outline-offset: -2px;
}

/* 还在连接：圆点一明一暗，和连上了、断开了的静止圆点区分开。 */
.item .status-dot.connecting {
  animation: connecting 1.2s ease-in-out infinite;
}

@keyframes connecting {
  50% {
    opacity: 0.3;
  }
}

@media (prefers-reduced-motion: reduce) {
  .item .status-dot.connecting {
    animation: none;
  }
}

.item.on {
  color: var(--ok);
}

.spacer {
  flex: 1;
}

.actions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 0 6px;
}

/* 右边几组之间的细线，比状态栏矮一截。 */
.divider {
  flex-shrink: 0;
  width: 1px;
  height: 12px;
  background: var(--border);
}
</style>
