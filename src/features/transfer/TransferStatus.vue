<script setup lang="ts">
import { ArrowDownUp, ChevronDown, ChevronUp } from "@lucide/vue";
import { computed, watch } from "vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import { errorMessage } from "@/shared/api";
import { t } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { formatSize } from "@/shared/util";
import { useApp } from "@/shared/store";
import { isActive } from "@/features/transfer/transfer";
import { accentText, dangerText, dim, hoverFill } from "@/shared/ui-kit/styles";

const app = useApp();
const open = defineModel<boolean>("open", { required: true });

const active = computed(() => app.transfers.filter(isActive));
const finished = computed(() => app.transfers.length - active.value.length);
const failed = computed(() => app.transfers.filter((item) => item.state === "failed").length);
const speed = computed(() => active.value.reduce((sum, item) => sum + item.speed, 0));

// 有新任务时自动展开。
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
  <div :class="[hoverFill, 'flex h-full cursor-pointer items-center gap-2 px-3', open && 'bg-zinc-900/5 dark:bg-white/10']" @click="open = !open">
    <ArrowDownUp :size="13" />
    <span>{{ t("transfer.title") }}</span>
    <span v-if="active.length" :class="accentText">{{ t("transfer.active", { count: active.length }) }} · {{ formatSize(speed) }}/s</span>
    <span v-if="finished" :class="dim">{{ t("transfer.finished", { count: finished }) }}</span>
    <span v-if="failed" :class="dangerText">{{ t("transfer.failedCount", { count: failed }) }}</span>
    <UiButton v-if="finished" ghost size="tiny" @click.stop="clear">{{ t("transfer.clear") }}</UiButton>
    <component :is="open ? ChevronDown : ChevronUp" :size="14" />
  </div>
</template>
