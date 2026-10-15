<script setup lang="ts">
// 文本编辑器：所有标签页共用一个 Monaco 实例，切换时换模型并恢复光标和滚动位置。
import { AArrowDown, AArrowUp, CornerDownRight, Lock, PanelLeft, Pilcrow, Replace, RotateCw, Save, SaveAll, Search, WrapText, X } from "@lucide/vue";
import type { editor as MonacoEditor } from "monaco-editor";
import { SplitterGroup, SplitterPanel, SplitterResizeHandle } from "reka-ui";
import { computed, markRaw, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from "vue";
import EditorFiles from "@/features/files/EditorFiles.vue";
import { nextActive } from "@/features/files/editorTree";
import { api, errorMessage } from "@/shared/api";
import { checkIcon, openContextMenu, shortcut } from "@/shared/contextMenu";
import { loadedMonoFamily } from "@/shared/fonts";
import { t } from "@/shared/i18n";
import { FONT_MAX, FONT_MIN, stepFont } from "@/shared/settings";
import { useApp } from "@/shared/store";
import { isDark } from "@/shared/theme";
import { confirm, message } from "@/shared/ui";
import { baseName, parentPath } from "@/shared/util";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiSpin from "@/shared/ui-kit/UiSpin.vue";
import { accentText, dangerText, dim, glassBar, line, toolGroup } from "@/shared/ui-kit/styles";

/** path 为初始打开的文件。 */
const props = defineProps<{ sessionId: string; path: string }>();
const emit = defineEmits<{ close: []; saved: [] }>();
const app = useApp();

/** 模型由 Monaco 管理，不放入响应式。 */
interface Tab {
  path: string;
  loading: boolean;
  failure: string | null;
  /** 无写权限时只读打开。 */
  readOnly: boolean;
  dirty: boolean;
  savedVersion: number;
  language: string;
  model: MonacoEditor.ITextModel | null;
  view: MonacoEditor.ICodeEditorViewState | null;
}

/** 语言菜单列出的常见语言；其他语言仍可由 Monaco 按文件名识别。 */
const LANGUAGES = [
  "plaintext", "shell", "ini", "json", "yaml", "xml", "html", "css", "scss", "less", "javascript", "typescript",
  "python", "go", "rust", "java", "c", "cpp", "csharp", "php", "ruby", "perl", "lua", "sql", "markdown", "dockerfile", "powershell", "bat",
];

const monacoTheme = () => (isDark.value ? "shx-dark" : "shx-light");
let monaco: (typeof import("@/features/files/monaco"))["monaco"] | undefined;
const ready = ref(false);
const host = ref<HTMLElement>();
const editor = shallowRef<MonacoEditor.IStandaloneCodeEditor>();
const tabs = ref<Tab[]>([]);
const activePath = ref<string | null>(null);
const active = computed(() => tabs.value.find((tab) => tab.path === activePath.value) ?? null);
const dirtyTabs = computed(() => tabs.value.filter((tab) => tab.dirty));
const saving = ref(false);
const root = ref(parentPath(props.path));

const status = reactive({ line: 1, column: 1, selected: 0, indent: "", eol: "LF" });

const side = ref<InstanceType<typeof SplitterPanel>>();
const sideOpen = ref(true);

function toggleSide() {
  if (side.value?.isCollapsed) side.value.expand();
  else side.value?.collapse();
}

const tabOf = (path: string) => tabs.value.find((tab) => tab.path === path);

function refreshStatus() {
  const ed = editor.value;
  const model = ed?.getModel();
  if (!ed || !model) return;
  const p = ed.getPosition();
  const selection = ed.getSelection();
  const opts = model.getOptions();
  Object.assign(status, {
    line: p?.lineNumber ?? 1,
    column: p?.column ?? 1,
    selected: selection ? model.getValueLengthInRange(selection) : 0,
    indent: opts.insertSpaces ? t("editor.spaces", { size: opts.tabSize }) : t("editor.tabs", { size: opts.tabSize }),
    eol: model.getEOL() === "\r\n" ? "CRLF" : "LF",
  });
}

/** 切换到 tab 的模型；未加载时清空。 */
function attach(tab: Tab | null) {
  const ed = editor.value;
  if (!ed) return;
  ed.setModel(tab?.model ?? null);
  if (!tab?.model) return;
  ed.updateOptions({ readOnly: tab.readOnly });
  if (tab.view) ed.restoreViewState(tab.view);
  refreshStatus();
  ed.focus();
}

function activate(path: string | null) {
  if (path === activePath.value) return;
  const current = active.value;
  if (current?.model && editor.value) current.view = editor.value.saveViewState();
  activePath.value = path;
  attach(active.value);
}

async function load(tab: Tab) {
  tab.loading = true;
  tab.failure = null;
  try {
    const file = await api.readText(props.sessionId, tab.path);
    const m = monaco!;
    const uri = m.Uri.file(tab.path);
    m.editor.getModel(uri)?.dispose();
    // 由 Monaco 按文件名识别语言。
    const model = markRaw(m.editor.createModel(file.content, undefined, uri));
    Object.assign(tab, { model, readOnly: !file.writable, savedVersion: model.getAlternativeVersionId(), dirty: false, language: model.getLanguageId(), view: null });
    model.onDidChangeContent(() => {
      tab.dirty = model.getAlternativeVersionId() !== tab.savedVersion;
      if (tab === active.value) refreshStatus();
    });
    model.onDidChangeLanguage(() => (tab.language = model.getLanguageId()));
  } catch (e) {
    tab.failure = errorMessage(e);
  } finally {
    tab.loading = false;
  }
  if (tab === active.value) attach(tab);
}

/** 已打开则切换，否则新开标签页。 */
function open(path: string) {
  if (!tabOf(path)) {
    tabs.value.push({ path, loading: true, failure: null, readOnly: false, dirty: false, savedVersion: 0, language: "", model: null, view: null });
    if (monaco) void load(tabOf(path)!);
  }
  activate(path);
}

async function save(tab = active.value, quiet = false): Promise<boolean> {
  const model = tab?.model;
  if (!tab || !model || tab.readOnly) return true;
  const version = model.getAlternativeVersionId();
  try {
    await api.writeText(props.sessionId, tab.path, model.getValue());
  } catch (e) {
    message.error(`${baseName(tab.path)}: ${t("common.saveFailed", { error: errorMessage(e) })}`);
    return false;
  }
  tab.savedVersion = version;
  tab.dirty = model.getAlternativeVersionId() !== version;
  if (!quiet) message.success(t("common.saved"));
  emit("saved");
  return true;
}

async function saveActive() {
  if (saving.value) return;
  saving.value = true;
  await save().finally(() => (saving.value = false));
}

async function saveAll() {
  if (saving.value || !dirtyTabs.value.length) return;
  saving.value = true;
  try {
    const results = await Promise.all(dirtyTabs.value.filter((tab) => !tab.readOnly).map((tab) => save(tab, true)));
    if (results.every(Boolean)) message.success(t("editor.savedAll"));
  } finally {
    saving.value = false;
  }
}

/** 重新读取文件，有未保存修改时先确认。 */
async function reload() {
  const tab = active.value;
  if (!tab || tab.loading) return;
  if (tab.dirty && !(await confirm(t("editor.reload"), t("editor.reloadBody", { name: baseName(tab.path) }), { danger: true, ok: t("editor.reload") }))) return;
  await load(tab);
}

async function closeTab(tab: Tab) {
  if (tab.dirty && !(await confirm(t("editor.discardTitle"), t("editor.discardBody", { name: baseName(tab.path) }), { danger: true, ok: t("editor.discardTitle") }))) return;
  const next = nextActive(tabs.value.map((x) => x.path), tab.path, activePath.value);
  if (tab === active.value) editor.value?.setModel(null);
  tabs.value = tabs.value.filter((x) => x !== tab);
  tab.model?.dispose();
  activePath.value = null;
  activate(next);
}

async function close() {
  const dirty = dirtyTabs.value;
  if (dirty.length === 1 && !(await confirm(t("editor.discardTitle"), t("editor.discardBody", { name: baseName(dirty[0].path) }), { danger: true, ok: t("editor.discardTitle") }))) return;
  if (dirty.length > 1 && !(await confirm(t("editor.discardTitle"), t("editor.discardAllBody", { count: dirty.length }), { danger: true, ok: t("editor.discardTitle") }))) return;
  emit("close");
}

function run(action: string) {
  const ed = editor.value;
  if (!ed?.getModel()) return;
  ed.focus();
  void ed.getAction(action)?.run();
}

function resizeFont(delta: number) {
  app.settings.editorFontSize = stepFont(app.settings.editorFontSize, delta);
}

function toggleEol() {
  const model = active.value?.model;
  if (!model || active.value?.readOnly || !monaco) return;
  const { EndOfLineSequence } = monaco.editor;
  model.pushEOL(model.getEOL() === "\r\n" ? EndOfLineSequence.LF : EndOfLineSequence.CRLF);
  refreshStatus();
}

function chooseLanguage(event: MouseEvent) {
  const model = active.value?.model;
  if (!model || !monaco) return;
  const m = monaco;
  const known = new Map(m.languages.getLanguages().map((lang) => [lang.id, lang.aliases?.[0] ?? lang.id]));
  const ids = LANGUAGES.filter((id) => known.has(id));
  if (!ids.includes(model.getLanguageId())) ids.unshift(model.getLanguageId());
  openContextMenu(event, [
    { group: t("editor.language") },
    ...ids.map((id) => ({
      label: known.get(id) ?? id,
      icon: checkIcon(id === model.getLanguageId()),
      action: () => m.editor.setModelLanguage(model, id),
    })),
  ]);
}

const languageName = computed(() => {
  const id = active.value?.language;
  if (!id || !ready.value || !monaco) return id ?? "";
  return monaco.languages.getLanguages().find((lang) => lang.id === id)?.aliases?.[0] ?? id;
});

onMounted(async () => {
  try {
    // 首次使用时加载，之后常驻。
    monaco = (await import("@/features/files/monaco")).monaco;
  } catch (e) {
    return void message.error(errorMessage(e));
  }
  if (!host.value) return;
  const m = monaco;
  const created = m.editor.create(host.value, {
    model: null,
    theme: monacoTheme(),
    readOnlyMessage: { value: t("editor.readOnlyHint") },
    fontSize: app.settings.editorFontSize,
    fontFamily: loadedMonoFamily.value,
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    // 滚动条样式见 base.css。
    scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10, useShadows: false },
    wordWrap: app.settings.editorWordWrap ? "on" : "off",
    renderWhitespace: app.settings.editorWhitespace ? "all" : "selection",
    renderControlCharacters: app.settings.editorWhitespace,
  });
  editor.value = created;
  ready.value = true;
  created.onDidChangeCursorSelection(refreshStatus);
  created.onDidChangeModelOptions(refreshStatus);
  created.addCommand(m.KeyMod.CtrlCmd | m.KeyCode.KeyS, () => void saveActive());
  created.addCommand(m.KeyMod.CtrlCmd | m.KeyMod.Alt | m.KeyCode.KeyS, () => void saveAll());
  sideOpen.value = !side.value?.isCollapsed;
  open(props.path);
  for (const tab of tabs.value) if (!tab.model && tab.loading) void load(tab);
});

watch(
  () => app.settings.editorFontSize,
  (fontSize) => editor.value?.updateOptions({ fontSize }),
);
watch(
  () => app.settings.editorWordWrap,
  (wrap) => editor.value?.updateOptions({ wordWrap: wrap ? "on" : "off" }),
);
watch(
  () => app.settings.editorWhitespace,
  (show) => editor.value?.updateOptions({ renderWhitespace: show ? "all" : "selection", renderControlCharacters: show }),
);
watch(loadedMonoFamily, (fontFamily) => editor.value?.updateOptions({ fontFamily }));
watch(isDark, () => monaco?.editor.setTheme(monacoTheme()));

onBeforeUnmount(() => {
  editor.value?.dispose();
  for (const tab of tabs.value) tab.model?.dispose();
});
</script>

<template>
  <Teleport to="body">
    <Transition appear enter-active-class="transition-opacity duration-200 ease-out" enter-from-class="opacity-0">
    <div class="fixed inset-0 z-40 flex items-center justify-center bg-zinc-900/25 dark:bg-black/45">
      <!-- 与 monaco.ts 的编辑区底色一致，加载前不闪烁。 -->
      <PopTransition appear kind="zoom">
      <div :class="['flex h-11/12 w-11/12 flex-col overflow-hidden rounded-2xl border border-white/70 bg-white shadow-2xl shadow-zinc-900/30 dark:border-white/10 dark:bg-zinc-900 dark:shadow-black/60']">
        <div :class="[glassBar, line, 'flex h-12 shrink-0 items-center gap-2 overflow-x-auto border-b px-2']" data-no-scrollbar>
          <div :class="toolGroup">
            <UiButton ghost size="small" :selected="sideOpen" @click="toggleSide"><template #icon><PanelLeft :size="14" /></template>{{ t("editor.files") }}</UiButton>
          </div>
          <div :class="toolGroup">
            <UiButton ghost size="small" :title="shortcut.mod('S')" :disabled="!active?.dirty || active.readOnly || saving" @click="saveActive"><template #icon><Save :size="14" /></template>{{ t("common.save") }}</UiButton>
            <UiButton ghost size="small" :disabled="!dirtyTabs.some((tab) => !tab.readOnly) || saving" @click="saveAll"><template #icon><SaveAll :size="14" /></template>{{ t("editor.saveAll") }}</UiButton>
            <UiButton ghost size="small" :disabled="!active || active.loading" @click="reload"><template #icon><RotateCw :size="14" /></template>{{ t("editor.reload") }}</UiButton>
          </div>
          <div :class="toolGroup">
            <UiButton ghost size="small" :title="shortcut.mod('F')" :disabled="!active?.model" @click="run('actions.find')"><template #icon><Search :size="14" /></template>{{ t("editor.find") }}</UiButton>
            <UiButton ghost size="small" :disabled="!active?.model || active.readOnly" @click="run('editor.action.startFindReplaceAction')"><template #icon><Replace :size="14" /></template>{{ t("editor.replace") }}</UiButton>
            <UiButton ghost size="small" :disabled="!active?.model" @click="run('editor.action.gotoLine')"><template #icon><CornerDownRight :size="14" /></template>{{ t("editor.gotoLine") }}</UiButton>
          </div>
          <div :class="toolGroup">
            <span class="pl-2.5 text-xs font-medium">{{ t("editor.fontSize") }}</span>
            <IconButton :title="t('editor.fontSmaller')" :disabled="app.settings.editorFontSize <= FONT_MIN" @click="resizeFont(-1)"><AArrowDown :size="15" /></IconButton>
            <span :class="[dim, 'w-5 text-center text-xs tabular-nums']">{{ app.settings.editorFontSize }}</span>
            <IconButton :title="t('editor.fontLarger')" :disabled="app.settings.editorFontSize >= FONT_MAX" @click="resizeFont(1)"><AArrowUp :size="15" /></IconButton>
          </div>
          <div :class="toolGroup">
            <UiButton ghost size="small" :selected="app.settings.editorWordWrap" @click="app.settings.editorWordWrap = !app.settings.editorWordWrap"><template #icon><WrapText :size="14" /></template>{{ t("editor.wordWrap") }}</UiButton>
            <UiButton ghost size="small" :selected="app.settings.editorWhitespace" @click="app.settings.editorWhitespace = !app.settings.editorWhitespace"><template #icon><Pilcrow :size="14" /></template>{{ t("editor.whitespace") }}</UiButton>
          </div>
          <span class="flex-1" />
          <span v-if="active?.readOnly" class="inline-flex h-6 shrink-0 items-center gap-1 rounded-md bg-amber-500/15 px-2 text-xs whitespace-nowrap text-amber-700 dark:text-amber-300" :title="t('editor.readOnlyHint')"><Lock :size="11" :stroke-width="2.2" />{{ t("editor.readOnly") }}</span>
          <IconButton :title="t('common.close')" @click="close"><X :size="16" /></IconButton>
        </div>

        <SplitterGroup direction="horizontal" auto-save-id="shx-editor" class="min-h-0 flex-1">
          <SplitterPanel ref="side" :default-size="24" :min-size="18" :max-size="45" collapsible :collapsed-size="0" class="overflow-hidden" @collapse="sideOpen = false" @expand="sideOpen = true">
            <EditorFiles v-model:root="root" :session-id="sessionId" :active="activePath" @open="open" />
          </SplitterPanel>
          <SplitterResizeHandle :class="[line, 'group relative w-px border-l data-[state=drag]:border-blue-500']">
            <span class="absolute inset-y-0 -right-1 -left-1 z-10 group-hover:bg-blue-500/20 group-data-[state=drag]:bg-blue-500/20" />
          </SplitterResizeHandle>
          <SplitterPanel :min-size="40" class="flex min-w-0 flex-col">
            <div :class="[line, 'flex h-9 shrink-0 items-end gap-0.5 overflow-x-auto border-b px-1.5']" data-no-scrollbar>
              <div
                v-for="tab in tabs"
                :key="tab.path"
                class="group -mb-px flex h-8 max-w-56 min-w-24 shrink-0 cursor-pointer items-center gap-1.5 rounded-t-lg border border-b-0 pr-1 pl-2.5 text-sm transition-colors duration-150"
                :class="tab.path === activePath
                  ? [line, 'bg-white font-medium text-zinc-900 dark:bg-zinc-900 dark:text-white']
                  : [dim, 'border-transparent hover:bg-zinc-900/5 hover:text-zinc-800 dark:hover:bg-white/8 dark:hover:text-zinc-100']"
                :title="tab.path"
                @click="activate(tab.path)"
                @mousedown.middle.prevent="closeTab(tab)"
              >
                <UiSpin v-if="tab.loading" :size="12" />
                <FileIcon v-else :name="baseName(tab.path)" :size="14" text class="shrink-0" />
                <span class="truncate" :class="tab.failure && 'line-through'">{{ baseName(tab.path) }}</span>
                <!-- 未保存时显示圆点，悬停变为关闭按钮。 -->
                <span class="relative ml-auto grid size-5 shrink-0 place-items-center">
                  <span v-if="tab.dirty" :class="[accentText, 'text-xs group-hover:invisible']">●</span>
                  <IconButton
                    class="absolute! inset-0"
                    :class="tab.dirty || tab.path !== activePath ? 'invisible group-hover:visible' : ''"
                    size="tiny"
                    :title="t('common.close')"
                    @click.stop="closeTab(tab)"
                  ><X :size="12" /></IconButton>
                </span>
              </div>
            </div>
            <div class="relative min-h-0 flex-1 bg-inherit">
              <div ref="host" class="h-full" />
              <div v-if="!ready || active?.loading" class="absolute inset-0 flex items-center justify-center bg-inherit p-6"><UiSpin :size="22" /></div>
              <div v-else-if="active?.failure" :class="[dangerText, 'absolute inset-0 flex items-center justify-center bg-inherit p-6 text-center break-all']">{{ active.failure }}</div>
              <div v-else-if="!active" :class="[dim, 'absolute inset-0 flex items-center justify-center bg-inherit p-6 text-sm']">{{ t("editor.noFile") }}</div>
            </div>
          </SplitterPanel>
        </SplitterGroup>

        <div :class="[glassBar, line, dim, 'flex h-7 shrink-0 items-center gap-4 border-t px-3 text-xs whitespace-nowrap']">
          <span class="min-w-0 flex-1 truncate font-mono" :title="active?.path">{{ active?.path }}</span>
          <template v-if="active?.model">
            <span class="tabular-nums">{{ t("editor.position", { line: status.line, column: status.column }) }}<template v-if="status.selected">{{ t("editor.selection", { count: status.selected }) }}</template></span>
            <span>{{ status.indent }}</span>
            <button type="button" class="cursor-pointer hover:text-zinc-800 disabled:cursor-default dark:hover:text-zinc-100" :disabled="active.readOnly" @click="toggleEol">{{ status.eol }}</button>
            <span>UTF-8</span>
            <button type="button" class="cursor-pointer hover:text-zinc-800 dark:hover:text-zinc-100" :title="t('editor.language')" @click="chooseLanguage">{{ languageName }}</button>
          </template>
        </div>
      </div>
      </PopTransition>
    </div>
    </Transition>
  </Teleport>
</template>
