<script setup lang="ts">
// 连上保活主机、发现服务器上留着不止一个会话时显示：一行一个会话，写明里面在跑什么，
// 行尾的按钮直接进入；最后一行是新开一个。由 `chooseSession` 打开。
import { HeartPulse, Plus, X } from "@lucide/vue";
import { NButton } from "naive-ui";
import { computed, onMounted, ref } from "vue";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { t } from "../../shared/i18n";
import { formatTime } from "../../shared/util";
import type { SessionChoice } from "./sessionChooser";

const props = defineProps<{ host: string; sessions: SessionChoice[] }>();
/** 交回要进入的会话名；空字符串表示新开一个，null 表示不连了。 */
const emit = defineEmits<{ done: [name: string | null] }>();

const body = ref<HTMLElement>();

/** 空闲的 shell：里面没有在跑别的程序。登录 shell 的名字前面带一个 `-`。 */
const SHELLS = ["sh", "bash", "zsh", "fish", "dash", "ash", "ksh", "csh", "tcsh"];
const idle = (session: SessionChoice) => SHELLS.includes(session.command.replace(/^-/, ""));

/** 最近还有动静的那个：多半就是要找的，按钮做成实心，回车选它。 */
const latest = computed(() => props.sessions.reduce((a, b) => (b.activity > a.activity ? b : a)).name);

/** 最后一次有输出距今多久。服务器的钟可能和本机差一点，算出来在未来的一律当作刚刚。 */
function ago(epochSeconds: number): string {
  const seconds = Date.now() / 1000 - epochSeconds;
  if (seconds < 60) return t("time.justNow");
  if (seconds < 3600) return t("time.minutesAgo", { n: Math.floor(seconds / 60) });
  if (seconds < 86400) return t("time.hoursAgo", { n: Math.floor(seconds / 3600) });
  return formatTime(epochSeconds);
}

// 外层对话框的焦点锁定在挂载时会把焦点放到第一个按钮上，而且发生在这里之后；等它做完再放。
onMounted(() => setTimeout(() => body.value?.querySelector<HTMLElement>(".join.latest")?.focus()));
</script>

<template>
  <div ref="body" class="chooser">
    <DialogHeader :icon="HeartPulse" :title="t('keepAlive.chooseTitle')" :detail="t('keepAlive.chooseDetail', { name: host, count: sessions.length })">
      <IconButton :focusable="false" :title="t('common.cancel')" @click="emit('done', null)"><X :size="16" /></IconButton>
    </DialogHeader>

    <div class="sessions">
      <div v-for="session in sessions" :key="session.name" class="session" :class="{ busy: !idle(session) }">
        <div class="slot">#{{ session.slot }}</div>
        <div class="words">
          <div class="line">
            <span class="command ellipsis" :title="session.command">{{ idle(session) ? t("keepAlive.idle") : session.command }}</span>
            <span v-if="!idle(session)" class="chip running">{{ t("keepAlive.running") }}</span>
            <span v-if="session.attached" class="chip">{{ t("keepAlive.inUse") }}</span>
          </div>
          <div class="sub">
            <span class="path ellipsis" :title="session.path">{{ session.path }}</span>
            <span class="time" :title="formatTime(session.activity)">{{ ago(session.activity) }}</span>
          </div>
        </div>
        <NButton
          class="join"
          :class="{ latest: session.name === latest }"
          size="small"
          type="primary"
          :secondary="session.name !== latest"
          @click="emit('done', session.name)"
        >
          {{ t("keepAlive.join") }}
        </NButton>
      </div>

      <div class="session fresh">
        <div class="slot"><Plus :size="16" :stroke-width="2.2" /></div>
        <div class="words">
          <div class="line">
            <span class="command">{{ t("keepAlive.newSession") }}</span>
          </div>
          <div class="sub">{{ t("keepAlive.newSessionHint") }}</div>
        </div>
        <NButton class="join" size="small" @click="emit('done', '')">{{ t("keepAlive.create") }}</NButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.chooser {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px;
}

.sessions {
  display: flex;
  flex-direction: column;
  max-height: min(420px, 60vh);
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg);
}

.session {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 12px;
  padding: 11px 12px;
  transition: background-color 0.12s;
}

.session + .session {
  border-top: 1px solid var(--border);
}

.session:hover {
  background: var(--bg-hover);
}

/* 会话的编号。里面有程序在跑的用强调色，一眼能和空闲的分开。 */
.slot {
  display: grid;
  flex-shrink: 0;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: var(--fill);
  color: var(--text-dim);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
}

.busy .slot {
  background: var(--accent-soft);
  color: var(--accent);
}

.fresh .slot {
  border: 1px dashed var(--border);
  background: none;
}

.words {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.line {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.command {
  font-weight: 600;
}

.busy .command {
  font-family: var(--font-mono);
}

.chip {
  flex-shrink: 0;
  padding: 1px 7px;
  border-radius: 9px;
  background: var(--fill);
  color: var(--text-dim);
  font-size: 11px;
  line-height: 16px;
}

.chip.running {
  background: color-mix(in srgb, var(--ok) 16%, transparent);
  color: var(--ok);
}

.sub {
  display: flex;
  gap: 8px;
  min-width: 0;
  color: var(--text-dim);
  font-size: 12px;
}

.path {
  font-family: var(--font-mono);
}

.time {
  flex-shrink: 0;
}

.time::before {
  margin-right: 8px;
  content: "·";
}

.join {
  flex-shrink: 0;
  min-width: 60px;
}
</style>
