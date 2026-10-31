<script setup lang="ts">
// “常用用户名”对话框；改动即时保存，所以只有“关闭”按钮。
import { UserRound } from "@lucide/vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import { t } from "@/shared/i18n";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiTagsInput from "@/shared/ui-kit/UiTagsInput.vue";
import { useApp } from "@/shared/store";
import { usernamesDialog } from "@/features/home/usernamesDialog";
import { dim, line, well } from "@/shared/ui-kit/styles";

const app = useApp();
</script>

<template>
  <UiModal v-model:show="usernamesDialog.open" class="w-xl">
    <template #header>
      <DialogHeader :icon="UserRound" :title="t('usernames.title')" />
    </template>

    <div :class="[line, well, 'max-h-65 overflow-y-auto rounded-xl border p-3']">
      <UiTagsInput v-model:value="app.settings.commonUsernames" :placeholder="t('usernames.placeholder')" />
    </div>
    <p :class="[dim, 'mt-3 mb-0 text-xs/relaxed']">{{ t("usernames.hint") }}</p>

    <template #footer>
      <div class="flex w-full justify-between gap-2">
        <UiButton @click="app.resetCommonUsernames()">{{ t("usernames.reset") }}</UiButton>
        <UiButton type="primary" @click="usernamesDialog.open = false">{{ t("common.close") }}</UiButton>
      </div>
    </template>
  </UiModal>
</template>
