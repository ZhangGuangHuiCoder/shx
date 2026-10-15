<script setup lang="ts">
import { Files } from "@lucide/vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import { computed, ref, watch } from "vue";
import { api, errorMessage, type FileEntry } from "@/shared/api";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import { t, type MessageKey } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { formatMode, formatSize, formatTime } from "@/shared/util";
import { dim, focusRing, line, well } from "@/shared/ui-kit/styles";

// 权限位开关样式。
const permBit = `grid cursor-pointer place-items-center rounded-lg border p-0 font-mono text-sm transition-colors duration-150 ${focusRing}`;
const permOn = "border-blue-500/50 bg-blue-500/12 font-semibold text-blue-600 dark:bg-blue-400/15 dark:text-blue-300";
const permOff = "border-zinc-900/10 bg-transparent text-zinc-500 hover:border-zinc-900/25 hover:text-zinc-800 dark:border-white/10 dark:text-zinc-400 dark:hover:border-white/25 dark:hover:text-zinc-100";

const props = defineProps<{ sessionId: string; items: FileEntry[] }>();
const emit = defineEmits<{ changed: [] }>();
const show = defineModel<boolean>("show", { required: true });

const mode = ref(0o644);
const octal = ref("644");
const owner = ref("");
const group = ref("");
const recursive = ref(false);
const applying = ref(false);
const treeSize = ref<string | null>(null);
const measuring = ref(false);

const single = computed(() => (props.items.length === 1 ? props.items[0] : null));
const hasDir = computed(() => props.items.some((e) => e.kind === "dir"));
const initial = computed(() => {
  const first = props.items[0];
  return {
    mode: (first?.mode ?? 0o644) & 0o7777,
    owner: single.value ? single.value.owner ?? String(single.value.uid ?? "") : "",
    group: single.value ? single.value.group ?? String(single.value.gid ?? "") : "",
  };
});

watch(show, (visible) => {
  if (!visible) return;
  setMode(initial.value.mode);
  owner.value = initial.value.owner;
  group.value = initial.value.group;
  recursive.value = false;
  treeSize.value = null;
});

function setMode(value: number) {
  mode.value = value;
  octal.value = value.toString(8).padStart(3, "0");
}

function onOctal(text: string) {
  octal.value = text;
  if (/^[0-7]{3,4}$/.test(text)) mode.value = parseInt(text, 8);
}

/** 常用权限预设。 */
const PRESETS = [0o644, 0o600, 0o755, 0o700, 0o775];
const WHO: MessageKey[] = ["props.owner", "props.groupClass", "props.others"];
const WHAT: MessageKey[] = ["props.read", "props.write", "props.execute"];
const bit = (who: number, what: number) => 1 << ((2 - who) * 3 + (2 - what));

const octalValid = computed(() => /^[0-7]{3,4}$/.test(octal.value));
const modeChanged = computed(() => mode.value !== initial.value.mode || props.items.length > 1 || recursive.value);
const ownerChanged = computed(() => owner.value.trim() !== "" && (owner.value !== initial.value.owner || group.value !== initial.value.group));

async function measure() {
  measuring.value = true;
  try {
    treeSize.value = formatSize(await api.du(props.sessionId, single.value!.path));
  } catch (e) {
    message.error(errorMessage(e));
  } finally {
    measuring.value = false;
  }
}

async function apply() {
  const paths = props.items.map((e) => e.path);
  applying.value = true;
  try {
    if (ownerChanged.value) await api.chown(props.sessionId, paths, owner.value.trim(), group.value.trim(), recursive.value);
    if (modeChanged.value) await api.chmod(props.sessionId, paths, mode.value, recursive.value);
    emit("changed");
    show.value = false;
  } catch (e) {
    message.error(errorMessage(e));
  } finally {
    applying.value = false;
  }
}
</script>

<template>
  <UiModal v-model:show="show" class="w-lg">
    <template #header>
      <DialogHeader :title="single ? single.name : t('props.items', { count: items.length })" :detail="single ? single.path : t('props.title')">
        <template #icon>
          <FileIcon v-if="single" :name="single.name" :is-dir="single.isDir" :kind="single.kind" :size="19" />
          <Files v-else :size="18" />
        </template>
      </DialogHeader>
    </template>

    <div class="flex flex-col gap-5">
      <div v-if="single" :class="[line, well, 'grid grid-cols-4 items-baseline gap-x-5 gap-y-1.5 rounded-xl border px-4 py-3 *:even:col-span-3']">
        <span :class="dim">{{ t("props.type") }}</span><span>{{ t(`kind.${single.kind}`) }}</span>
        <template v-if="single.linkTarget">
          <span :class="dim">{{ t("props.target") }}</span><span class="break-all select-text">{{ single.linkTarget }}</span>
        </template>
        <span :class="dim">{{ t("props.size") }}</span>
        <span>
          <template v-if="single.kind === 'dir'">
            {{ treeSize ?? "" }}
            <UiButton v-if="!treeSize" ghost type="primary" size="tiny" :loading="measuring" @click="measure">{{ t("props.calculate") }}</UiButton>
          </template>
          <template v-else>{{ formatSize(single.size) }}<span :class="dim">{{ t("props.bytes", { count: single.size.toLocaleString() }) }}</span></template>
        </span>
        <span :class="dim">{{ t("props.modified") }}</span><span>{{ formatTime(single.mtime) }}</span>
      </div>

      <section>
        <h4 :class="[dim, 'mx-0 mt-0 mb-2.5 text-xs font-semibold']">{{ t("props.permissions") }}</h4>
        <!-- 左侧权限位，右侧同一值的其他写法及预设，两边联动。 -->
        <div class="flex items-start gap-5">
          <div class="grid shrink-0 grid-cols-4 items-center gap-1.5">
            <span />
            <span v-for="what in WHAT" :key="what" :class="[dim, 'text-center text-xs']">{{ t(what) }}</span>
            <template v-for="(who, w) in WHO" :key="who">
              <span class="pr-3 text-sm">{{ t(who) }}</span>
              <button
                v-for="(what, a) in WHAT"
                :key="what"
                type="button"
                :class="[permBit, 'h-8 w-11', mode & bit(w, a) ? permOn : permOff]"
                :aria-pressed="!!(mode & bit(w, a))"
                :title="`${t(who)} · ${t(what)}`"
                @click="setMode(mode ^ bit(w, a))"
              >
                {{ "rwx"[a] }}
              </button>
            </template>
          </div>
          <div :class="[line, 'flex min-w-0 flex-1 flex-col gap-2 self-stretch border-l pl-5']">
            <div class="flex items-center gap-3">
              <UiInput :value="octal" class="w-20! shrink-0 font-mono" :status="octalValid ? undefined : 'error'" placeholder="644" @update:value="onOctal" />
              <code :class="[dim, 'font-mono text-xs tracking-widest']">{{ formatMode(mode, single?.kind ?? "file") }}</code>
            </div>
            <span :class="[dim, 'mt-2 text-xs']">{{ t("props.presets") }}</span>
            <div class="grid grid-cols-5 gap-1.5">
              <button v-for="preset in PRESETS" :key="preset" type="button" :class="[permBit, 'h-7 text-xs', mode === preset ? permOn : permOff]" @click="setMode(preset)">
                {{ preset.toString(8) }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <section>
        <h4 :class="[dim, 'mx-0 mt-0 mb-2.5 text-xs font-semibold']">{{ t("props.ownership") }}</h4>
        <!-- 显式标注用户和组：有值时占位文字不可见。 -->
        <div class="flex items-center gap-2">
          <UiInput v-model:value="owner" class="min-w-0 flex-1 font-mono">
            <template #prefix><span :class="[dim, 'pr-1 font-sans text-xs']">{{ t("props.user") }}</span></template>
          </UiInput>
          <span :class="dim">:</span>
          <UiInput v-model:value="group" class="min-w-0 flex-1 font-mono">
            <template #prefix><span :class="[dim, 'pr-1 font-sans text-xs']">{{ t("props.group") }}</span></template>
          </UiInput>
        </div>
      </section>

      <UiCheckbox v-if="hasDir" v-model:checked="recursive">{{ t("props.recursive") }}</UiCheckbox>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2">
        <UiButton @click="show = false">{{ t("common.close") }}</UiButton>
        <UiButton type="primary" :disabled="!octalValid || (!modeChanged && !ownerChanged)" :loading="applying" @click="apply">{{ t("props.apply") }}</UiButton>
      </div>
    </template>
  </UiModal>
</template>
