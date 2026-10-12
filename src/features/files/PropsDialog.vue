<script setup lang="ts">
import { Files } from "@lucide/vue";
import { NButton, NCheckbox, NInput, NModal, NSpace } from "naive-ui";
import { computed, ref, watch } from "vue";
import { api, errorMessage, type FileEntry } from "../../shared/api";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import FileIcon from "../../shared/components/FileIcon.vue";
import { t, type MessageKey } from "../../shared/i18n";
import { message } from "../../shared/ui";
import { formatMode, formatSize, formatTime } from "../../shared/util";

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

/** 值得单独放一个按钮的常用权限。 */
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
  <NModal v-model:show="show" preset="card" style="width: 470px" :bordered="false">
    <template #header>
      <DialogHeader :title="single ? single.name : t('props.items', { count: items.length })" :detail="single ? single.path : t('props.title')">
        <template #icon>
          <FileIcon v-if="single" :name="single.name" :is-dir="single.isDir" :kind="single.kind" :size="19" />
          <Files v-else :size="18" />
        </template>
      </DialogHeader>
    </template>

    <div class="props">
      <div v-if="single" class="facts">
        <span class="dim">{{ t("props.type") }}</span><span>{{ t(`kind.${single.kind}`) }}</span>
        <template v-if="single.linkTarget">
          <span class="dim">{{ t("props.target") }}</span><span class="value">{{ single.linkTarget }}</span>
        </template>
        <span class="dim">{{ t("props.size") }}</span>
        <span>
          <template v-if="single.kind === 'dir'">
            {{ treeSize ?? "" }}
            <NButton v-if="!treeSize" text type="primary" size="tiny" :loading="measuring" @click="measure">{{ t("props.calculate") }}</NButton>
          </template>
          <template v-else>{{ formatSize(single.size) }}<span class="dim">{{ t("props.bytes", { count: single.size.toLocaleString() }) }}</span></template>
        </span>
        <span class="dim">{{ t("props.modified") }}</span><span>{{ formatTime(single.mtime) }}</span>
      </div>

      <section>
        <h4>{{ t("props.permissions") }}</h4>
        <div class="perms">
          <span />
          <span v-for="what in WHAT" :key="what" class="dim head">{{ t(what) }}</span>
          <template v-for="(who, w) in WHO" :key="who">
            <span class="who">{{ t(who) }}</span>
            <button
              v-for="(what, a) in WHAT"
              :key="what"
              type="button"
              class="bit"
              :class="{ on: !!(mode & bit(w, a)) }"
              :aria-pressed="!!(mode & bit(w, a))"
              :title="`${t(who)} · ${t(what)}`"
              @click="setMode(mode ^ bit(w, a))"
            >{{ "rwx"[a] }}</button>
          </template>
        </div>
        <div class="octal">
          <NInput :value="octal" size="small" class="octal-input" :status="octalValid ? undefined : 'error'" @update:value="onOctal" />
          <code class="symbolic">{{ formatMode(mode, single?.kind ?? "file") }}</code>
          <span class="dim presets-label">{{ t("props.presets") }}</span>
          <button v-for="preset in PRESETS" :key="preset" type="button" class="preset" :class="{ on: mode === preset }" @click="setMode(preset)">
            {{ preset.toString(8) }}
          </button>
        </div>
      </section>

      <section>
        <h4>{{ t("props.ownership") }}</h4>
        <div class="owner">
          <NInput v-model:value="owner" size="small" :placeholder="t('props.user')" />
          <span class="dim">:</span>
          <NInput v-model:value="group" size="small" :placeholder="t('props.group')" />
        </div>
      </section>

      <NCheckbox v-if="hasDir" v-model:checked="recursive">{{ t("props.recursive") }}</NCheckbox>
    </div>

    <template #footer>
      <NSpace justify="end">
        <NButton @click="show = false">{{ t("common.close") }}</NButton>
        <NButton type="primary" :disabled="!octalValid || (!modeChanged && !ownerChanged)" :loading="applying" @click="apply">{{ t("props.apply") }}</NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.props {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.facts {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 7px 16px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
}

.value {
  word-break: break-all;
  user-select: text;
  -webkit-user-select: text;
}

h4 {
  margin: 0 0 10px;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.4px;
}

/* 纵向是对象，横向是权限，九个权限位各有一个开关。 */
.perms {
  display: grid;
  grid-template-columns: max-content repeat(3, 64px);
  align-items: center;
  gap: 6px 8px;
}

.head {
  font-size: 12px;
  text-align: center;
}

.who {
  padding-right: 12px;
}

.bit {
  height: 28px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: none;
  color: var(--text-dim);
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 13px;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s, color 0.12s;
}

.bit:hover {
  color: var(--text);
}

.bit.on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
  color: var(--accent);
  font-weight: 600;
}

.octal {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.octal-input {
  width: 72px;
  flex-shrink: 0;
}

.octal-input :deep(input),
.symbolic {
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
}

.symbolic {
  color: var(--text-dim);
  font-size: 12px;
}

.presets-label {
  margin-left: auto;
  font-size: 12px;
}

.preset {
  height: 22px;
  padding: 0 7px;
  border: 0;
  border-radius: 6px;
  background: var(--fill);
  color: var(--text-dim);
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 11.5px;
  cursor: pointer;
}

.preset:hover {
  color: var(--text);
}

.preset.on {
  background: var(--accent-soft);
  color: var(--accent);
}

.owner {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  gap: 8px;
}
</style>
