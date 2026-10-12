<script setup lang="ts">
import { ArrowDown, ArrowUp, FolderOpen, Pause, Play, RotateCw, X } from "@lucide/vue";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { computed } from "vue";
import { api, errorMessage, type Transfer } from "../../shared/api";
import { t } from "../../shared/i18n";
import { message } from "../../shared/ui";
import { formatDuration, formatSize } from "../../shared/util";
import { useApp } from "../../stores/app";
import { isActive } from "./transfer";

// 传输任务列表。它由状态栏里的“传输”一项展开，画在状态栏上方。
const app = useApp();
const ordered = computed(() => [...app.transfers].reverse());

function percent(item: Transfer) {
  if (item.state === "done") return 100;
  return item.total ? Math.min(100, (item.transferred / item.total) * 100) : 0;
}

function detail(item: Transfer) {
  if (item.state === "failed") return item.error ?? "";
  // 还没有可统计的数字：压缩包多大要打包完才知道。
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

/** 在系统的文件管理器里显示下载好的文件。它可能已经被移走或删掉。 */
function reveal(path: string) {
  revealItemInDir(path).catch(() => message.error(t("transfer.revealFailed", { path })));
}
</script>

<template>
  <div class="list">
    <div v-for="item in ordered" :key="item.id" class="row" :class="item.state">
      <component :is="item.direction === 'upload' ? ArrowUp : ArrowDown" :size="14" class="dir" />
      <div class="info">
        <div class="line">
          <span class="ellipsis name" :title="item.source">{{ item.name }}</span>
          <span class="ellipsis dim dest" :title="item.destDir">→ {{ item.destDir }}</span>
          <span class="state">{{ t(`transfer.${item.state}`) }}</span>
        </div>
        <div class="track"><div class="fill" :class="{ working: item.state === 'packing' }" :style="{ width: `${percent(item)}%` }" /></div>
        <div class="ellipsis dim sub" :title="detail(item)">{{ detail(item) }}<template v-if="item.current && item.filesTotal > 1"> · {{ item.current }}</template></div>
      </div>
      <div class="actions">
        <button v-if="item.local && item.state === 'done'" class="icon-btn" :title="t('transfer.reveal')" @click="reveal(item.local)"><FolderOpen :size="13" /></button>
        <button v-if="item.state === 'running'" class="icon-btn" :title="t('transfer.pause')" @click="act(api.transferControl(item.id, 'pause'))"><Pause :size="13" /></button>
        <button v-if="item.state === 'paused'" class="icon-btn" :title="t('transfer.resume')" @click="act(api.transferControl(item.id, 'resume'))"><Play :size="13" /></button>
        <button v-if="item.state === 'failed' || item.state === 'cancelled'" class="icon-btn" :title="t('transfer.retry')" @click="act(api.transferRetry(item.id))"><RotateCw :size="13" /></button>
        <button v-if="isActive(item)" class="icon-btn" :title="t('common.cancel')" @click="act(api.transferControl(item.id, 'cancel'))"><X :size="13" /></button>
      </div>
    </div>
    <div v-if="!ordered.length" class="none dim">{{ t("transfer.empty") }}</div>
  </div>
</template>

<style scoped>
.row.failed .state,
.row.failed .sub {
  color: var(--danger);
}

.list {
  height: 170px;
  overflow-y: auto;
  border-bottom: 1px solid var(--border);
  background: var(--bg);
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border);
}

.dir {
  flex-shrink: 0;
  color: var(--text-dim);
}

.info {
  flex: 1;
  min-width: 0;
}

.line {
  display: flex;
  gap: 8px;
}

.name {
  flex-shrink: 0;
  max-width: 45%;
}

.dest {
  flex: 1;
}

.state {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--text-dim);
}

.row.done .state {
  color: var(--ok);
}

.track {
  height: 3px;
  margin: 4px 0;
  border-radius: 2px;
  background: var(--bg-hover);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.2s linear;
}

/* 在工作但还没有可度量的进度：一段色条沿轨道来回移动。 */
.fill.working {
  width: 30% !important;
  transition: none;
  animation: working 1.2s ease-in-out infinite;
}

@keyframes working {
  from {
    transform: translateX(-100%);
  }

  to {
    transform: translateX(334%);
  }
}

.row.done .fill {
  background: var(--ok);
}

.row.failed .fill,
.row.cancelled .fill {
  background: var(--text-dim);
}

.sub {
  font-size: 11px;
}

.actions {
  display: flex;
  flex-shrink: 0;
  width: 56px;
  justify-content: flex-end;
}

.none {
  padding: 28px 0;
  text-align: center;
}
</style>
