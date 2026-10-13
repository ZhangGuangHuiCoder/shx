<script setup lang="ts">
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ChevronDown, ChevronUp, ClipboardPaste, Copy, Eraser, SquareChevronRight, TextSelect, X } from "@lucide/vue";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api, errorMessage } from "../../shared/api";
import IconButton from "../../shared/components/IconButton.vue";
import { openContextMenu } from "../../shared/contextMenu";
import { loadedMonoFamily } from "../../shared/fonts";
import { t } from "../../shared/i18n";
import { confirm, message } from "../../shared/ui";
import { useApp } from "../../stores/app";
import { openAssistant, setCurrentTerminal, type UseCommand } from "../commands/commandAssistant";
import { terminalTheme } from "./terminalTheme";

const props = defineProps<{ sessionId: string; /** 开启会话保持的主机：终端所在的 tmux 会话。 */ keepAlive: string | null; active: boolean }>();
const emit = defineEmits<{ cwd: [path: string]; /** 重连后得到的是新 shell，而不是之前那个。 */ restarted: []; /** shell 现在是否运行在保持会话里。 */ attached: [on: boolean] }>();
const app = useApp();

const host = ref<HTMLElement>();
const searching = ref(false);
const query = ref("");
const searchInput = ref<HTMLInputElement>();

const term = new Terminal({
  fontFamily: loadedMonoFamily.value,
  fontSize: app.settings.terminalFontSize,
  cursorBlink: true,
  scrollback: 10000,
  // 清屏时把内容移入回滚缓冲区，而不是丢弃。
  scrollOnEraseInDisplay: true,
  allowProposedApi: true,
  macOptionIsMeta: true,
  theme: terminalTheme.value,
});
const fit = new FitAddon();
const search = new SearchAddon();
let observer: ResizeObserver | undefined;

/**
 * 把终端恢复到普通状态，但不移动光标，也不改动屏幕内容：关闭鼠标上报、括号粘贴和
 * 应用键模式，恢复默认颜色和全屏滚动区域。因连接丢失而被打断的程序（编辑器、分页器）
 * 没有机会自己恢复这些。
 */
const RESET_MODES = "\x1b[?1000;1002;1003;1006;1004;2004l\x1b[?1l\x1b>\x1b[?7h\x1b[?25h\x1b7\x1b[?6l\x1b[r\x1b8\x1b[0m\x1b(B";

/** 画一条分隔线，把断开前的输出和新 shell 的输出分开。 */
function markReconnected(): Promise<void> {
  // 之前有全屏程序在运行：切回启动它时的屏幕。
  const leaveAlternate = term.buffer.active.type === "alternate" ? "\x1b[?1049l" : "";
  return new Promise((resolve) => {
    term.write(leaveAlternate + RESET_MODES, () => {
      // 画在屏幕所有内容的下方，不管光标停在哪里。
      const buffer = term.buffer.active;
      let last = term.rows - 1;
      while (last > buffer.cursorY && !buffer.getLine(buffer.baseY + last)?.translateToString(true)) last--;
      term.write(`\x1b[${last + 1};1H\r\n\x1b[2m${t("term.reconnected")}\x1b[0m\r\n`, resolve);
    });
  });
}

/** 每个标签页只询问一次：没有 tmux 的主机不该每次重连都来打扰。 */
let installOffered = false;

/**
 * 让服务器为该标签页的 tmux 会话做好准备。返回其中的 shell 是新的（`fresh`）还是
 * 保持下来的；终端只能不用 tmux 时返回 null：服务器没有 tmux（此时提议安装），
 * 或者检查本身失败。
 */
async function prepareKeepAlive(id: string, name: string): Promise<{ fresh: boolean } | null> {
  try {
    const state = await api.keepAlivePrepare(id, name, term.cols, term.rows);
    if (state.ready) return { fresh: state.fresh };
    if (!installOffered) {
      installOffered = true;
      // 不等待：普通 shell 同时打开，安装命令就在它里面执行。
      void offerInstall(id, state.install);
    }
  } catch (e) {
    message.warning(t("keepAlive.failed", { error: errorMessage(e) }), { duration: 8000 });
  }
  return null;
}

async function offerInstall(id: string, command: string | null) {
  if (!command) return void message.warning(t("keepAlive.manual"), { duration: 10000 });
  const ok = await confirm(t("keepAlive.missingTitle"), t("keepAlive.missingBody", { command }), { ok: t("keepAlive.install") });
  // 回答可能很久之后才来：只往当初询问所针对的那个 shell 里输入。
  if (!ok || id !== props.sessionId) return;
  write(` ${command}\r`);
  term.focus();
}

/**
 * 在当前会话上启动 shell，并把输出接入终端。返回结果：新 shell（`fresh`）、
 * 从保持会话恢复（`resumed`），或失败（`failed`）。
 */
async function openShell(): Promise<"fresh" | "resumed" | "failed"> {
  const id = props.sessionId;
  const kept = props.keepAlive ? await prepareKeepAlive(id, props.keepAlive) : null;
  // 准备期间又换了连接：这个连接已经作废，接下来由新连接来打开 shell。
  if (id !== props.sessionId) return "failed";
  // tmux 接入时会重绘整个屏幕。先把屏幕上的内容移入回滚缓冲区，
  // 以免之前的内容被覆盖。
  if (kept) term.write("\x1b[2J\x1b[H");
  try {
    await api.termOpen(
      id,
      term.cols,
      term.rows,
      (bytes) => {
        // 会话已被替换后才到达的旧输出，直接丢弃。
        if (id === props.sessionId) term.write(bytes);
      },
      kept ? props.keepAlive : null,
    );
    emit("attached", !!kept);
    return kept && !kept.fresh ? "resumed" : "fresh";
  } catch (e) {
    if (id !== props.sessionId) return "failed";
    emit("attached", false);
    term.write(`\r\n\x1b[31m${t("term.openFailed", { error: errorMessage(e) })}\x1b[0m\r\n`);
    return "failed";
  }
}

function refit() {
  // 隐藏的标签页没有尺寸，这时去适配会把终端压成 1 列。
  if (host.value?.offsetWidth && host.value.offsetHeight) fit.fit();
}

/** 尺寸连续变化时，两次适配之间至少隔这么久（毫秒）。 */
const FIT_INTERVAL = 60;
let fitTimer: ReturnType<typeof setTimeout> | undefined;
let lastFit = 0;

/**
 * 跟着容器的尺寸适配。拖动分隔条或窗口边缘时尺寸每一帧都在变，但不必每一帧都适配：
 * 每次适配都要重排整个回滚缓冲区，还要通知服务器改窗口大小，保活会话里的 tmux
 * 会因此重画整屏。第一次立刻做，之后隔一小段做一次，停下来以后再补上最后一次。
 */
function refitSoon() {
  if (fitTimer !== undefined) return;
  fitTimer = setTimeout(
    () => {
      fitTimer = undefined;
      lastFit = performance.now();
      refit();
    },
    Math.max(0, FIT_INTERVAL - (performance.now() - lastFit)),
  );
}

function write(data: string) {
  api.termWrite(props.sessionId, data).catch(() => {});
}

function find(forward: boolean) {
  if (forward) search.findNext(query.value);
  else search.findPrevious(query.value);
}

async function copySelection() {
  if (term.hasSelection()) await writeText(term.getSelection());
}

async function pasteClipboard() {
  // 剪贴板里不是文本（比如图片）时读取会失败。
  const text = await readText().catch(() => "");
  if (!text) return void message.info(t("term.noText"));
  // `paste` 会应用括号粘贴模式，多行文本不会被逐行执行。
  term.paste(text);
  term.focus();
}

/** 命令助手里选中的命令输入到这里，要求执行时直接执行。 */
const useCommand: UseCommand = (command, run) => {
  // 以粘贴方式输入，多行命令才会作为一个整体到达。
  term.paste(command);
  if (run) write("\r");
  // 等对话框释放键盘焦点之后再聚焦。
  setTimeout(() => term.focus(), 100);
};

function showContextMenu(event: MouseEvent) {
  openContextMenu(event, [
    { label: t("cmd.assistantMenu"), icon: SquareChevronRight, action: () => openAssistant(useCommand) },
    "divider",
    { label: t("common.copy"), icon: Copy, disabled: !term.hasSelection(), action: copySelection },
    { label: t("common.paste"), icon: ClipboardPaste, action: pasteClipboard },
    "divider",
    { label: t("common.selectAll"), icon: TextSelect, action: () => term.selectAll() },
    { label: t("term.clear"), icon: Eraser, action: () => term.clear() },
  ]);
}

async function openSearch() {
  searching.value = true;
  await nextTick();
  searchInput.value?.select();
}

function closeSearch() {
  searching.value = false;
  search.clearDecorations();
  term.focus();
}

onMounted(async () => {
  term.loadAddon(fit);
  term.loadAddon(search);
  term.loadAddon(new Unicode11Addon());
  term.loadAddon(new WebLinksAddon((_event, uri) => void openUrl(uri)));
  term.unicode.activeVersion = "11";
  term.open(host.value!);
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    term.loadAddon(webgl);
  } catch {
    // 该 webview 不支持 WebGL：xterm 退回到 DOM 渲染器。
  }
  refit();

  // shell 通过 OSC 7（`file://host/path`）报告当前工作目录。
  term.parser.registerOscHandler(7, (data) => {
    try {
      emit("cwd", decodeURIComponent(new URL(data).pathname));
    } catch {
      // 序列格式不对：忽略。
    }
    return true;
  });
  term.attachCustomKeyEventHandler((e) => {
    // Cmd+F，或 Ctrl+Shift+F；这样单独的 Ctrl+F 仍能传给 shell。
    const findKey = e.key.toLowerCase() === "f" && (e.metaKey ? !e.ctrlKey : e.ctrlKey && e.shiftKey);
    if (e.type === "keydown" && findKey) {
      void openSearch();
      return false;
    }
    return true;
  });
  term.onData(write);
  term.onResize(({ cols, rows }) => api.termResize(props.sessionId, cols, rows).catch(() => {}));

  await openShell();
  observer = new ResizeObserver(refitSoon);
  observer.observe(host.value!);
  if (props.active) term.focus();
});

// 已重连：之前的输出保留，并与新 shell 的输出隔开。
watch(
  () => props.sessionId,
  async () => {
    await markReconnected();
    // 保持下来的 shell 仍有它的当前目录和钩子；只有新 shell 才需要重新设置。
    if ((await openShell()) === "fresh") emit("restarted");
  },
);

watch(
  () => props.active,
  async (active) => {
    setCurrentTerminal(useCommand, active);
    if (!active) return;
    await nextTick();
    refit();
    term.focus();
  },
  { immediate: true },
);

watch(
  () => app.settings.terminalFontSize,
  (size) => {
    term.options.fontSize = size;
    refit();
  },
);

// 换了字体，或者内置字体这时才加载完：字宽可能不一样，列数要重算。
watch(loadedMonoFamily, (family) => {
  term.options.fontFamily = family;
  refit();
});

watch(terminalTheme, (theme) => (term.options.theme = theme));

onBeforeUnmount(() => {
  setCurrentTerminal(useCommand, false);
  observer?.disconnect();
  clearTimeout(fitTimer);
  term.dispose();
});

defineExpose({ write, focus: () => term.focus() });
</script>

<template>
  <div class="terminal" :style="{ background: terminalTheme.background }">
    <div ref="host" class="host" @contextmenu="showContextMenu" />
    <div v-if="searching" class="search">
      <input
        ref="searchInput"
        v-model="query"
        :placeholder="t('term.find')"
        @keydown.enter.prevent="find(!$event.shiftKey)"
        @keydown.esc.prevent="closeSearch"
        @input="find(true)"
      />
      <IconButton size="tiny" :title="t('term.previous')" @click="find(false)"><ChevronUp :size="14" /></IconButton>
      <IconButton size="tiny" :title="t('term.next')" @click="find(true)"><ChevronDown :size="14" /></IconButton>
      <IconButton size="tiny" :title="t('common.close')" @click="closeSearch"><X :size="14" /></IconButton>
    </div>
  </div>
</template>

<style scoped>
.terminal {
  position: relative;
  height: 100%;
  padding: 6px 0 0 8px;
}

.host {
  height: 100%;
}

.search {
  position: absolute;
  top: 6px;
  right: 18px;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--bg-raised);
  box-shadow: 0 8px 24px -8px var(--shadow);
}

.search input {
  width: 180px;
  height: 24px;
  padding: 0 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  outline: none;
  background: var(--bg);
  color: var(--text);
  font: inherit;
}

.search input:focus {
  border-color: var(--accent);
}
</style>
