<script setup lang="ts">
import { Lock, Save, X } from "@lucide/vue";
import { NButton, NSpin } from "naive-ui";
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { api, errorMessage } from "../../shared/api";
import { t } from "../../shared/i18n";
import { isDark } from "../../shared/theme";
import { confirm, message } from "../../shared/ui";
import { baseName } from "../../shared/util";
import { useApp } from "../../stores/app";

const props = defineProps<{ sessionId: string; path: string }>();
const emit = defineEmits<{ close: []; saved: [] }>();
const app = useApp();

type Monaco = typeof import("./monaco");
/** Monaco 自带的配色里，与应用当前明暗主题对应的那一个。 */
const monacoTheme = () => (isDark.value ? "vs-dark" : "vs");
let monacoApi: Monaco["monaco"] | undefined;
const host = ref<HTMLElement>();
const loading = ref(true);
const saving = ref(false);
const dirty = ref(false);
/** 当前用户写不了这个文件：只能查看，不让改了半天才发现存不了。 */
const readOnly = ref(false);
const failure = ref<string | null>(null);
const language = ref("");
const position = ref("");
const editor = shallowRef<import("monaco-editor").editor.IStandaloneCodeEditor>();
let savedVersion = 0;

async function save() {
  const model = editor.value?.getModel();
  if (!model || saving.value || readOnly.value) return;
  saving.value = true;
  try {
    const version = model.getAlternativeVersionId();
    await api.writeText(props.sessionId, props.path, model.getValue());
    savedVersion = version;
    dirty.value = model.getAlternativeVersionId() !== savedVersion;
    message.success(t("common.saved"));
    emit("saved");
  } catch (e) {
    message.error(t("common.saveFailed", { error: errorMessage(e) }));
  } finally {
    saving.value = false;
  }
}

async function close() {
  if (dirty.value && !(await confirm(t("editor.discardTitle"), t("editor.discardBody", { name: baseName(props.path) }), { danger: true, ok: t("editor.discardTitle") }))) return;
  emit("close");
}

onMounted(async () => {
  let m: Monaco;
  let text: string;
  try {
    // Monaco 首次使用时加载，之后一直保留，后面打开文件不用再等。
    const [loaded, file] = await Promise.all([import("./monaco"), api.readText(props.sessionId, props.path)]);
    [m, text] = [loaded, file.content];
    readOnly.value = !file.writable;
  } catch (e) {
    failure.value = errorMessage(e);
    loading.value = false;
    return;
  }
  loading.value = false;
  if (!host.value) return;

  const { monaco } = m;
  monacoApi = monaco;
  const uri = monaco.Uri.file(props.path);
  monaco.editor.getModel(uri)?.dispose();
  const model = monaco.editor.createModel(text, m.languageFor(baseName(props.path)), uri);
  language.value = model.getLanguageId();
  const created = monaco.editor.create(host.value, {
    model,
    theme: monacoTheme(),
    readOnly: readOnly.value,
    // 在只读的文件里打字时，Monaco 在光标处给出的说明。
    readOnlyMessage: { value: t("editor.readOnlyHint") },
    fontSize: app.settings.editorFontSize,
    automaticLayout: true,
    minimap: { enabled: text.length > 20_000 },
    scrollBeyondLastLine: false,
    // 滚动条的粗细和应用其他地方一致；样子在 styles.css 里统一。
    scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10, useShadows: false },
    renderWhitespace: "selection",
  });
  editor.value = created;
  savedVersion = model.getAlternativeVersionId();
  model.onDidChangeContent(() => (dirty.value = model.getAlternativeVersionId() !== savedVersion));
  created.onDidChangeCursorPosition(({ position: p }) => (position.value = t("editor.position", { line: p.lineNumber, column: p.column })));
  created.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => void save());
  created.focus();
});

watch(
  () => app.settings.editorFontSize,
  (fontSize) => editor.value?.updateOptions({ fontSize }),
);

watch(isDark, () => monacoApi?.editor.setTheme(monacoTheme()));

onBeforeUnmount(() => {
  const model = editor.value?.getModel();
  editor.value?.dispose();
  model?.dispose();
});
</script>

<template>
  <Teleport to="body">
    <div class="backdrop">
      <div class="window" :class="{ light: !isDark }">
        <div class="head">
          <span class="ellipsis path" :title="path">{{ path }}<span v-if="dirty" class="dirty"> ●</span></span>
          <span v-if="readOnly" class="read-only" :title="t('editor.readOnlyHint')"><Lock :size="11" :stroke-width="2.2" />{{ t("editor.readOnly") }}</span>
          <NButton v-else size="tiny" type="primary" :disabled="!dirty" :loading="saving" @click="save">
            <template #icon><Save :size="13" /></template>{{ t("common.save") }}
          </NButton>
          <button class="icon-btn" :title="t('common.close')" @click="close"><X :size="15" /></button>
        </div>
        <div class="body">
          <div ref="host" class="host" />
          <div v-if="loading" class="cover"><NSpin :size="22" /></div>
          <div v-else-if="failure" class="cover failure">{{ failure }}</div>
        </div>
        <div class="status dim">
          <span>{{ language }}</span>
          <span>{{ position }}</span>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 1500;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(0 0 0 / 40%);
}

.window {
  width: 90vw;
  height: 88vh;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: 14px;
  overflow: hidden;
  /* Monaco 两套配色各自的编辑区底色：编辑器加载出来之前就是这个颜色。 */
  background: #1e1e1e;
  box-shadow: 0 24px 60px -12px var(--shadow);
}

.window.light {
  background: #fffffe;
}

.head {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 8px 0 14px;
  background: var(--bg-raised);
  border-bottom: 1px solid var(--border);
}

.path {
  flex: 1;
}

.dirty {
  color: var(--accent);
}

/* 占着保存按钮的位置：这个文件存不了。 */
.read-only {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 22px;
  padding: 0 8px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--pin) 15%, transparent);
  color: var(--pin);
  font-size: 12px;
  white-space: nowrap;
}

.body {
  position: relative;
  flex: 1;
  min-height: 0;
  background: inherit;
}

.host {
  height: 100%;
}

.cover {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: inherit;
}

.failure {
  color: var(--danger);
}

.status {
  display: flex;
  justify-content: space-between;
  height: 22px;
  padding: 0 12px;
  line-height: 22px;
  font-size: 11px;
  background: var(--bg-raised);
  border-top: 1px solid var(--border);
}
</style>
