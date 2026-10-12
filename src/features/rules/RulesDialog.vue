<script setup lang="ts">
// “可编辑文件类型”对话框：匹配这些规则的文件，双击时在内置编辑器中打开。
// 改动立即生效并保存，所以底栏只有“关闭”，没有“保存”。
import { FilePen } from "@lucide/vue";
import { NButton, NDynamicTags, NModal, NSpace } from "naive-ui";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import { t } from "../../shared/i18n";
import { useApp } from "../../stores/app";
import { rulesDialog } from "./rulesDialog";

const app = useApp();
</script>

<template>
  <NModal v-model:show="rulesDialog.open" preset="card" style="width: 600px" :bordered="false">
    <template #header>
      <DialogHeader :icon="FilePen" :title="t('rules.title')" />
    </template>

    <div class="rules">
      <NDynamicTags v-model:value="app.settings.editRules" />
    </div>
    <p class="hint">{{ t("rules.hint") }}</p>

    <template #footer>
      <NSpace justify="space-between">
        <NButton @click="app.resetEditRules()">{{ t("rules.reset") }}</NButton>
        <NButton type="primary" @click="rulesDialog.open = false">{{ t("common.close") }}</NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.rules {
  max-height: 260px;
  padding: 12px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
}

.hint {
  margin: 12px 0 0;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.7;
}
</style>
