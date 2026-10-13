<script setup lang="ts">
// 命令助手里新建、修改一条命令的那一页。
import { Info, Pencil, Plus, Trash2, X } from "@lucide/vue";
import { NAutoComplete, NButton, NInput } from "naive-ui";
import { computed, nextTick, onMounted, reactive, ref } from "vue";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { shortcut } from "../../shared/contextMenu";
import { t } from "../../shared/i18n";
import CommandText from "./CommandText.vue";
import { groupKey, type CustomCommand } from "./commands";
import { groupIcon } from "./groupIcons";

const props = defineProps<{
  /** 要修改的命令；`id` 为空表示新建。 */
  command: CustomCommand;
  /** 已有分组的标题，供选择。 */
  groups: string[];
}>();
const emit = defineEmits<{ save: [command: CustomCommand]; back: []; close: []; remove: [] }>();

const form = reactive<CustomCommand>({ ...props.command });
const nameInput = ref<InstanceType<typeof NInput>>();
const existing = computed(() => !!props.command.id);
const valid = computed(() => !!form.command.trim());
/** 没有名称时，用命令的第一行作为名称。 */
const name = computed(() => form.name.trim() || form.command.trim().split("\n")[0]);
const groupOptions = computed(() => {
  const typed = form.group.trim().toLowerCase();
  const matching = props.groups.filter((title) => title.toLowerCase().includes(typed));
  // 输入的正好是某个分组时列出全部，方便改选别的。
  return matching.some((title) => title.toLowerCase() === typed) ? props.groups : matching;
});

function submit() {
  if (valid.value) emit("save", { id: form.id, name: name.value, command: form.command.trim(), group: form.group.trim() });
}

onMounted(() => void nextTick(() => nameInput.value?.focus()));
defineExpose({ submit });
</script>

<template>
  <div class="editor">
    <DialogHeader :icon="existing ? Pencil : Plus" :title="t(existing ? 'cmd.editTitle' : 'cmd.new')" :detail="existing ? props.command.name : ''" class="head">
      <IconButton :title="t('common.close')" @click="emit('close')"><X :size="16" /></IconButton>
    </DialogHeader>

    <div class="fields">
      <div class="row">
        <label class="field">
          <span class="label">{{ t("cmd.fieldName") }}</span>
          <NInput ref="nameInput" v-model:value="form.name" :placeholder="t('cmd.namePlaceholder')" />
        </label>
        <label class="field group-field">
          <span class="label">{{ t("cmd.fieldGroup") }}</span>
          <NAutoComplete v-model:value="form.group" :options="groupOptions" :get-show="() => true" clearable :placeholder="t('cmd.groupPlaceholder')" />
        </label>
      </div>

      <label class="field">
        <span class="label">{{ t("cmd.fieldCommand") }}</span>
        <NInput v-model:value="form.command" type="textarea" :autosize="{ minRows: 5, maxRows: 9 }" :placeholder="t('cmd.commandPlaceholder')" class="mono" />
      </label>
      <p class="tip"><Info :size="13" /> {{ t("cmd.placeholderTip") }}</p>

      <div class="field">
        <span class="label">{{ t("cmd.preview") }}</span>
        <div class="preview">
          <div class="glyph"><component :is="groupIcon(groupKey(form.group))" :size="15" :stroke-width="1.9" /></div>
          <CommandText :name="name || t('cmd.previewName')" :command="form.command.trim() || t('cmd.commandPlaceholder')" :class="{ blank: !valid }" />
        </div>
      </div>
    </div>

    <footer>
      <NButton v-if="existing" quaternary type="error" size="small" @click="emit('remove')">
        <template #icon><Trash2 :size="14" /></template>
        {{ t("common.delete") }}
      </NButton>
      <span class="spacer" />
      <span class="keys"><kbd>esc</kbd> {{ t("cmd.keyBack") }}</span>
      <span class="keys"><kbd>{{ shortcut.mod("↵") }}</kbd> {{ t("cmd.keySave") }}</span>
      <NButton size="small" @click="emit('back')">{{ t("common.cancel") }}</NButton>
      <NButton size="small" type="primary" :disabled="!valid" @click="submit">{{ t("common.save") }}</NButton>
    </footer>
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.head {
  padding: 16px 14px 14px 18px;
}

.fields {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
  padding: 18px 22px;
  border-top: 1px solid var(--border);
  overflow-y: auto;
}

.row {
  display: flex;
  gap: 14px;
}

.field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.fields > .field {
  flex: none;
}

.group-field {
  flex: 0 0 38%;
}

.label {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}

.mono :deep(textarea) {
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
}

.tip {
  display: flex;
  gap: 6px;
  margin: -6px 0 0;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 18px;
}

.tip svg {
  flex-shrink: 0;
  margin-top: 2.5px;
}

.preview {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px dashed var(--border);
  border-radius: 10px;
}

.glyph {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  flex-shrink: 0;
  border-radius: 8px;
  background: var(--accent-soft);
  color: var(--accent);
}

.blank {
  opacity: 0.5;
}

footer {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 52px;
  flex-shrink: 0;
  padding: 0 16px 0 14px;
  border-top: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-raised));
}

.spacer {
  flex: 1;
}

.keys {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-right: 8px;
  color: var(--text-dim);
  font-size: 12px;
}
</style>
