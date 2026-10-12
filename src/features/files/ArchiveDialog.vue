<script setup lang="ts">
// “添加到压缩包”对话框：压缩包的名称和格式、压缩等级、选项，一行一项。
// 打包下载时也用它询问打包方式。
import { FileArchive, FolderArchive } from "@lucide/vue";
import { NButton, NCheckbox, NInput, NModal, NRadioButton, NRadioGroup, NSelect, NSpace } from "naive-ui";
import { computed, nextTick, reactive, ref } from "vue";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import { t } from "../../shared/i18n";
import { DEFAULT_FORMAT, FORMATS, levelFor, levelNumber, LEVELS, mayGrow, splitName, type Level } from "./archive";

export interface ArchiveRequest {
  /** `compress`：压缩包写在数据旁边；`download`：为下载而打包，可选项更少。 */
  mode: "compress" | "download";
  /** 被打包条目所在的文件夹。 */
  dir: string;
  /** 要打包的条目。 */
  names: string[];
  /** 压缩包的初始名称，不含扩展名。 */
  suggested: string;
  /** `dir` 中已有的名字，用于在覆盖前提醒。 */
  existing?: string[];
}

export interface ArchiveAnswer {
  archive: string;
  /** 压缩级别 0–9；null 表示使用工具的默认级别。 */
  level: number | null;
  removeSource: boolean;
}

const show = ref(false);
const request = ref<ArchiveRequest | null>(null);
const nameInput = ref<InstanceType<typeof NInput>>();
/** `name` 是压缩包名称中扩展名之前的部分；扩展名由格式 `ext` 决定，显示在它右边。 */
const form = reactive({ name: "", ext: DEFAULT_FORMAT.ext, level: "normal" as Level, removeSource: false });
let settle: ((answer: ArchiveAnswer | null) => void) | undefined;

/** 对话框再次打开时沿用上次的格式和级别。 */
const last = { ext: DEFAULT_FORMAT.ext, level: "normal" as Level };

/** 内容一行里最多列出多少个名字；其余的由标签上的项数交代。 */
const NAME_LIMIT = 20;

const format = computed(() => FORMATS.find((f) => f.ext === form.ext) ?? DEFAULT_FORMAT);
const formatOptions = FORMATS.map((f) => ({ value: f.ext, label: f.ext.slice(1) }));
/** 压缩包的完整名称。 */
const archive = computed(() => (form.name.trim() ? form.name.trim() + form.ext : ""));
const invalid = computed(() => !archive.value || archive.value.includes("/"));
const replaces = computed(() => request.value?.mode === "compress" && !!request.value.existing?.includes(archive.value));
const content = computed(() => {
  const names = request.value?.names ?? [];
  return names.slice(0, NAME_LIMIT).join(t("common.listSeparator")) + (names.length > NAME_LIMIT ? "…" : "");
});

function setFormat(ext: string) {
  form.ext = ext;
  form.level = levelFor(format.value, form.level);
}

/** 名称末尾是一个还可能接着写下去的扩展名（".tar"）：先留在名称里，等输入停下再归位。 */
let unsettled = false;

/** 把名称末尾的压缩包扩展名移到格式那一栏。 */
function moveExtension() {
  const { stem, format: typed } = splitName(form.name);
  if (!typed) return;
  form.name = stem;
  setFormat(typed.ext);
}

/** 连扩展名一起输入（或粘贴）了完整的名称时，扩展名归到格式那一栏。 */
function onName(value: string) {
  form.name = value;
  const typed = splitName(value).format;
  unsettled = !!typed && mayGrow(typed);
  if (!unsettled) moveExtension();
}

function settleName() {
  if (unsettled) moveExtension();
  unsettled = false;
}

/** 打开对话框；返回用户的选择，关闭时返回 null。 */
function ask(req: ArchiveRequest): Promise<ArchiveAnswer | null> {
  settle?.(null);
  request.value = req;
  form.ext = last.ext;
  form.level = levelFor(format.value, last.level);
  form.name = req.suggested;
  form.removeSource = false;
  unsettled = false;
  show.value = true;
  void nextTick(() => {
    nameInput.value?.focus();
    nameInput.value?.select();
  });
  return new Promise((resolve) => (settle = resolve));
}

function finish(answer: ArchiveAnswer | null) {
  show.value = false;
  settle?.(answer);
  settle = undefined;
}

function submit() {
  settleName();
  if (invalid.value) return;
  Object.assign(last, { ext: form.ext, level: form.level });
  finish({ archive: archive.value, level: levelNumber(form.level), removeSource: request.value?.mode === "compress" && form.removeSource });
}

defineExpose({ ask });
</script>

<template>
  <NModal
    :show="show"
    preset="card"
    style="width: 480px"
    :bordered="false"
    :mask-closable="false"
    @update:show="(open: boolean) => !open && finish(null)"
  >
    <template v-if="request" #header>
      <DialogHeader
        :icon="request.mode === 'download' ? FolderArchive : FileArchive"
        :title="t(request.mode === 'download' ? 'zip.titleDownload' : 'zip.title')"
        :detail="request.mode === 'download' ? t('zip.whereDownload') : t('zip.where', { dir: request.dir })"
      />
    </template>
    <div v-if="request" class="archive" @keydown.enter="submit">
      <label>{{ t("zip.archive") }}</label>
      <div>
        <div class="name">
          <NInput ref="nameInput" :value="form.name" :status="form.name.trim() && invalid ? 'error' : undefined" @update:value="onName" @blur="settleName" />
          <span class="dot">.</span>
          <NSelect :value="form.ext" :options="formatOptions" class="ext" :title="t('zip.format')" @update:value="setFormat" />
        </div>
        <div v-if="replaces" class="warn">{{ t("zip.exists") }}</div>
      </div>

      <!-- 四个级别始终都在，所选格式不支持的不能选：换格式时这一行不会跳动。 -->
      <label>{{ t("zip.level") }}</label>
      <NRadioGroup v-model:value="form.level" class="levels">
        <NRadioButton v-for="level in LEVELS" :key="level" :value="level" :disabled="!format.levels.includes(level)">
          {{ t(`zip.level.${level}`) }}
        </NRadioButton>
      </NRadioGroup>

      <label>{{ t("zip.content", { count: request.names.length }) }}</label>
      <div class="ellipsis content" :title="request.names.join('\n')">{{ content }}</div>

      <template v-if="request.mode === 'compress'">
        <label>{{ t("zip.options") }}</label>
        <NCheckbox v-model:checked="form.removeSource" class="option">{{ t("zip.removeSource") }}</NCheckbox>
      </template>
    </div>
    <template #footer>
      <NSpace justify="end">
        <NButton @click="finish(null)">{{ t("common.cancel") }}</NButton>
        <NButton type="primary" :disabled="invalid" @click="submit">{{ t("common.ok") }}</NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
/* 左边一列标签，右边是各自的内容；每一行和输入框一样高，标签对着第一行。 */
.archive {
  --row: 34px;
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  align-items: start;
  gap: 12px 16px;
}

label {
  color: var(--text-dim);
  line-height: var(--row);
}

/* 文件名 . 扩展名：扩展名就是压缩格式。 */
.name {
  display: flex;
  align-items: center;
  gap: 6px;
}

.dot {
  color: var(--text-dim);
  font-weight: 600;
}

.ext {
  flex: 0 0 112px;
}

.warn {
  margin-top: 5px;
  color: var(--pin);
  font-size: 12px;
}

/* 四个级别平分整行。 */
.levels {
  display: flex;
}

.levels :deep(.n-radio-button) {
  flex: 1;
  padding: 0;
  text-align: center;
}

.content {
  line-height: var(--row);
}

.option {
  min-height: var(--row);
  align-items: center;
}
</style>
