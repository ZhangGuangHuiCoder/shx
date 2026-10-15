<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiSpin from "@/shared/ui-kit/UiSpin.vue";
import { t } from "@/shared/i18n";
import { useApp, type Tab } from "@/shared/store";
import FileManager from "@/features/files/FileManager.vue";
import { filePaneMinWidth } from "@/features/session/layout";
import TerminalPane from "@/features/session/TerminalPane.vue";

const props = defineProps<{ tab: Tab; active: boolean }>();
const app = useApp();

/** 面板使用的会话；断线重连期间保留旧会话，界面状态不重置。 */
const sessionId = ref(props.tab.sessionId);
watch(
  () => props.tab.sessionId,
  (id) => {
    if (id) sessionId.value = id;
  },
);

const terminal = ref<InstanceType<typeof TerminalPane>>();
const files = ref<InstanceType<typeof FileManager>>();
const root = ref<HTMLElement>();
const layout = computed(() => (app.settings.fileLayout === "right" ? "right" : "bottom"));
/** 文件管理器占比，按布局分别记忆。 */
const share = reactive({ bottom: 0.45, right: 0.5 });
/** 右侧布局时的最小宽度，保证列表无需横向滚动。 */
const paneMinWidth = computed(() => (layout.value === "right" ? filePaneMinWidth(app.fileTree.show) : 0));
/** 右侧布局时终端的最小宽度，空间不足时优先保证终端。 */
const TERMINAL_KEEP = 360;
const paneSize = computed(() => {
  const size = `${share[layout.value] * 100}%`;
  return layout.value === "right" ? { width: size, minWidth: `min(${paneMinWidth.value}px, calc(100% - ${TERMINAL_KEEP}px))` } : { height: size };
});
function setAttached(attached: boolean) {
  props.tab.attached = attached;
}

let stopDrag: (() => void) | undefined;

function startDrag(e: MouseEvent) {
  e.preventDefault();
  const move = (ev: MouseEvent) => {
    const box = root.value!.getBoundingClientRect();
    const part = layout.value === "right" ? (box.right - ev.clientX) / box.width : (box.bottom - ev.clientY) / box.height;
    const least = Math.min(paneMinWidth.value, box.width - TERMINAL_KEEP) / box.width;
    share[layout.value] = Math.min(0.85, Math.max(0.12, part, least));
  };
  stopDrag = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", stopDrag!);
    stopDrag = undefined;
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", stopDrag);
}

onBeforeUnmount(() => stopDrag?.());
</script>

<template>
  <div ref="root" class="absolute inset-0 flex" :class="layout === 'right' ? 'flex-row' : 'flex-col'">
    <template v-if="sessionId">
      <div class="min-h-0 min-w-0 flex-1">
        <TerminalPane ref="terminal" :session-id="sessionId" :keep-alive="tab.keepAlive" :active="active" @cwd="files?.terminalCwd($event)" @restarted="files?.shellRestarted()" @attached="setAttached" />
      </div>
      <!-- 隐藏而不卸载，保留状态。 -->
      <div
        v-show="app.settings.showFiles"
        class="shrink-0 border-zinc-900/10 bg-white/60 transition-colors duration-150 hover:bg-blue-500 dark:border-white/10 dark:bg-zinc-900/60 dark:hover:bg-blue-500"
        :class="layout === 'right' ? 'w-1 cursor-col-resize border-l' : 'h-1 cursor-row-resize border-t'"
        @mousedown="startDrag"
      />
      <div v-show="app.settings.showFiles" class="min-h-0 min-w-0 shrink-0" :style="paneSize">
        <FileManager ref="files" :session-id="sessionId" :profile-id="tab.profileId" :active="active && app.settings.showFiles && tab.status === 'connected'" @send="terminal?.write($event)" />
      </div>
    </template>
    <div v-if="tab.status !== 'connected'" class="absolute inset-0 z-20 flex flex-col items-center justify-center gap-3" :class="sessionId ? 'bg-zinc-100/80 backdrop-blur-sm dark:bg-zinc-950/80' : 'bg-zinc-100/60 dark:bg-zinc-950/60'">
      <template v-if="tab.status === 'connecting'">
        <UiSpin :size="22" />
        <div>{{ t("session.connecting", { name: tab.title }) }}</div>
      </template>
      <template v-else>
        <div class="max-w-130 text-center text-red-600 select-text dark:text-red-400">{{ tab.error ?? t("session.closed") }}</div>
        <UiButton type="primary" size="small" @click="app.reconnect(tab)">{{ t("session.reconnect") }}</UiButton>
      </template>
    </div>
  </div>
</template>

