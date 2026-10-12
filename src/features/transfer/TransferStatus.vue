<script setup lang="ts">
import { ArrowDownUp, ChevronDown, ChevronUp } from "@lucide/vue";
import { computed, watch } from "vue";
import { errorMessage } from "../../shared/api";
import { t } from "../../shared/i18n";
import { message } from "../../shared/ui";
import { formatSize } from "../../shared/util";
import { useApp } from "../../stores/app";
import { isActive } from "./transfer";

// 状态栏里的“传输”一项：汇总各任务的进展，点击展开或收起任务列表。
const app = useApp();
/** 任务列表是否展开；列表由状态栏画在自己上方。 */
const open = defineModel<boolean>("open", { required: true });

const active = computed(() => app.transfers.filter(isActive));
const finished = computed(() => app.transfers.length - active.value.length);
const failed = computed(() => app.transfers.filter((item) => item.state === "failed").length);
const speed = computed(() => active.value.reduce((sum, item) => sum + item.speed, 0));

// 有新任务排入时自动展开列表。
watch(
  () => app.transfers.length,
  (now, before) => {
    if (now > before) open.value = true;
  },
);

function clear() {
  app.clearTransfers().catch((e) => message.error(errorMessage(e)));
}
</script>

<template>
  <div class="transfer-status" :class="{ open }" @click="open = !open">
    <ArrowDownUp :size="13" />
    <span>{{ t("transfer.title") }}</span>
    <span v-if="active.length" class="accent">{{ t("transfer.active", { count: active.length }) }} · {{ formatSize(speed) }}/s</span>
    <span v-if="finished" class="dim">{{ t("transfer.finished", { count: finished }) }}</span>
    <span v-if="failed" class="bad">{{ t("transfer.failedCount", { count: failed }) }}</span>
    <button v-if="finished" class="icon-btn clear" @click.stop="clear">{{ t("transfer.clear") }}</button>
    <component :is="open ? ChevronDown : ChevronUp" :size="14" />
  </div>
</template>

<style scoped>
.transfer-status {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 100%;
  padding: 0 10px;
  cursor: pointer;
}

.transfer-status:hover,
.transfer-status.open {
  background: var(--bg-hover);
}

.accent {
  color: var(--accent);
}

.bad {
  color: var(--danger);
}

.clear {
  height: 20px;
  font-size: 12px;
}
</style>
