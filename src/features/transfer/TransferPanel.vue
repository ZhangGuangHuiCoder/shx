<script setup lang="ts">
import { ArrowDown, ArrowUp, FolderOpen, Pause, Play, RotateCw, X } from "@lucide/vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { computed } from "vue";
import { api, errorMessage, type Transfer, type TransferState } from "@/shared/api";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { t } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { formatDuration, formatSize } from "@/shared/util";
import { useApp } from "@/shared/store";
import { isActive } from "@/features/transfer/transfer";
import { dangerText, dim, line, lineFill, okText } from "@/shared/ui-kit/styles";

const RUNNING = { text: dim, bar: "bg-linear-to-r from-blue-400 to-blue-500" };
const STATE: Record<TransferState, { text: string; bar: string }> = {
  queued: RUNNING,
  packing: RUNNING,
  running: RUNNING,
  paused: RUNNING,
  done: { text: okText, bar: "bg-emerald-500" },
  failed: { text: dangerText, bar: "bg-zinc-400" },
  cancelled: { text: dim, bar: "bg-zinc-400" },
};

const app = useApp();
const ordered = computed(() => [...app.transfers].reverse());

function percent(item: Transfer) {
  if (item.state === "done") return 100;
  return item.total ? Math.min(100, (item.transferred / item.total) * 100) : 0;
}

function detail(item: Transfer) {
  if (item.state === "failed") return item.error ?? "";
  // 打包完成前无法得知大小。
  if (item.state === "packing") return t("transfer.packingDetail");
  const parts = [`${formatSize(item.transferred)} / ${formatSize(item.total)}`];
  if (item.filesTotal > 1) parts.push(t("transfer.files", { done: item.filesDone, total: item.filesTotal }));
  if (item.state === "running") {
    parts.push(`${formatSize(item.speed)}/s`);
    if (item.speed > 0) {
      parts.push(t("transfer.remaining", { time: formatDuration((item.total - item.transferred) / item.speed) }));
    }
  }
  return parts.join(" · ");
}

function act(run: Promise<void>) {
  run.catch((e) => message.error(errorMessage(e)));
}

/** 在系统文件管理器中显示（文件可能已被移走）。 */
function reveal(path: string) {
  revealItemInDir(path).catch(() => message.error(t("transfer.revealFailed", { path })));
}
</script>

<template>
  <div :class="[line, 'h-44 overflow-y-auto border-b']">
    <div v-for="item in ordered" :key="item.id" :class="[line, 'flex items-center gap-3 border-b px-3 py-2']">
      <component :is="item.direction === 'upload' ? ArrowUp : ArrowDown" :size="14" :class="[dim, 'shrink-0']" />
      <div class="min-w-0 flex-1">
        <div class="flex gap-2 text-sm">
          <span class="max-w-2/5 shrink-0 truncate" :title="item.source">{{ item.name }}</span>
          <span :class="[dim, 'flex-1 truncate']" :title="item.destDir">→ {{ item.destDir }}</span>
          <span :class="[STATE[item.state].text, 'shrink-0 text-xs']">{{ t(`transfer.${item.state}`) }}</span>
        </div>
        <!-- 进度不可度量时闪烁。 -->
        <div :class="[lineFill, 'my-1 h-1 overflow-hidden rounded-full']">
          <div
            :class="[STATE[item.state].bar, 'h-full rounded-full', item.state === 'packing' ? 'motion-safe:animate-pulse' : 'transition-all duration-200 ease-linear']"
            :style="{ width: item.state === 'packing' ? '100%' : `${percent(item)}%` }"
          />
        </div>
        <div :class="[item.state === 'failed' ? dangerText : dim, 'truncate text-xs']" :title="detail(item)">{{ detail(item) }}<template v-if="item.current && item.filesTotal > 1"> · {{ item.current }}</template></div>
      </div>
      <div class="flex w-14 shrink-0 justify-end">
        <IconButton v-if="item.local && item.state === 'done'" size="tiny" :title="t('transfer.reveal')" @click="reveal(item.local)"><FolderOpen :size="13" /></IconButton>
        <IconButton v-if="item.state === 'running'" size="tiny" :title="t('transfer.pause')" @click="act(api.transferControl(item.id, 'pause'))"><Pause :size="13" /></IconButton>
        <IconButton v-if="item.state === 'paused'" size="tiny" :title="t('transfer.resume')" @click="act(api.transferControl(item.id, 'resume'))"><Play :size="13" /></IconButton>
        <IconButton v-if="item.state === 'failed' || item.state === 'cancelled'" size="tiny" :title="t('transfer.retry')" @click="act(api.transferRetry(item.id))"><RotateCw :size="13" /></IconButton>
        <IconButton v-if="isActive(item)" size="tiny" :title="t('common.cancel')" @click="act(api.transferControl(item.id, 'cancel'))"><X :size="13" /></IconButton>
      </div>
    </div>
    <div v-if="!ordered.length" :class="[dim, 'py-7 text-center text-sm']">{{ t("transfer.empty") }}</div>
  </div>
</template>
