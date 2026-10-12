<script setup lang="ts">
import { NButton, NSpin } from "naive-ui";
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import { t } from "../../shared/i18n";
import { useApp, type Tab } from "../../stores/app";
import FileManager from "../files/FileManager.vue";
import TerminalPane from "./TerminalPane.vue";

const props = defineProps<{ tab: Tab; active: boolean }>();
const app = useApp();

/**
 * 各面板所使用的会话。它比连接活得久：标签页断开或重连期间保留上一个会话，
 * 终端和文件视图保持原样，等新会话建立后接着用，而不是从头开始。
 */
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
/** 文件管理器位于终端下方，或在右侧。 */
const layout = computed(() => (app.settings.fileLayout === "right" ? "right" : "bottom"));
/** 文件管理器占会话区域的比例，按布局分别记住。 */
const share = reactive({ bottom: 0.45, right: 0.5 });
const paneSize = computed(() => {
  const size = `${share[layout.value] * 100}%`;
  return layout.value === "right" ? { width: size } : { height: size };
});
/** 终端报告它最终是否进入了该标签页的保持会话。 */
function setAttached(attached: boolean) {
  props.tab.attached = attached;
}

let stopDrag: (() => void) | undefined;

function startDrag(e: MouseEvent) {
  e.preventDefault();
  const move = (ev: MouseEvent) => {
    const box = root.value!.getBoundingClientRect();
    const part = layout.value === "right" ? (box.right - ev.clientX) / box.width : (box.bottom - ev.clientY) / box.height;
    share[layout.value] = Math.min(0.85, Math.max(0.12, part));
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
  <div ref="root" class="session" :class="layout">
    <template v-if="sessionId">
      <div class="terminal-pane">
        <TerminalPane ref="terminal" :session-id="sessionId" :keep-alive="tab.keepAlive" :active="active" @cwd="files?.terminalCwd($event)" @restarted="files?.shellRestarted()" @attached="setAttached" />
      </div>
      <!-- 隐藏文件管理器时只是不显示，不卸载：它的当前目录和选择留着，再显示时还在。 -->
      <div v-show="app.settings.showFiles" class="splitter" @mousedown="startDrag" />
      <div v-show="app.settings.showFiles" class="file-pane" :style="paneSize">
        <FileManager ref="files" :session-id="sessionId" :active="active && app.settings.showFiles && tab.status === 'connected'" @send="terminal?.write($event)" />
      </div>
    </template>
    <div v-if="tab.status !== 'connected'" class="overlay" :class="{ solid: !sessionId }">
      <template v-if="tab.status === 'connecting'">
        <NSpin :size="22" />
        <div>{{ t("session.connecting", { name: tab.title }) }}</div>
      </template>
      <template v-else>
        <div class="error">{{ tab.error ?? t("session.closed") }}</div>
        <NButton type="primary" size="small" @click="app.reconnect(tab)">{{ t("session.reconnect") }}</NButton>
      </template>
    </div>
  </div>
</template>

<style scoped>
.session {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
}

.session.right {
  flex-direction: row;
}

.terminal-pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.splitter {
  height: 5px;
  flex-shrink: 0;
  border-top: 1px solid var(--border);
  background: var(--bg-raised);
  cursor: row-resize;
}

.right > .splitter {
  width: 5px;
  height: auto;
  border-top: 0;
  border-left: 1px solid var(--border);
  cursor: col-resize;
}

.splitter:hover {
  background: var(--accent);
}

.file-pane {
  flex-shrink: 0;
  min-width: 0;
  min-height: 0;
}

.overlay {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  background: var(--overlay);
}

.overlay.solid {
  background: var(--bg);
}

.error {
  max-width: 520px;
  color: var(--danger);
  text-align: center;
  user-select: text;
  -webkit-user-select: text;
}
</style>
