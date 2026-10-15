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
import { api, errorMessage } from "@/shared/api";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { openContextMenu } from "@/shared/contextMenu";
import { loadedMonoFamily } from "@/shared/fonts";
import { t } from "@/shared/i18n";
import { confirm, message } from "@/shared/ui";
import { bindings, formatAccelerator, setShortcutTerminal, type TerminalOps } from "@/app/shortcuts";
import { useApp } from "@/shared/store";
import { openAssistant, setCurrentTerminal, type UseCommand } from "@/features/commands/commandAssistant";
import { terminalSurface, terminalTheme } from "@/features/session/terminalTheme";
import { VIBRANCY } from "@/shared/titlebar";
import { fg, glassPop } from "@/shared/ui-kit/styles";

const props = defineProps<{ sessionId: string; /** 保活时的 tmux 会话名。 */ keepAlive: string | null; active: boolean }>();
const emit = defineEmits<{ cwd: [path: string]; /** 重连后为新 shell。 */ restarted: []; /** 是否运行在保活会话中。 */ attached: [on: boolean] }>();
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
  // 清屏时内容移入回滚缓冲区。
  scrollOnEraseInDisplay: true,
  allowProposedApi: true,
  macOptionIsMeta: true,
  allowTransparency: VIBRANCY,
  theme: terminalTheme.value,
});
const fit = new FitAddon();
const search = new SearchAddon();
let observer: ResizeObserver | undefined;

/** 重置被断线中断的程序遗留的终端模式（鼠标上报、括号粘贴、滚动区域等），不动光标和屏幕内容。 */
const RESET_MODES = "\x1b[?1000;1002;1003;1006;1004;2004l\x1b[?1l\x1b>\x1b[?7h\x1b[?25h\x1b7\x1b[?6l\x1b[r\x1b8\x1b[0m\x1b(B";

/** 分隔断线前后的输出。 */
function markReconnected(): Promise<void> {
  // 若有全屏程序，切回主屏幕。
  const leaveAlternate = term.buffer.active.type === "alternate" ? "\x1b[?1049l" : "";
  return new Promise((resolve) => {
    term.write(leaveAlternate + RESET_MODES, () => {
      // 画在所有内容下方。
      const buffer = term.buffer.active;
      let last = term.rows - 1;
      while (last > buffer.cursorY && !buffer.getLine(buffer.baseY + last)?.translateToString(true)) last--;
      term.write(`\x1b[${last + 1};1H\r\n\x1b[2m${t("term.reconnected")}\x1b[0m\r\n`, resolve);
    });
  });
}

/** 每个标签页只询问一次安装 tmux。 */
let installOffered = false;

/** 准备 tmux 会话；无 tmux（提议安装）或检查失败时返回 null。 */
async function prepareKeepAlive(id: string, name: string): Promise<{ fresh: boolean } | null> {
  try {
    const state = await api.keepAlivePrepare(id, name, term.cols, term.rows);
    if (state.ready) return { fresh: state.fresh };
    if (!installOffered) {
      installOffered = true;
      // 不等待：安装命令在随后打开的普通 shell 中执行。
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
  // 回答可能很晚才到，只写入当初的 shell。
  if (!ok || id !== props.sessionId) return;
  write(` ${command}\r`);
  term.focus();
}

/** 启动 shell 并接入输出。 */
async function openShell(): Promise<"fresh" | "resumed" | "failed"> {
  const id = props.sessionId;
  const kept = props.keepAlive ? await prepareKeepAlive(id, props.keepAlive) : null;
  // 准备期间连接已更换，交由新连接处理。
  if (id !== props.sessionId) return "failed";
  // tmux 会重绘全屏，先把现有内容移入回滚缓冲区。
  if (kept) term.write("\x1b[2J\x1b[H");
  try {
    await api.termOpen(
      id,
      term.cols,
      term.rows,
      (bytes) => {
        // 丢弃旧会话的迟到输出。
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
  // 隐藏时无尺寸，适配会压成 1 列。
  if (host.value?.offsetWidth && host.value.offsetHeight) fit.fit();
}

/** 适配节流间隔（毫秒）。 */
const FIT_INTERVAL = 60;
let fitTimer: ReturnType<typeof setTimeout> | undefined;
let lastFit = 0;

/** 节流适配尺寸：每次适配会重排回滚缓冲区并让 tmux 重绘整屏。首次立即执行，结束后补一次。 */
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
  // 剪贴板非文本时读取失败。
  const text = await readText().catch(() => "");
  if (!text) return void message.info(t("term.noText"));
  // 括号粘贴模式，多行不会被逐行执行。
  term.paste(text);
  term.focus();
}

const useCommand: UseCommand = (command, run) => {
  // 以粘贴方式输入，多行命令整体到达。
  term.paste(command);
  if (run) write("\r");
  // 等对话框释放焦点后再聚焦。
  setTimeout(() => term.focus(), 100);
};

function showContextMenu(event: MouseEvent) {
  openContextMenu(event, [
    { label: t("cmd.assistantMenu"), icon: SquareChevronRight, action: () => openAssistant(useCommand) },
    "divider",
    { label: t("common.copy"), icon: Copy, hint: formatAccelerator(bindings.value["term.copy"]), disabled: !term.hasSelection(), action: copySelection },
    { label: t("common.paste"), icon: ClipboardPaste, hint: formatAccelerator(bindings.value["term.paste"]), action: pasteClipboard },
    "divider",
    { label: t("common.selectAll"), icon: TextSelect, hint: formatAccelerator(bindings.value["term.selectAll"]), action: () => term.selectAll() },
    { label: t("term.clear"), icon: Eraser, hint: formatAccelerator(bindings.value["term.clear"]), action: () => term.clear() },
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

/** 注册给快捷键分发器的终端操作。 */
const TERMINAL_OPS: TerminalOps = {
  copy: () => void copySelection(),
  paste: () => void pasteClipboard(),
  selectAll: () => term.selectAll(),
  clear: () => term.clear(),
  openSearch: () => void openSearch(),
};

onMounted(async () => {
  term.loadAddon(fit);
  term.loadAddon(search);
  term.loadAddon(new Unicode11Addon());
  term.loadAddon(new WebLinksAddon((_event, uri) => void openUrl(uri)));
  term.unicode.activeVersion = "11";
  term.open(host.value!);
  // 毛玻璃下不用 WebGL：macOS WebKit 会把透明窗口中 WebGL 画布的透明部分画成黑色。
  if (!VIBRANCY) {
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch {
      // 不支持 WebGL 时回退 DOM 渲染器。
    }
  }
  refit();

  // OSC 7 报告工作目录。
  term.parser.registerOscHandler(7, (data) => {
    try {
      emit("cwd", decodeURIComponent(new URL(data).pathname));
    } catch {
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

// 重连：保留旧输出并加分隔。
watch(
  () => props.sessionId,
  async () => {
    await markReconnected();
    // 只有新 shell 需要重新设置钩子。
    if ((await openShell()) === "fresh") emit("restarted");
  },
);

watch(
  () => props.active,
  async (active) => {
    setCurrentTerminal(useCommand, active);
    setShortcutTerminal(TERMINAL_OPS, active);
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

// 字体变化时字宽可能不同，需重新适配。
watch(loadedMonoFamily, (family) => {
  term.options.fontFamily = family;
  refit();
});

watch(terminalTheme, (theme) => (term.options.theme = theme));

onBeforeUnmount(() => {
  setCurrentTerminal(useCommand, false);
  setShortcutTerminal(TERMINAL_OPS, false);
  observer?.disconnect();
  clearTimeout(fitTimer);
  term.dispose();
});

defineExpose({ write, focus: () => term.focus() });
</script>

<template>
  <div class="relative h-full pt-2 pl-2" :style="{ background: terminalSurface }">
    <div ref="host" class="h-full" @contextmenu="showContextMenu" />
    <div v-if="searching" :class="[glassPop, 'absolute top-2 right-5 z-10 flex items-center gap-1 rounded-xl p-1']">
      <input
        :class="[fg, 'h-7 w-45 rounded-lg border border-zinc-900/10 bg-white/70 px-2 text-sm outline-none placeholder:text-zinc-400 focus:border-blue-500 focus:ring-3 focus:ring-blue-500/20 dark:border-white/10 dark:bg-white/5 dark:placeholder:text-zinc-500']"
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
