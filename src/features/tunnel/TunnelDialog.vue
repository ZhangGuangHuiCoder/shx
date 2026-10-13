<script setup lang="ts">
// 隧道对话框：把一台主机当作出口，本机的流量经它出去（类似 VPN）。
// 从上到下：现在的状态、哪些流量走隧道。设置跟着主机保存。
import { Globe, GlobeLock, Info, KeyRound, Power, ShieldCheck, Split, TriangleAlert } from "@lucide/vue";
import { NButton, NInput, NModal, NSpace, NSpin, NSwitch } from "naive-ui";
import { computed, reactive, ref, watch, type Component } from "vue";
import { api, errorMessage, type AppError, type TunnelConfig } from "../../shared/api";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import { t, type MessageKey } from "../../shared/i18n";
import { message } from "../../shared/ui";
import { formatSize } from "../../shared/util";
import { useApp } from "../../stores/app";
import { defaultTunnelConfig, MAX_NETWORKS, parseNetworks, parseTunnelConfig, tunnelProblem } from "./tunnel";
import { tunnelDialog } from "./tunnelDialog";

const app = useApp();
const profile = computed(() => app.profiles.find((p) => p.id === tunnelDialog.profileId) ?? null);

const MODES: { value: TunnelConfig["route"]; icon: Component; label: MessageKey; hint: MessageKey }[] = [
  { value: "all", icon: Globe, label: "tunnel.routeAll", hint: "tunnel.routeAllHint" },
  { value: "include", icon: Split, label: "tunnel.routeInclude", hint: "tunnel.routeIncludeHint" },
];

const form = reactive<TunnelConfig>(defaultTunnelConfig());
/** 两个网段列表在输入框里的样子：每行一个。 */
const cidrsText = ref("");
const bypassText = ref("");

/** 助手是不是装进了系统：装了之后开隧道不用再输密码。没有这回事的系统上是 null。 */
const helper = ref<boolean | null>(null);
const helperBusy = ref(false);

async function setHelper(installed: boolean) {
  helperBusy.value = true;
  try {
    await api.tunnelHelperSet(installed);
  } catch (e) {
    if ((e as AppError).kind !== "cancelled") message.error(errorMessage(e));
  }
  helper.value = await api.tunnelHelper().catch(() => null);
  helperBusy.value = false;
}

watch(
  () => tunnelDialog.open && profile.value,
  (shown) => {
    if (!shown) return;
    void api.tunnelHelper().then((installed) => (helper.value = installed), () => (helper.value = null));
    // 存下来的设置过一遍校验：旧版本留下的、现在已经没有的项就此丢掉。
    Object.assign(form, parseTunnelConfig(shown.tunnel) ?? defaultTunnelConfig());
    cidrsText.value = form.cidrs.join("\n");
    bypassText.value = form.bypass.join("\n");
  },
  { immediate: true },
);

const cidrs = computed(() => parseNetworks(cidrsText.value));
const bypass = computed(() => parseNetworks(bypassText.value));
/** 当前要用的那个列表里不是网段的条目。 */
const invalid = computed(() => (form.route === "include" ? cidrs.value.invalid : bypass.value.invalid));
const config = computed<TunnelConfig>(() => ({ route: form.route, cidrs: cidrs.value.networks, bypass: bypass.value.networks }));
/** 设置里妨碍保存和开启的问题。 */
const problem = computed(() => {
  if (invalid.value.length) return t("tunnel.invalidNetworks", { items: invalid.value.slice(0, 5).join(t("common.listSeparator")) });
  const found = tunnelProblem(config.value);
  if (found === "noNetworks") return t("tunnel.noNetworks");
  if (found === "tooManyNetworks") return t("tunnel.tooManyNetworks", { max: MAX_NETWORKS });
  return null;
});
const valid = computed(() => !problem.value);

/** 隧道正以这台主机为出口。 */
const run = computed(() => (app.tunnel && app.tunnel.profileId === profile.value?.id ? app.tunnel : null));
/** 隧道正开在别的主机上：同一时间只能有一条。 */
const elsewhere = computed(() => (app.tunnel && !run.value ? app.profiles.find((p) => p.id === app.tunnel!.profileId)?.name ?? "" : null));
const busy = computed(() => run.value?.phase === "starting" || run.value?.phase === "stopping");

const traffic = computed(() => {
  const current = run.value;
  if (!current?.status) return "";
  const { connections, up, down } = current.status;
  return t("tunnel.traffic", {
    connections,
    up: formatSize(up),
    upRate: formatSize(current.upRate),
    down: formatSize(down),
    downRate: formatSize(current.downRate),
  });
});

function save(): boolean {
  if (!profile.value || !valid.value) return false;
  app.saveTunnelConfig(profile.value.id, config.value);
  return true;
}

function saveOnly() {
  if (save()) message.success(t("common.saved"));
}

function start() {
  if (profile.value && save()) void app.startTunnel(profile.value);
}
</script>

<template>
  <NModal v-model:show="tunnelDialog.open" preset="card" style="width: 520px" :bordered="false" :mask-closable="false">
    <template #header>
      <DialogHeader :icon="GlobeLock" :title="t('tunnel.name')" :detail="profile ? `${profile.name} · ${profile.username}@${profile.host}` : ''">
        <template #tagline><span class="tagline">{{ t("tunnel.intro") }}</span></template>
      </DialogHeader>
    </template>

    <div v-if="profile" class="tunnel">
      <div v-if="run" class="state" :class="run.phase">
        <div class="state-line">
          <NSpin v-if="run.phase !== 'running'" :size="13" />
          <i v-else class="status-dot connected" />
          <span class="state-name">{{ t(`tunnel.${run.phase}`) }}</span>
          <span class="ellipsis dim mono" :title="run.status?.device">{{ run.status?.server }}</span>
        </div>
        <div v-if="traffic" class="dim">{{ traffic }}</div>
        <div v-if="run.status?.warning" class="warning"><TriangleAlert :size="13" />{{ run.status.warning }}</div>
        <div v-if="run.phase === 'reconnecting'" class="dim">{{ t("tunnel.reconnectingHint") }}</div>
      </div>
      <div v-else-if="elsewhere !== null" class="notice"><GlobeLock :size="15" />{{ t("tunnel.elsewhere", { name: elsewhere }) }}</div>

      <div class="field">
        <span class="label">{{ t("tunnel.route") }}</span>
        <div class="modes" role="radiogroup" :aria-label="t('tunnel.route')">
          <button v-for="mode in MODES" :key="mode.value" type="button" role="radio" class="mode" :aria-checked="form.route === mode.value"
            :class="{ on: form.route === mode.value }" @click="form.route = mode.value">
            <span class="mode-head">
              <component :is="mode.icon" :size="15" :stroke-width="1.9" />
              <span class="mode-name">{{ t(mode.label) }}</span>
              <i class="mode-mark" />
            </span>
            <span class="mode-hint">{{ t(mode.hint) }}</span>
          </button>
        </div>
      </div>

      <div class="field">
        <span class="label">{{ t(form.route === "include" ? "tunnel.cidrs" : "tunnel.bypass") }}</span>
        <NInput v-if="form.route === 'include'" v-model:value="cidrsText" type="textarea" class="mono" :autosize="{ minRows: 3, maxRows: 6 }" :status="problem ? 'error' : undefined" :placeholder="t('tunnel.networksPlaceholder')" />
        <NInput v-else v-model:value="bypassText" type="textarea" class="mono" :autosize="{ minRows: 3, maxRows: 6 }" :status="problem ? 'error' : undefined" :placeholder="t('tunnel.networksPlaceholder')" />
        <span v-if="problem" class="problem">{{ problem }}</span>
      </div>
      <p v-if="run" class="hint">{{ t("tunnel.editWhileRunning") }}</p>

      <!-- 免密码的开关是一项设置，单独成行，不和下面的说明混在一起。 -->
      <div v-if="helper !== null" class="helper">
        <div class="helper-icon"><KeyRound :size="15" :stroke-width="1.9" /></div>
        <div class="helper-text">
          <div class="helper-name">{{ t("tunnel.helper") }}</div>
          <div class="dim">{{ t("tunnel.helperHint") }}</div>
        </div>
        <NSwitch :value="helper" :loading="helperBusy" :disabled="helperBusy" :aria-label="t('tunnel.helper')" @update:value="setHelper" />
      </div>

      <!-- 用之前该知道的两件事：带得了什么，会动系统的什么。 -->
      <ul class="notes">
        <li><Info :size="13" />{{ t("tunnel.limits") }}</li>
        <li><ShieldCheck :size="13" />{{ t("tunnel.privilege") }}</li>
      </ul>
    </div>

    <template #footer>
      <NSpace justify="space-between">
        <NButton v-if="run" type="error" secondary :loading="busy" :disabled="busy" @click="app.stopTunnel()">{{ t(busy ? `tunnel.${run.phase}` : "tunnel.stop") }}</NButton>
        <NButton v-else type="primary" :disabled="!valid || elsewhere !== null" @click="start">
          <template #icon><Power :size="15" /></template>
          {{ t("tunnel.start") }}
        </NButton>
        <NSpace>
          <NButton @click="tunnelDialog.open = false">{{ t("common.close") }}</NButton>
          <NButton :disabled="!valid" @click="saveOnly">{{ t("common.save") }}</NButton>
        </NSpace>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
/* 内容比窗口高时在框里滚动，标题和底栏的按钮始终看得到。 */
.tunnel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-height: calc(100vh - 230px);
  /* 右边让出滚动条的位置，输入框的描边不被它压住。 */
  margin-right: -10px;
  padding-right: 10px;
  overflow-y: auto;
}

.tunnel > * {
  flex-shrink: 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.label {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}

/* 标题后面的一句说明：超长时省略，不挤占主机信息那一行。 */
.tagline {
  overflow: hidden;
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 400;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 免密码的开关：图标、说明、开关排成一行，描边把它和别的设置分开。 */
.helper {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  font-size: 12px;
  line-height: 1.5;
}

.helper-icon {
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--fill);
  color: var(--text-dim);
}

.helper-text {
  flex: 1;
  min-width: 0;
}

.helper-name {
  font-size: 13px;
  font-weight: 600;
}

/* 说明放在最下面，字小、颜色淡：要看的时候找得到，不看的时候不挡着设置。 */
.notes {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0 2px;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.6;
  list-style: none;
}

.notes li {
  display: flex;
  align-items: flex-start;
  gap: 7px;
}

.notes svg {
  flex-shrink: 0;
  margin-top: 3px;
  opacity: 0.8;
}

.hint {
  margin: 0;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.6;
}

.problem {
  color: var(--danger);
  font-size: 12px;
}

.dim {
  color: var(--text-dim);
}

.mono,
.mono :deep(textarea) {
  font-family: var(--font-mono);
  font-size: 12px;
}

.mono :deep(textarea) {
  line-height: 1.6;
}

/* 路由模式：两张并排的卡片，各带一句说明；选中的那张描上强调色。 */
.modes {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.mode {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: none;
  color: var(--text-dim);
  font: inherit;
  font-size: 12px;
  line-height: 1.5;
  text-align: left;
  cursor: pointer;
  transition:
    background-color 0.12s,
    border-color 0.12s;
}

.mode:hover {
  background: var(--fill);
}

.mode.on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.mode:focus-visible {
  outline: 2px solid var(--accent-border);
  outline-offset: 1px;
}

.mode-head {
  display: flex;
  align-items: center;
  gap: 7px;
}

.mode-name {
  flex: 1;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
}

.mode.on .mode-head {
  color: var(--accent);
}

.mode.on .mode-name {
  color: var(--accent);
}

/* 右上角的单选圆点。 */
.mode-mark {
  flex-shrink: 0;
  width: 14px;
  height: 14px;
  border: 1.5px solid var(--border);
  border-radius: 50%;
  transition:
    border-color 0.12s,
    border-width 0.12s;
}

.mode.on .mode-mark {
  border-width: 4px;
  border-color: var(--accent);
}

/* 隧道现在的状态：运行中是好消息的颜色，其余时候是进行中的中性色。 */
.state {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
  font-size: 12px;
  line-height: 1.5;
}

.state.running {
  border-color: color-mix(in srgb, var(--ok) 45%, transparent);
  background: color-mix(in srgb, var(--ok) 9%, transparent);
}

.state-line {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.state-name {
  flex-shrink: 0;
  font-size: 13px;
  font-weight: 600;
}

.state.running .state-name {
  color: var(--ok);
}

.warning {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  color: var(--pin);
}

.warning svg {
  flex-shrink: 0;
  margin-top: 2px;
}

.notice {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--fill);
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.5;
}

.notice svg {
  flex-shrink: 0;
}
</style>
