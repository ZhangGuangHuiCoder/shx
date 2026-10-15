<script setup lang="ts">
// 规则走向切换：直连 | 隧道，隧道选中时为绿色。
import { computed } from "vue";
import type { TunnelVia } from "@/shared/api";
import { t } from "@/shared/i18n";
import SegmentedControl, { type SegmentOption } from "@/shared/ui-kit/SegmentedControl.vue";

const via = defineModel<TunnelVia>({ required: true });
const options = computed<SegmentOption<TunnelVia>[]>(() => [
  { value: "direct", label: t("tunnel.viaDirect") },
  { value: "tunnel", label: t("tunnel.viaTunnel"), tone: "ok" },
]);
</script>

<template>
  <SegmentedControl v-model="via" size="small" :options="options" />
</template>
