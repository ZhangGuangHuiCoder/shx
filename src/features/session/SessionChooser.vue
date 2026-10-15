<script setup lang="ts">
// 保活主机有多个会话时，让用户选择进入哪个或新开。
import { HeartPulse, Plus, X } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import { t } from "@/shared/i18n";
import { formatTime } from "@/shared/util";
import type { SessionChoice } from "@/features/session/sessionChooser";
import { accentText, dim, hoverFill, line, okChip, plainChip, well } from "@/shared/ui-kit/styles";

const row = `flex shrink-0 items-center gap-3 border-t p-3 transition-colors duration-150 first:border-t-0 ${line} ${hoverFill}`;
const tag = "shrink-0 rounded-full px-2 py-px text-xs/4";

const props = defineProps<{ host: string; sessions: SessionChoice[] }>();
/** 空串表示新开，null 表示取消。 */
const emit = defineEmits<{ done: [name: string | null] }>();

const body = ref<HTMLElement>();

/** 是否空闲（登录 shell 名以 - 开头）。 */
const SHELLS = ["sh", "bash", "zsh", "fish", "dash", "ash", "ksh", "csh", "tcsh"];
const idle = (session: SessionChoice) => SHELLS.includes(session.command.replace(/^-/, ""));

/** 最近活动的会话，作为默认选项。 */
const latest = computed(() => props.sessions.reduce((a, b) => (b.activity > a.activity ? b : a)).name);

/** 距最后输出的时长；时钟偏差导致的未来时间按“刚刚”处理。 */
function ago(epochSeconds: number): string {
  const seconds = Date.now() / 1000 - epochSeconds;
  if (seconds < 60) return t("time.justNow");
  if (seconds < 3600) return t("time.minutesAgo", { n: Math.floor(seconds / 60) });
  if (seconds < 86400) return t("time.hoursAgo", { n: Math.floor(seconds / 3600) });
  return formatTime(epochSeconds);
}

// 对话框焦点锁定会在此之后聚焦第一个按钮，延后一拍再设焦点。
onMounted(() => setTimeout(() => body.value?.querySelector<HTMLElement>(".join.latest")?.focus()));
</script>

<template>
  <div ref="body" class="flex flex-col gap-4 p-5">
    <DialogHeader :icon="HeartPulse" :title="t('keepAlive.chooseTitle')" :detail="t('keepAlive.chooseDetail', { name: host, count: sessions.length })">
      <IconButton tabindex="-1" :title="t('common.cancel')" @click="emit('done', null)"><X :size="16" /></IconButton>
    </DialogHeader>

    <div :class="[line, 'flex max-h-105 flex-col overflow-y-auto rounded-2xl border', well]">
      <div v-for="session in sessions" :key="session.name" :class="row">
        <div class="grid size-9 shrink-0 place-items-center rounded-xl font-mono text-sm font-semibold" :class="idle(session) ? [dim, 'bg-zinc-900/5 dark:bg-white/10'] : [accentText, 'bg-blue-500/12 dark:bg-blue-400/15']">#{{ session.slot }}</div>
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="flex min-w-0 items-center gap-2">
            <span class="truncate text-sm font-semibold" :class="{ 'font-mono': !idle(session) }" :title="session.command">{{ idle(session) ? t("keepAlive.idle") : session.command }}</span>
            <span v-if="!idle(session)" :class="[tag, okChip]">{{ t("keepAlive.running") }}</span>
            <span v-if="session.attached" :class="[tag, plainChip]">{{ t("keepAlive.inUse") }}</span>
          </div>
          <div :class="[dim, 'flex min-w-0 gap-2 text-xs']">
            <span class="truncate font-mono" :title="session.path">{{ session.path }}</span>
            <span class="shrink-0" :title="formatTime(session.activity)"><span class="mr-2">·</span>{{ ago(session.activity) }}</span>
          </div>
        </div>
        <UiButton
          class="join min-w-15 shrink-0"
          :class="{ latest: session.name === latest }"
          size="small"
          :type="session.name === latest ? 'primary' : 'default'"
          @click="emit('done', session.name)"
        >
          {{ t("keepAlive.join") }}
        </UiButton>
      </div>

      <div :class="row">
        <div :class="[line, dim, 'grid size-9 shrink-0 place-items-center rounded-xl border border-dashed']"><Plus :size="16" :stroke-width="2.2" /></div>
        <div class="flex min-w-0 flex-1 flex-col gap-1">
          <div class="flex min-w-0 items-center gap-2">
            <span class="text-sm font-semibold">{{ t("keepAlive.newSession") }}</span>
          </div>
          <div :class="[dim, 'flex min-w-0 gap-2 text-xs']">{{ t("keepAlive.newSessionHint") }}</div>
        </div>
        <UiButton class="min-w-15 shrink-0" size="small" @click="emit('done', '')">{{ t("keepAlive.create") }}</UiButton>
      </div>
    </div>
  </div>
</template>
