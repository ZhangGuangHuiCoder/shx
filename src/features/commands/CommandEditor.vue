<script setup lang="ts">
import { Info, Pencil, Plus, Trash2, X } from "@lucide/vue";
import { computed, nextTick, onMounted, reactive, ref } from "vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import UiAutocomplete from "@/shared/ui-kit/UiAutocomplete.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import { shortcut } from "@/shared/contextMenu";
import { t } from "@/shared/i18n";
import CommandText from "@/features/commands/CommandText.vue";
import type { CustomCommand } from "@/features/commands/commands";
import { dim, keycap, line, sectionLabel, well } from "@/shared/ui-kit/styles";


const props = defineProps<{
  /** id 为空表示新建。 */
  command: CustomCommand;
  groups: string[];
}>();
const emit = defineEmits<{ save: [command: CustomCommand]; back: []; close: []; remove: [] }>();

const form = reactive<CustomCommand>({ ...props.command });
const nameInput = ref<InstanceType<typeof UiInput>>();
const existing = computed(() => !!props.command.id);
const valid = computed(() => !!form.command.trim());
/** 未填名称时取命令首行。 */
const name = computed(() => form.name.trim() || form.command.trim().split("\n")[0]);
const groupOptions = computed(() => {
  const typed = form.group.trim().toLowerCase();
  const matching = props.groups.filter((title) => title.toLowerCase().includes(typed));
  // 输入恰为某分组时列出全部，便于改选。
  return matching.some((title) => title.toLowerCase() === typed) ? props.groups : matching;
});

function submit() {
  if (valid.value) emit("save", { id: form.id, name: name.value, command: form.command.trim(), group: form.group.trim() });
}

onMounted(() => void nextTick(() => nameInput.value?.focus()));
defineExpose({ submit });
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <DialogHeader :icon="existing ? Pencil : Plus" :title="t(existing ? 'cmd.editTitle' : 'cmd.new')" :detail="existing ? props.command.name : ''" class="py-4 pr-4 pl-5">
      <IconButton :title="t('common.close')" @click="emit('close')"><X :size="16" /></IconButton>
    </DialogHeader>

    <div :class="[line, 'flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto border-t px-6 py-5']">
      <div class="flex gap-4">
        <label class="flex min-w-0 flex-1 flex-col gap-2">
          <span :class="sectionLabel">{{ t("cmd.fieldName") }}</span>
          <UiInput ref="nameInput" v-model:value="form.name" :placeholder="t('cmd.namePlaceholder')" />
        </label>
        <label class="flex min-w-0 basis-2/5 flex-col gap-2">
          <span :class="sectionLabel">{{ t("cmd.fieldGroup") }}</span>
          <UiAutocomplete v-model:value="form.group" :options="groupOptions" clearable :placeholder="t('cmd.groupPlaceholder')" />
        </label>
      </div>

      <label class="flex flex-col gap-2 min-w-0">
        <span :class="sectionLabel">{{ t("cmd.fieldCommand") }}</span>
        <UiInput v-model:value="form.command" type="textarea" :autosize="{ minRows: 5, maxRows: 9 }" :placeholder="t('cmd.commandPlaceholder')" class="font-mono text-xs/relaxed!" />
      </label>
      <p :class="[dim, '-mt-2 mb-0 flex gap-2 text-xs/5']"><Info :size="13" class="mt-1 shrink-0" /> {{ t("cmd.placeholderTip") }}</p>

      <div class="flex flex-col gap-2 min-w-0">
        <span :class="sectionLabel">{{ t("cmd.preview") }}</span>
        <div :class="[line, well, 'flex items-center gap-3 rounded-xl border border-dashed p-3']">
          <CommandText :name="name || t('cmd.previewName')" :command="form.command.trim() || t('cmd.commandPlaceholder')" :class="{ 'opacity-50': !valid }" />
        </div>
      </div>
    </div>

    <footer :class="[line, well, 'flex h-13 shrink-0 items-center gap-2 border-t px-4']">
      <UiButton v-if="existing" ghost type="error" size="small" @click="emit('remove')">
        <template #icon><Trash2 :size="14" /></template>
        {{ t("common.delete") }}
      </UiButton>
      <span class="flex-1" />
      <span :class="[dim, 'mr-2 inline-flex items-center gap-1 text-xs']"><kbd :class="keycap">esc</kbd> {{ t("cmd.keyBack") }}</span>
      <span :class="[dim, 'mr-2 inline-flex items-center gap-1 text-xs']"><kbd :class="keycap">{{ shortcut.mod("↵") }}</kbd> {{ t("cmd.keySave") }}</span>
      <UiButton size="small" @click="emit('back')">{{ t("common.cancel") }}</UiButton>
      <UiButton size="small" type="primary" :disabled="!valid" @click="submit">{{ t("common.save") }}</UiButton>
    </footer>
  </div>
</template>
