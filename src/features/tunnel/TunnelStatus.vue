<script setup lang="ts">
import { GlobeLock } from "@lucide/vue";
import { computed } from "vue";
import { t } from "../../shared/i18n";
import { formatSize } from "../../shared/util";
import { useApp } from "../../stores/app";
import { openTunnelDialog } from "./tunnelDialog";

// 状态栏里的“隧道”一项：隧道开着时才有，显示出口是哪台主机、它的地址和此刻的流量；点击打开它的设置。
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
  <div v-if="run" class="tunnel-status" :class="run.phase" :title="title" @click="openTunnelDialog(run.profileId)">
    <GlobeLock :size="13" />
    <span class="ellipsis name">{{ name }}</span>
    <span v-if="run.status?.server" class="ellipsis server">({{ run.status.server }})</span>
    <span v-if="run.phase === 'running'" class="rates">
      <span class="rate">↑ {{ formatSize(run.upRate) }}/s</span>
      <span class="rate">↓ {{ formatSize(run.downRate) }}/s</span>
    </span>
    <span v-else>{{ t(`tunnel.${run.phase}`) }}</span>
  </div>
</template>

<style scoped>
.tunnel-status {
  flex-shrink: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 100%;
  padding: 0 10px;
  color: var(--pin);
  cursor: pointer;
}

.tunnel-status:hover {
  background: var(--bg-hover);
}

.tunnel-status.running {
  color: var(--ok);
}

.tunnel-status svg {
  flex-shrink: 0;
}

.name {
  max-width: 140px;
}

/* 服务器的地址跟在名称后面；IPv6 的地址长，放不下时截断。 */
.server {
  max-width: 200px;
  font-variant-numeric: tabular-nums;
}

.rates {
  flex-shrink: 0;
  display: flex;
  gap: 4px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

/* 每个速率占固定的宽度：数字每秒都在变，位数和单位变了也不该把右边的东西推来推去。 */
.rate {
  min-width: 11ch;
  white-space: nowrap;
}
</style>
