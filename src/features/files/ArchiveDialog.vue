<script setup lang="ts">
// “添加到压缩包”对话框，打包下载时也复用。
import { FileArchive, FolderArchive } from "@lucide/vue";
import { computed, nextTick, reactive, ref } from "vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import SegmentedControl from "@/shared/ui-kit/SegmentedControl.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiSelect from "@/shared/ui-kit/UiSelect.vue";
import { t } from "@/shared/i18n";
import { DEFAULT_FORMAT, FORMATS, levelFor, levelNumber, LEVELS, mayGrow, splitName, type Level } from "@/features/files/archive";
import { dim, warnText } from "@/shared/ui-kit/styles";

export interface ArchiveRequest {
  /** compress：在原处生成压缩包；download：为下载打包，选项更少。 */
  mode: "compress" | "download";
  dir: string;
  names: string[];
  /** 初始名称，不含扩展名。 */
  suggested: string;
  /** 已有文件名，用于覆盖提醒。 */
  existing?: string[];
}

export interface ArchiveAnswer {
  archive: string;
  /** 0–9，null 为工具默认级别。 */
  level: number | null;
  removeSource: boolean;
}

const show = ref(false);
const request = ref<ArchiveRequest | null>(null);
const nameInput = ref<InstanceType<typeof UiInput>>();
/** name 不含扩展名，扩展名由 ext 决定。 */
const form = reactive({ name: "", ext: DEFAULT_FORMAT.ext, level: "normal" as Level, removeSource: false });
let settle: ((answer: ArchiveAnswer | null) => void) | undefined;

/** 记住上次的格式和级别。 */
const last = { ext: DEFAULT_FORMAT.ext, level: "normal" as Level };

/** 最多列出的名称数。 */
const NAME_LIMIT = 20;

const format = computed(() => FORMATS.find((f) => f.ext === form.ext) ?? DEFAULT_FORMAT);
const levelOptions = computed(() => LEVELS.map((level) => ({ value: level, label: t(`zip.level.${level}`), disabled: !format.value.levels.includes(level) })));
const formatOptions = FORMATS.map((f) => ({ value: f.ext, label: f.ext.slice(1) }));
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

/** 末尾扩展名可能还会延长（如 .tar），等输入停顿后再归位。 */
let unsettled = false;

/** 把名称末尾的扩展名移到格式栏。 */
function moveExtension() {
  const { stem, format: typed } = splitName(form.name);
  if (!typed) return;
  form.name = stem;
  setFormat(typed.ext);
}

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

/** 关闭时返回 null。 */
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
  <UiModal :show="show" :mask-closable="false" class="w-md" @update:show="(open: boolean) => !open && finish(null)">
    <template v-if="request" #header>
      <DialogHeader
        :icon="request.mode === 'download' ? FolderArchive : FileArchive"
        :title="t(request.mode === 'download' ? 'zip.titleDownload' : 'zip.title')"
        :detail="request.mode === 'download' ? t('zip.whereDownload') : t('zip.where', { dir: request.dir })"
      />
    </template>
    <div v-if="request" class="flex flex-col gap-3 *:flex *:items-start *:gap-4" @keydown.enter="submit">
      <div>
        <label :class="[dim, 'w-24 shrink-0 leading-9']">{{ t("zip.archive") }}</label>
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <UiInput ref="nameInput" class="min-w-0 flex-1" :value="form.name" :status="form.name.trim() && invalid ? 'error' : undefined" @update:value="onName" @blur="settleName" />
            <span :class="[dim, 'font-semibold']">.</span>
            <UiSelect :value="form.ext" :options="formatOptions" class="w-28! shrink-0" :title="t('zip.format')" @update:value="(v: string | number) => setFormat(String(v))" />
          </div>
          <div v-if="replaces" :class="[warnText, 'mt-1 text-xs']">{{ t("zip.exists") }}</div>
        </div>
      </div>

      <!-- 始终显示全部级别，不支持的禁用，避免切换格式时布局跳动。 -->
      <div>
        <label :class="[dim, 'w-24 shrink-0 leading-9']">{{ t("zip.level") }}</label>
        <SegmentedControl v-model="form.level" class="min-w-0 flex-1" :options="levelOptions" :label="t('zip.level')" />
      </div>

      <div>
        <label :class="[dim, 'w-24 shrink-0 leading-9']">{{ t("zip.content", { count: request.names.length }) }}</label>
        <div class="min-w-0 flex-1 truncate leading-9" :title="request.names.join('\n')">{{ content }}</div>
      </div>

      <div v-if="request.mode === 'compress'">
        <label :class="[dim, 'w-24 shrink-0 leading-9']">{{ t("zip.options") }}</label>
        <UiCheckbox v-model:checked="form.removeSource" class="min-h-9 items-center">{{ t("zip.removeSource") }}</UiCheckbox>
      </div>
    </div>
    <template #footer>
      <div class="flex justify-end gap-2">
        <UiButton @click="finish(null)">{{ t("common.cancel") }}</UiButton>
        <UiButton type="primary" :disabled="invalid" @click="submit">{{ t("common.ok") }}</UiButton>
      </div>
    </template>
  </UiModal>
</template>
