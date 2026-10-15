<script setup lang="ts">
import { GlobeLock } from "@lucide/vue";
import { computed } from "vue";
import { t } from "@/shared/i18n";
import { formatSize } from "@/shared/util";
import { useApp } from "@/shared/store";
import { openTunnelDialog } from "@/features/tunnel/tunnelDialog";
import { dim, hoverFill, okText, warnText } from "@/shared/ui-kit/styles";

// 状态栏中的隧道状态，点击打开设置。
const app = useApp();
const run = computed(() => app.tunnel);
const name = computed(() => app.profiles.find((p) => p.id === run.value?.profileId)?.name ?? "");
const title = computed(() => {
  const status = run.value?.status;
  if (!status) return t("tunnel.name");
  return [t("tunnel.name"), name.value, status.device, status.server].filter(Boolean).join(" · ");
});
</script>

<template>
  <div
    v-if="run"
    :class="[hoverFill, 'flex h-full min-w-0 shrink cursor-pointer items-center gap-2 px-3', run.phase === 'running' ? okText : warnText]"
    :title="title"
    @click="openTunnelDialog(run.profileId)"
  >
    <GlobeLock :size="13" class="shrink-0" />
    <span class="max-w-35 truncate">{{ name }}</span>
    <span v-if="run.status?.server" class="max-w-50 truncate tabular-nums">({{ run.status.server }})</span>
    <span v-if="run.phase === 'running'" :class="[dim, 'flex shrink-0 gap-1 tabular-nums']">
      <span class="min-w-22 whitespace-nowrap">↑ {{ formatSize(run.upRate) }}/s</span>
      <span class="min-w-22 whitespace-nowrap">↓ {{ formatSize(run.downRate) }}/s</span>
    </span>
    <span v-else>{{ t(`tunnel.${run.phase}`) }}</span>
  </div>
</template>
