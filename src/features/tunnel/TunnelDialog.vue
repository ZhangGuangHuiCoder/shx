<script setup lang="ts">
// 隧道对话框：以某主机为出口转发本机流量（类似 VPN），设置随主机保存。
import { CircleMinus, Clock, GlobeLock, Info, KeyRound, ListOrdered, Plus, Power, ShieldCheck, TriangleAlert } from "@lucide/vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiPopconfirm from "@/shared/ui-kit/UiPopconfirm.vue";
import UiSpin from "@/shared/ui-kit/UiSpin.vue";
import { computed, nextTick, reactive, ref, watch } from "vue";
import { api, errorMessage, type AppError, type TunnelConfig, type TunnelHelper, type TunnelVia } from "@/shared/api";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { t, type MessageKey } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { formatSize } from "@/shared/util";
import { useApp } from "@/shared/store";
import { completeNetwork, defaultTunnelConfig, EVERYTHING, MAX_RULES, normalizeNetwork, parseTunnelConfig, ruleRow, placedRow, rowsFromRules, rulesFromRows, tunnelProblem, type RuleRow } from "@/features/tunnel/tunnel";
import BuiltinRules from "@/features/tunnel/BuiltinRules.vue";
import CidrInput from "@/features/tunnel/CidrInput.vue";
import { tunnelDialog } from "@/features/tunnel/tunnelDialog";
import ViaSwitch from "@/features/tunnel/ViaSwitch.vue";
import { dangerText, dim, line, notice, okText, statusDot, warnText, well } from "@/shared/ui-kit/styles";

const app = useApp();
const profile = computed(() => app.profiles.find((p) => p.id === tunnelDialog.profileId) ?? null);

const opposite = (via: TunnelVia): TunnelVia => (via === "tunnel" ? "direct" : "tunnel");

/** 内置规则：兜底走向，以及可停用的本地网络规则。 */
const form = reactive({ default: "tunnel" as TunnelVia, lan: { enabled: false, via: "direct" as TunnelVia } });
/** 用户规则，自动排序（最具体的在上）；末尾始终保留一张空的添加卡片。 */
const rows = ref<RuleRow[]>([]);
const parsed = computed(() => rulesFromRows(rows.value));
const config = computed<TunnelConfig>(() => ({ default: form.default, rules: parsed.value.rules, lan: form.lan.enabled ? form.lan.via : null }));
const board = ref<HTMLElement | null>(null);
/** 本机网段，用于说明。 */
const localNets = ref<string[]>([]);
/** 已保存的设置（序列化以便比较）。 */
let stored = "";

const isBlank = (row: RuleRow) => row === rows.value[rows.value.length - 1] && !row.text.trim();

/** 新行默认走向与兜底相反。 */
watch(
  () => rows.value[rows.value.length - 1]?.text,
  (last) => {
    if (last === undefined || last.trim()) rows.value.push(ruleRow(opposite(form.default)));
  },
);

function setDefault(via: TunnelVia) {
  form.default = via;
  const last = rows.value[rows.value.length - 1];
  if (last && isBlank(last)) last.via = opposite(via);
}

function removeRow(row: RuleRow) {
  rows.value = rows.value.filter((r) => r !== row);
}

function place(row: RuleRow) {
  rows.value = placedRow(rows.value, row);
}

/**
 * 失焦时整理该行：补全、规范化，无效则删除，再移到正确位置。
 * 延迟移动，避免此时鼠标正点击别处而点偏。
 */
function leave(event: FocusEvent, row: RuleRow) {
  const element = event.currentTarget as HTMLElement;
  // 焦点仍在本行内。
  if (element.contains(event.relatedTarget as Node | null)) return;
  const net = normalizeNetwork(completeNetwork(row.text));
  // 全部地址即兜底规则。
  if (net === EVERYTHING && row.enabled) {
    setDefault(row.via);
    return removeRow(row);
  }
  if (!net) return void (isBlank(row) || removeRow(row));
  row.text = net;
  // 网段重复时保留新行，删除旧行。
  rows.value = rows.value.filter((other) => other === row || normalizeNetwork(other.text) !== net);
  window.setTimeout(() => element.contains(document.activeElement) || place(row), 150);
}

/** 粘贴多个网段时逐个成行，走向沿用当前行，跳过无效和重复项。 */
function addMany(items: string[], row: RuleRow) {
  const present = new Set(rows.value.map((other) => normalizeNetwork(other.text)));
  const nets = [...new Set(items.flatMap((item) => normalizeNetwork(item) ?? []))].filter((net) => !present.has(net));
  const added = nets.map((net) => ruleRow(row.via, net));
  rows.value.splice(rows.value.length - 1, 0, ...added);
  added.forEach(place);
}

/** 回车跳到末尾空行继续输入。 */
function focusBlank() {
  void nextTick(() => board.value?.querySelector<HTMLElement>(".add-card input")?.focus());
}

/** 免密助手状态；旧版本仍需输一次密码，因此提示更新。不需要助手的系统为 null。 */
const helper = ref<TunnelHelper | null>(null);
const helperBusy = ref(false);
/** 助手按钮：按状态执行安装、更新或卸载（卸载前确认）。 */
const HELPER: Record<TunnelHelper, { label: MessageKey; hint: MessageKey; tone: string }> = {
  missing: { label: "tunnel.helperMissing", hint: "tunnel.helperHint", tone: dim },
  outdated: { label: "tunnel.helperOutdated", hint: "tunnel.helperOutdatedHint", tone: warnText },
  installed: { label: "tunnel.helperInstalled", hint: "tunnel.helperInstalledHint", tone: okText },
};

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
    void api.tunnelHelper().then((state) => (helper.value = state), () => (helper.value = null));
    void api.tunnelLocal().then((nets) => (localNets.value = nets), () => (localNets.value = []));
    // 规范化保存的设置，兼容旧版格式。
    const saved = parseTunnelConfig(shown.tunnel) ?? defaultTunnelConfig();
    Object.assign(form, { default: saved.default, lan: { enabled: saved.lan !== null, via: saved.lan ?? "direct" } });
    rows.value = [...rowsFromRules(saved.rules), ruleRow(opposite(saved.default))];
    stored = JSON.stringify(config.value);
  },
  { immediate: true },
);

const problem = computed(() => {
  const found = tunnelProblem(config.value);
  if (found === "noNetworks") return t("tunnel.noNetworks");
  if (found === "tooManyRules") return t("tunnel.tooManyRules", { max: MAX_RULES });
  return null;
});
// 有未完成的行时不能开启（失焦后会自动补全）。
const valid = computed(() => !problem.value && !parsed.value.incomplete);
const counts = computed(() => {
  const active = parsed.value.rules.filter((rule) => !rule.disabled);
  return { tunnel: active.filter((rule) => rule.via === "tunnel").length, direct: active.filter((rule) => rule.via === "direct").length };
});
const run = computed(() => (app.tunnel && app.tunnel.profileId === profile.value?.id ? app.tunnel : null));
/** 隧道在其他主机上（同时只能有一条）。 */
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

/** 自动保存：只存通过校验且与已存值不同的设置，避免仅打开就写入。 */
watch(config, (current) => {
  const text = JSON.stringify(current);
  if (!profile.value || !valid.value || text === stored) return;
  app.saveTunnelConfig(profile.value.id, current);
  stored = text;
});

function start() {
  if (profile.value && valid.value) void app.startTunnel(profile.value);
}
</script>

<template>
  <UiModal v-model:show="tunnelDialog.open" :mask-closable="false" :auto-focus="false" class="w-lg">
    <template #header>
      <DialogHeader :icon="GlobeLock" :title="t('tunnel.name')" :detail="profile ? `${profile.name} · ${profile.username}@${profile.host}` : ''">
        <template #tagline><span :class="[dim, 'truncate text-xs font-normal']">{{ t("tunnel.intro") }}</span></template>
      </DialogHeader>
    </template>

    <div v-if="profile" class="flex flex-col gap-4 *:shrink-0">
      <div
        v-if="run"
        class="flex flex-col gap-1 rounded-xl border p-3 text-xs/normal"
        :class="run.phase === 'running' ? 'border-emerald-500/40 bg-emerald-500/10' : [line, well]"
      >
        <div class="flex min-w-0 items-center gap-2">
          <UiSpin v-if="run.phase !== 'running'" :size="13" />
          <i v-else :class="[statusDot, 'bg-emerald-500']" />
          <span class="shrink-0 text-sm font-semibold" :class="{ 'text-emerald-700 dark:text-emerald-300': run.phase === 'running' }">{{ t(`tunnel.${run.phase}`) }}</span>
          <span :class="[dim, 'truncate font-mono text-xs']" :title="run.status?.device">{{ run.status?.server }}</span>
        </div>
        <div v-if="traffic" :class="dim">{{ traffic }}</div>
        <div v-if="run.status?.warning" :class="[warnText, 'flex items-start gap-2']"><TriangleAlert :size="13" class="mt-0.5 shrink-0" />{{ run.status.warning }}</div>
        <div v-if="run.phase === 'reconnecting'" :class="dim">{{ t("tunnel.reconnectingHint") }}</div>
      </div>
      <div v-else-if="elsewhere !== null" :class="notice"><GlobeLock :size="15" class="shrink-0" />{{ t("tunnel.elsewhere", { name: elsewhere }) }}</div>

      <div class="flex min-w-0 flex-col gap-2">
        <div :class="[dim, 'flex items-center justify-between text-xs font-semibold']">
          <span>{{ t("tunnel.rules") }}</span>
          <span v-if="parsed.rules.length" class="flex gap-3 font-normal">
            <span class="flex items-center gap-1"><i :class="[statusDot, 'bg-emerald-500']" />{{ t("tunnel.viaTunnel") }} {{ counts.tunnel }}</span>
            <span class="flex items-center gap-1"><i :class="[statusDot, 'bg-zinc-400']" />{{ t("tunnel.viaDirect") }} {{ counts.direct }}</span>
          </span>
        </div>
        <!-- 只有用户规则区滚动，底部内置规则固定。 -->
        <div ref="board" :class="[line, 'flex max-h-76 flex-col overflow-hidden rounded-2xl border', well]">
          <div class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto p-2">
            <div
              v-for="row in rows"
              :key="row.id"
              class="group/card flex h-10 shrink-0 items-center gap-2 rounded-xl border pr-2 pl-3 transition-colors duration-150 focus-within:border-blue-500/50 focus-within:bg-blue-500/8"
              :class="isBlank(row) ? 'add-card border-dashed border-zinc-900/15 hover:border-zinc-900/30 dark:border-white/15 dark:hover:border-white/30' : 'border-zinc-900/10 bg-white/70 shadow-xs hover:border-zinc-900/20 dark:border-white/10 dark:bg-white/5 dark:hover:border-white/20'"
              :title="isBlank(row) ? t('tunnel.rulePlaceholder') : undefined"
              @focusout="leave($event, row)"
            >
              <!-- 空卡片显示加号，填入后变为启用开关。 -->
              <Plus v-if="isBlank(row)" :size="15" :stroke-width="2" :class="[dim, 'shrink-0 group-focus-within/card:text-blue-500']" />
              <UiCheckbox v-else v-model:checked="row.enabled" :aria-label="t('tunnel.ruleEnabled')" />
              <CidrInput
                v-model="row.text"
                class="min-w-0 flex-1"
                :class="!row.enabled && 'opacity-50'"
                :example="isBlank(row)"
                :label="t('tunnel.ruleNet')"
                @many="(items) => addMany(items, row)"
                @enter="focusBlank"
              />
              <ViaSwitch v-if="!isBlank(row)" v-model="row.via" :class="!row.enabled && 'opacity-50'" />
              <IconButton
                v-if="!isBlank(row)"
                type="error"
                size="tiny"
                class="shrink-0 opacity-50 transition-opacity duration-150 group-focus-within/card:opacity-100 group-hover/card:opacity-100"
                :aria-label="t('common.delete')"
                :title="t('common.delete')"
                @click="removeRow(row)"
              >
                <CircleMinus :size="15" :stroke-width="1.8" />
              </IconButton>
            </div>
          </div>
        </div>
        <BuiltinRules
          :class="[line, 'flex max-h-76 flex-col overflow-hidden rounded-2xl border']"
          :lan="form.lan"
          :fallback="form.default"
          :local-nets="localNets"
          @update:lan="(v: { enabled: boolean; via: TunnelVia }) => Object.assign(form.lan, v)"
          @update:fallback="setDefault"
        />
        <span v-if="problem" :class="[dangerText, 'text-xs']">{{ problem }}</span>
      </div>

      <!-- 说明文字。 -->
      <ul :class="[dim, 'm-0 flex list-none flex-col gap-1 px-1 py-0 text-xs/relaxed *:flex *:items-start *:gap-2']">
        <li><ListOrdered :size="13" class="mt-1 shrink-0 opacity-80" />{{ t("tunnel.rulesHint") }}</li>
        <li><Info :size="13" class="mt-1 shrink-0 opacity-80" />{{ t("tunnel.limits") }}</li>
        <li><ShieldCheck :size="13" class="mt-1 shrink-0 opacity-80" />{{ t("tunnel.privilege") }}</li>
        <li v-if="run"><Clock :size="13" class="mt-1 shrink-0 opacity-80" />{{ t("tunnel.editWhileRunning") }}</li>
      </ul>
    </div>

    <template #footer>
      <div class="flex w-full items-center gap-3">
        <!-- 免密助手，不需要的系统不显示。 -->
        <UiPopconfirm v-if="helper !== null" :disabled="helper !== 'installed'" :positive-text="t('tunnel.helperRemove')" :negative-text="t('common.cancel')" @positive-click="setHelper(false)">
          <template #trigger>
            <UiButton :title="t(HELPER[helper].hint)" :loading="helperBusy" :disabled="helperBusy" @click="helper !== 'installed' && setHelper(true)">
              <template #icon><KeyRound :size="15" /></template>
              {{ t("tunnel.helper")
              }}<span
                :class="[line, 'ml-2 border-l pl-2', HELPER[helper].tone]"
                >{{ t(HELPER[helper].label) }}</span
              >
            </UiButton>
          </template>
          {{ t("tunnel.helperRemoveConfirm") }}
        </UiPopconfirm>
        <div class="ml-auto flex gap-2">
          <UiButton @click="tunnelDialog.open = false">{{ t("common.close") }}</UiButton>
          <UiButton v-if="run" type="error" :loading="busy" :disabled="busy" @click="app.stopTunnel()">{{ t(busy ? `tunnel.${run.phase}` : "tunnel.stop") }}</UiButton>
          <UiButton v-else type="primary" :disabled="!valid || elsewhere !== null" @click="start">
            <template #icon><Power :size="15" /></template>
            {{ t("tunnel.start") }}
          </UiButton>
        </div>
      </div>
    </template>
  </UiModal>
</template>
