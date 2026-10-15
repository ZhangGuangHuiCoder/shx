<script setup lang="ts">
// “可编辑文件类型”对话框；改动即时保存，所以只有“关闭”按钮。
import { FilePen } from "@lucide/vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import { t } from "@/shared/i18n";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiTagsInput from "@/shared/ui-kit/UiTagsInput.vue";
import { useApp } from "@/shared/store";
import { rulesDialog } from "@/features/rules/rulesDialog";
import { dim, line, well } from "@/shared/ui-kit/styles";

const app = useApp();
</script>

<template>
  <UiModal v-model:show="rulesDialog.open" class="w-xl">
    <template #header>
      <DialogHeader :icon="FilePen" :title="t('rules.title')" />
    </template>

    <div :class="[line, well, 'max-h-65 overflow-y-auto rounded-xl border p-3']">
      <UiTagsInput v-model:value="app.settings.editRules" />
    </div>
    <p :class="[dim, 'mt-3 mb-0 text-xs/relaxed']">{{ t("rules.hint") }}</p>

    <template #footer>
      <div class="flex w-full justify-between gap-2">
        <UiButton @click="app.resetEditRules()">{{ t("rules.reset") }}</UiButton>
        <UiButton type="primary" @click="rulesDialog.open = false">{{ t("common.close") }}</UiButton>
      </div>
    </template>
  </UiModal>
</template>
