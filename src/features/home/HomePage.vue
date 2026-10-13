<script setup lang="ts">
import {
  ArrowRight, ChevronRight, Copy, Fingerprint, GlobeLock, GripVertical, HeartPulse, KeyRound, LockKeyhole, MoreVertical, Moon, Pencil,
  Pin, PinOff, Plus, Search, SearchX, Sun, SunMoon, TerminalSquare, Trash2,
} from "@lucide/vue";
import draggable from "vuedraggable";
import { NButton, NInput } from "naive-ui";
import { computed, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import logoUrl from "../../assets/logo.svg";
import type { AuthKind, Profile } from "../../shared/api";
import BrandSlogan from "../../shared/components/BrandSlogan.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { openContextMenu, type MenuEntry } from "../../shared/contextMenu";
import { t } from "../../shared/i18n";
import { THEME_SETTINGS } from "../../shared/theme";
import { CUSTOM_TITLEBAR, OVERLAY_TITLEBAR } from "../../shared/titlebar";
import { confirm } from "../../shared/ui";
import { PINNED_GROUP, useApp } from "../../stores/app";
import { openTunnelDialog } from "../tunnel/tunnelDialog";
import { hostHue, resolveHostIcon } from "./hostIcons";
import IconPicker from "./IconPicker.vue";
import ProfileDialog from "./ProfileDialog.vue";

const app = useApp();
const query = ref("");
const searchInput = ref<InstanceType<typeof NInput>>();
const editing = ref<Profile | null>(null);
const showProfile = ref(false);
const picking = ref<Profile | null>(null);

interface ViewGroup {
  name: string;
  items: Profile[];
  pinned: boolean;
}

/** 没有标题栏时，标记顶栏中可以用来拖动窗口的部分。 */
const drag = OVERLAY_TITLEBAR || undefined;

/**
 * 所有卡片网格共用的 SortableJS 选项；只有拖动手柄能发起拖拽。被拖动的卡片由
 * Sortable 自己绘制（`forceFallback`）：webview 自带的拖拽图像不可靠，这样指针下的
 * 卡片看起来也和真的一样。
 */
const dragOptions = {
  animation: 180,
  handle: ".drag-handle",
  ghostClass: "card-ghost",
  dragClass: "card-drag",
  forceFallback: true,
  fallbackOnBody: true,
  fallbackTolerance: 4,
};

/**
 * 卡片只在页面打开时淡入：排序会重新插入卡片元素，
 * 否则动画会在拖拽过程中重放。
 */
const entering = ref(true);
onMounted(() => setTimeout(() => (entering.value = false), 700));

/** 各种登录方式在卡片上的标记。 */
const AUTH: Record<AuthKind, { icon: Component; label: () => string }> = {
  password: { icon: LockKeyhole, label: () => t("profile.password") },
  key: { icon: KeyRound, label: () => t("profile.key") },
  agent: { icon: Fingerprint, label: () => "Agent" },
};

/** 配置使用了当前版本不认识的登录方式时，退回显示密码标记。 */
const authOf = (p: Profile) => AUTH[p.auth] ?? AUTH.password;

const address = (p: Profile) => `${p.username}@${p.host}${p.port === 22 ? "" : `:${p.port}`}`;

/** 保存在选择器中选定的图标和颜色（点击“保存”时一起生效）。 */
function applyCustomize(value: { icon: string | null; color: number | null }) {
  const id = picking.value?.id;
  // 从 store 读取最新的副本，避免覆盖同时发生的其他修改。
  const current = app.profiles.find((p) => p.id === id);
  if (!current) {
    picking.value = null;
    return;
  }
  const next: Profile = { ...current };
  if (value.icon) next.icon = value.icon;
  else delete next.icon;
  if (value.color !== null) next.color = value.color;
  else delete next.color;
  app.saveProfile(next);
  picking.value = null;
}

/** 卡片的菜单；“更多”按钮和右键点击打开的是同一个。 */
function openCardMenu(event: MouseEvent, p: Profile) {
  const entries: MenuEntry[] = [
    { label: t("sidebar.connect"), icon: TerminalSquare, action: () => app.openTab(p) },
    "divider",
    { label: t(p.pinned ? "sidebar.unpin" : "sidebar.pin"), icon: p.pinned ? PinOff : Pin, action: () => app.togglePinned(p.id) },
    { label: t(p.keepAlive ? "home.keepAliveOff" : "home.keepAliveOn"), icon: HeartPulse, action: () => app.toggleKeepAlive(p.id) },
    // 隧道要改系统的路由，不是每个系统上都做得了。
    ...(app.tunnelSupported ? [{ label: t("tunnel.menu"), icon: GlobeLock, action: () => openTunnelDialog(p.id) }] : []),
    { label: t("common.edit"), icon: Pencil, action: () => edit(p) },
    { label: t("sidebar.duplicate"), icon: Copy, action: () => duplicate(p) },
    "divider",
    { label: t("common.delete"), icon: Trash2, danger: true, action: () => remove(p) },
  ];
  openContextMenu(event, entries);
}

/** 顺序始终跟随配置数组（手动拖拽的顺序）；搜索只做过滤。 */
const groups = computed<ViewGroup[]>(() => {
  const q = query.value.trim().toLowerCase();
  const matched = app.profiles.filter((p) =>
    q ? `${p.name} ${p.host} ${p.username} ${p.group} ${p.note}`.toLowerCase().includes(q) : true,
  );

  const out: ViewGroup[] = [];
  const pinned = matched.filter((p) => p.pinned);
  if (pinned.length) out.push({ name: PINNED_GROUP, pinned: true, items: pinned });

  const byName = new Map<string, Profile[]>();
  for (const p of matched.filter((x) => !x.pinned)) {
    const list = byName.get(p.group) ?? [];
    list.push(p);
    byName.set(p.group, list);
  }
  for (const name of byName.keys()) out.push({ name, pinned: false, items: byName.get(name)! });
  return out;
});

const matchCount = computed(() => groups.value.reduce((n, group) => n + group.items.length, 0));
const groupCount = computed(() => new Set(app.profiles.map((p) => p.group).filter(Boolean)).size);

/** 每个配置已打开的会话，卡片据此显示主机正在使用。 */
const sessions = computed(() => {
  const count = new Map<string, number>();
  for (const tab of app.tabs) {
    if (tab.status !== "closed") count.set(tab.profileId, (count.get(tab.profileId) ?? 0) + 1);
  }
  return count;
});
/** 正在连接的主机：卡片一直显示连接中，直到标签页打开。 */
const connecting = computed(() => new Set(app.connecting.map((tab) => tab.profileId)));

const sessionCount = computed(() => [...sessions.value.values()].reduce((n, c) => n + c, 0));

function groupTitle(group: ViewGroup) {
  if (group.pinned) return t("home.pinned");
  // 页面上没有其他分组时，未分组的主机才叫“全部主机”。
  return group.name || t(groups.value.length > 1 ? "home.ungrouped" : "home.all");
}

function isCollapsed(name: string) {
  // 搜索时显示所有匹配项：折叠的分组不能把搜到的内容藏起来。
  return !query.value.trim() && app.sidebar.collapsed.includes(name);
}

function edit(profile: Profile | null) {
  editing.value = profile;
  showProfile.value = true;
}

async function remove(profile: Profile) {
  if (!(await confirm(t("sidebar.deleteTitle"), t("sidebar.deleteBody", { name: profile.name }), { danger: true, ok: t("common.delete") }))) return;
  app.deleteProfile(profile.id);
}

function duplicate(profile: Profile) {
  const copy = app.duplicateProfile(profile.id);
  if (copy) edit(copy);
}

/** 顶栏按钮上代表各主题的图标。 */
const THEME_ICON = { light: Sun, dark: Moon, auto: SunMoon };

/** 依次换到下一种主题：浅色 → 深色 → 自动。 */
function cycleTheme() {
  const at = THEME_SETTINGS.indexOf(app.settings.theme);
  app.settings.theme = THEME_SETTINGS[(at + 1) % THEME_SETTINGS.length];
}

// ---- 搜索 ------------------------------------------------------------------

/** 在页面任何位置按 `/` 或 ⌘K 都跳到搜索框。 */
function onShortcut(e: KeyboardEvent) {
  const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, [contenteditable]");
  const slash = e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !typing;
  const commandK = e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey;
  // 页面上方有对话框打开时不响应。
  if ((!slash && !commandK) || showProfile.value || picking.value || document.querySelector(".n-modal")) return;
  e.preventDefault();
  searchInput.value?.focus();
  searchInput.value?.select();
}

onMounted(() => window.addEventListener("keydown", onShortcut));
onBeforeUnmount(() => window.removeEventListener("keydown", onShortcut));

// ---- 拖拽手动排序（vuedraggable / SortableJS） -----------------------------

/** 搜索过滤期间禁用，避免用过滤后的子集打乱全局顺序。 */
const canDrag = computed(() => !query.value.trim());

/** vuedraggable 会原地修改 `group.items`；把这个顺序保存到全局列表。 */
function onReorder(group: ViewGroup) {
  app.reorderProfiles(group.items.map((p) => p.id));
}
</script>

<template>
  <div class="home" :class="{ entering }">
    <!-- 上方没有会话标签栏时，这一栏就是窗口的顶部。 -->
    <header class="topbar" :class="{ top: !app.tabs.length }" :data-tauri-drag-region="drag">
      <!-- Windows 上自绘的标题栏里已经有品牌标识，这里不重复；格子留着，搜索框才居中。 -->
      <div class="brand"><BrandSlogan v-if="!CUSTOM_TITLEBAR" class="slogan" /></div>
      <div class="search">
        <NInput ref="searchInput" v-model:value="query" round clearable autofocus :placeholder="t('home.search')"
          @keydown.esc="query = ''">
          <template #prefix>
            <Search :size="15" />
          </template>
          <template v-if="!query" #suffix><kbd class="key">/</kbd></template>
        </NInput>
      </div>
      <div class="top-actions" :data-tauri-drag-region="drag">
        <NButton type="primary" @click="edit(null)">
          <template #icon><Plus :size="16" /></template>
          {{ t("sidebar.new") }}
        </NButton>
        <NButton circle :title="t('theme.switch', { name: t(`theme.${app.settings.theme}`) })" @click="cycleTheme">
          <template #icon><component :is="THEME_ICON[app.settings.theme]" :size="16" /></template>
        </NButton>
      </div>
    </header>

    <div class="scroll">
      <div class="page">
        <!-- 一个配置都没有：引导用户创建第一个。 -->
        <section v-if="!app.profiles.length" class="empty">
          <div class="empty-art"><img :src="logoUrl" alt="" /></div>
          <h2>{{ t("home.emptyTitle") }}</h2>
          <p>{{ t("home.emptyBody") }}</p>
          <NButton class="first-host" type="primary" size="large" @click="edit(null)">
            <template #icon><Plus :size="16" /></template>
            {{ t("sidebar.new") }}
          </NButton>
        </section>

        <template v-else>
          <section class="hero">
            <h1>{{ t("home.title") }}</h1>
            <p v-if="query.trim()" class="stats">{{ t("home.found", { count: matchCount }) }}</p>
            <p v-else class="stats">
              <span><b>{{ app.profiles.length }}</b> {{ t("home.stat.hosts") }}</span>
              <span v-if="groupCount"><b>{{ groupCount }}</b> {{ t("home.stat.groups") }}</span>
              <span v-if="sessionCount" class="live"><i class="pulse" /><b>{{ sessionCount }}</b> {{
                t("home.stat.sessions") }}</span>
            </p>
          </section>

          <section v-if="!groups.length" class="empty no-match">
            <SearchX :size="36" :stroke-width="1.4" />
            <p>{{ t("sidebar.noMatch") }}</p>
            <NButton @click="query = ''">{{ t("home.clearSearch") }}</NButton>
          </section>

          <section v-for="group in groups" :key="group.name" class="group-section">
            <button class="group-head" :aria-expanded="!isCollapsed(group.name)"
              @click="app.toggleGroupCollapsed(group.name)">
              <ChevronRight :size="14" class="chevron" :class="{ open: !isCollapsed(group.name) }" />
              <Pin v-if="group.pinned" :size="13" class="pin-mark" />
              <span class="group-name">{{ groupTitle(group) }}</span>
              <span class="count">{{ group.items.length }}</span>
              <span class="rule" />
            </button>
            <draggable v-if="!isCollapsed(group.name)" :list="group.items" item-key="id" class="grid"
              :disabled="!canDrag" v-bind="dragOptions" @end="onReorder(group)">
              <template #item="{ element, index }">
                <div class="card" :class="{ live: sessions.has(element.id), busy: connecting.has(element.id) }"
                  :style="{ '--h': String(element.color ?? hostHue(element.id)), '--i': Math.min(index, 12) }"
                  :title="element.note ? `${address(element)}\n${element.note}` : address(element)" tabindex="0"
                  @dblclick="app.openTab(element)" @keydown.enter.self="app.openTab(element)"
                  @contextmenu="openCardMenu($event, element)">
                  <div class="card-main">
                    <div class="avatar">
                      <component :is="resolveHostIcon(element.icon)" v-if="resolveHostIcon(element.icon)" :size="22"
                        :stroke-width="1.9" />
                      <span v-else>{{ (element.name.trim()[0] || "?").toUpperCase() }}</span>
                      <NButton class="avatar-edit" circle size="tiny" type="primary" :title="t('profile.customize')"
                        @click.stop="picking = element" @dblclick.stop>
                        <template #icon><Pencil :size="11" /></template>
                      </NButton>
                    </div>
                    <div class="meta">
                      <div class="name-line">
                        <span class="ellipsis name">{{ element.name }}</span>
                        <Pin v-if="element.pinned && !group.pinned" :size="12" class="pin-mark" />
                      </div>
                      <div class="ellipsis addr">{{ element.username }}@{{ element.host }}<span
                          v-if="element.port !== 22" class="port">:{{ element.port }}</span></div>
                    </div>
                  </div>

                  <div class="card-foot">
                    <span class="chip">
                      <component :is="authOf(element).icon" :size="11" />{{ authOf(element).label() }}
                    </span>
                    <span v-if="element.keepAlive" class="chip keep" :title="t('home.keepAliveTip')">
                      <HeartPulse :size="12" />
                    </span>
                    <span v-if="app.tunnel?.profileId === element.id" class="chip tunnel" :title="t('tunnel.chip')">
                      <GlobeLock :size="12" />
                    </span>
                    <span v-if="sessions.has(element.id)" class="chip live"><i class="pulse" />{{ t("home.sessions", {
                      count: sessions.get(element.id)! }) }}</span>
                    <span class="ellipsis note">{{ element.note || (group.pinned ? element.group : "") }}</span>
                    <!-- 连接进行中时转圈、点了没反应；这时它一直显示，不管鼠标在不在卡片上。 -->
                    <NButton class="go" :class="{ busy: connecting.has(element.id) }" size="tiny" type="primary" secondary
                      icon-placement="right" :loading="connecting.has(element.id)" :focusable="false"
                      @click.stop="app.openTab(element)" @dblclick.stop>
                      <template #icon><ArrowRight :size="12" /></template>
                      {{ t(connecting.has(element.id) ? "home.connecting" : "sidebar.connect") }}
                    </NButton>
                  </div>

                  <div class="tools" @dblclick.stop>
                    <IconButton v-if="canDrag" class="drag-handle" size="tiny" :focusable="false" :title="t('home.dragHandle')">
                      <GripVertical :size="14" />
                    </IconButton>
                    <IconButton size="tiny" :title="t('common.more')" @click.stop="openCardMenu($event, element)">
                      <MoreVertical :size="15" />
                    </IconButton>
                  </div>
                </div>
              </template>
            </draggable>
          </section>

          <footer class="help">
            <span class="hint"><kbd class="key">{{ t("home.help.connectKbd") }}</kbd>{{ t("home.help.connect") }}</span>
            <span class="hint"><kbd class="key">{{ t("home.help.dragKbd") }}</kbd>{{ t("home.help.drag") }}</span>
            <span class="hint"><kbd class="key">{{ t("home.help.menuKbd") }}</kbd>{{ t("home.help.menu") }}</span>
          </footer>
        </template>
      </div>
    </div>

    <ProfileDialog v-model:show="showProfile" :profile="editing" />
    <IconPicker :profile="picking" @save="applyCustomize" @close="picking = null" />
  </div>
</template>

<style scoped>
.home {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background:
    radial-gradient(1100px 460px at 50% -180px, var(--hero-glow), transparent 70%),
    var(--bg);
}

/* 在页头下方渐隐的点阵：有质感，又不和卡片抢视线。 */
.home::before {
  content: "";
  position: absolute;
  inset: 0;
  background-image: radial-gradient(var(--dot) 1px, transparent 1px);
  background-size: 22px 22px;
  mask-image: linear-gradient(to bottom, #000, transparent 340px);
  -webkit-mask-image: linear-gradient(to bottom, #000, transparent 340px);
  pointer-events: none;
}

/* ---- 顶栏 ---- */

.topbar {
  position: relative;
  z-index: 2;
  flex-shrink: 0;
  display: grid;
  grid-template-columns: 1fr minmax(160px, 520px) 1fr;
  align-items: center;
  gap: 16px;
  padding: 10px 18px;
  background: color-mix(in srgb, var(--bg-raised) 72%, transparent);
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
  border-bottom: 1px solid var(--border);
}

/* 红绿灯按钮在品牌标识和搜索框上方独占一条。 */
.topbar.top {
  padding-top: calc(10px + var(--lights-height));
}

/* 不接收鼠标：在品牌标识上按下也是拖动窗口。 */
.brand {
  display: flex;
  pointer-events: none;
}

.slogan {
  height: 28px;
}

.top-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.key {
  font-family: inherit;
  font-size: 11px;
  line-height: 1;
  padding: 3px 6px;
  border-radius: 5px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  color: var(--text-dim);
}

/* ---- 页面 ---- */

.scroll {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* 占满宽度：下面的网格每行能放多少卡片，取决于窗口有多宽。 */
.page {
  padding: 28px 32px 48px;
}

.hero {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 6px 18px;
}

.hero h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: 0.2px;
}

.stats {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  margin: 0;
  color: var(--text-dim);
}

.stats b {
  color: var(--text);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.stats .live {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.pulse {
  position: relative;
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--ok);
}

.pulse::after {
  content: "";
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: var(--ok);
  animation: pulse 2s ease-out infinite;
}

/* ---- 空状态 ---- */

.first-host {
  margin-top: 10px;
}

.empty {
  margin-top: 96px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--text-dim);
  text-align: center;
}

.empty h2 {
  margin: 10px 0 0;
  font-size: 18px;
  color: var(--text);
}

.empty p {
  margin: 0;
}

.empty-art {
  display: grid;
  place-items: center;
  width: 88px;
  height: 88px;
  border-radius: 24px;
  border: 1px solid var(--border);
  background: var(--bg-raised);
  box-shadow: 0 24px 60px -20px var(--accent);
}

.empty-art img {
  width: 52px;
  height: 52px;
  border-radius: 13px;
}

.empty.no-match {
  margin-top: 72px;
  gap: 12px;
}

/* ---- 分组 ---- */

.group-section {
  margin-top: 26px;
}

.group-head {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 4px 0;
  margin-bottom: 10px;
  background: none;
  border: none;
  color: var(--text-dim);
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.4px;
  cursor: pointer;
}

.group-head:hover .group-name {
  color: var(--text);
}

.chevron {
  transition: transform 0.15s;
}

.chevron.open {
  transform: rotate(90deg);
}

.count {
  min-width: 18px;
  padding: 1px 6px;
  border-radius: 9px;
  background: var(--fill);
  font-size: 11px;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}

.rule {
  flex: 1;
  height: 1px;
  margin-left: 4px;
  background: linear-gradient(to right, var(--border), transparent);
}

.pin-mark {
  flex-shrink: 0;
  color: var(--accent);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 14px;
}

/* ---- 主机卡片 ---- */

/* 卡片的所有颜色都由它的色相推出（--h，内联设置在卡片上）。 */
.card {
  --tint: hsl(var(--h) 70% 58%);
  --tint-text: hsl(var(--h) 78% 74%);
  --tint-soft: hsl(var(--h) 70% 58% / 15%);
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 0;
  padding: 14px 14px 11px;
  border-radius: 14px;
  border: 1px solid var(--border);
  background:
    linear-gradient(140deg, hsl(var(--h) 70% 58% / 9%), transparent 58%),
    var(--bg-raised);
  outline: none;
  transition: border-color 0.15s;
}

.entering .card {
  animation: card-in 0.24s ease-out backwards;
  animation-delay: calc(var(--i, 0) * 18ms);
}

[data-theme="light"] .card {
  --tint: hsl(var(--h) 68% 48%);
  --tint-text: hsl(var(--h) 70% 36%);
  --tint-soft: hsl(var(--h) 70% 48% / 11%);
  box-shadow: 0 1px 2px rgb(31 45 61 / 5%);
}

/* 悬停时只加深轮廓；真正的提示是浮现出来的操作按钮。 */
.card:hover,
.card:focus-within {
  border-color: hsl(var(--h) 45% 60% / 45%);
}

.card:focus-visible {
  border-color: var(--tint);
}

/* 卡片将要落下的位置：一个用卡片自身颜色描边的空位。 */
.card-ghost {
  border-style: dashed;
  border-color: var(--tint);
  background: var(--tint-soft);
}

.card-ghost>* {
  visibility: hidden;
}

/* 跟随指针的副本。Sortable 负责定位并使其半透明；
   它应该看起来就是从页面上被提起的那张卡片。 */
.card-drag {
  opacity: 1 !important;
  border-color: var(--tint);
  box-shadow: 0 18px 40px -14px var(--shadow), 0 4px 12px -6px var(--shadow);
  cursor: grabbing;
}

.card-main {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.avatar {
  position: relative;
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: 12px;
  font-size: 19px;
  font-weight: 700;
  color: #fff;
  background: linear-gradient(145deg, hsl(var(--h) 66% 58%), hsl(calc(var(--h) + 22) 62% 42%));
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 28%),
    0 6px 14px -8px hsl(var(--h) 75% 40%);
}

/* 有会话打开的主机带一个状态灯。 */
.card.live .avatar::after {
  content: "";
  position: absolute;
  top: -3px;
  right: -3px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--ok);
  border: 2px solid var(--bg-raised);
}

/* 头像角上的“自定义”按钮：悬停在卡片上时才出现。 */
.avatar-edit {
  position: absolute;
  right: -7px;
  bottom: -7px;
  opacity: 0;
}

.card:hover .avatar-edit,
.avatar-edit:focus-visible {
  opacity: 1;
}

.meta {
  flex: 1;
  min-width: 0;
}

.name-line {
  display: flex;
  align-items: center;
  gap: 6px;
  /* 给悬停时出现在角落的工具留出位置。 */
  padding-right: 4px;
}

.name {
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
}

.addr {
  margin-top: 2px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 16px;
  color: var(--text-dim);
}

.port {
  color: var(--tint-text);
}

.card-foot {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 22px;
  padding-top: 10px;
  box-sizing: content-box;
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.chip {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 7px;
  border-radius: 6px;
  background: var(--fill);
  color: var(--text-dim);
  font-size: 11px;
  white-space: nowrap;
}

/* 只显示图标：页脚还要放下登录方式、会话数和备注。 */
.chip.keep {
  padding: 0 5px;
  background: var(--tint-soft);
  color: var(--tint-text);
}

.chip.tunnel {
  padding: 0 5px;
  background: color-mix(in srgb, var(--ok) 15%, transparent);
  color: var(--ok);
}

.chip.live {
  gap: 5px;
  background: color-mix(in srgb, var(--ok) 15%, transparent);
  color: var(--ok);
}

.note {
  flex: 1;
  min-width: 0;
  font-size: 11.5px;
  color: var(--text-dim);
}

/* “连接”按钮：悬停在卡片上时才出现。 */
.go {
  flex-shrink: 0;
  opacity: 0;
}

.card:hover .go,
.card:focus-visible .go,
.card:focus-within .go {
  opacity: 1;
}

.go.busy {
  opacity: 1;
}

.card.busy {
  border-color: hsl(var(--h) 45% 60% / 45%);
}

.tools {
  position: absolute;
  top: 8px;
  right: 8px;
  display: flex;
  padding: 1px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--bg-raised) 88%, transparent);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  opacity: 0;
  transition: opacity 0.12s;
}

.card:hover .tools,
.card:focus-within .tools,
.card-drag .tools {
  opacity: 1;
}

.drag-handle {
  cursor: grab;
  touch-action: none;
}

.drag-handle:active {
  cursor: grabbing;
}

/* ---- 页脚提示 ---- */

.help {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px 22px;
  margin-top: 44px;
  color: var(--text-dim);
  font-size: 12px;
}

.hint {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  white-space: nowrap;
}

@keyframes card-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
}

@keyframes pulse {
  from {
    opacity: 0.7;
    transform: scale(1);
  }

  to {
    opacity: 0;
    transform: scale(2.8);
  }
}

@media (max-width: 720px) {
  .topbar {
    grid-template-columns: auto 1fr auto;
    gap: 10px;
  }

  /* 窗口窄时只留图标：它在标识的最左边，宽度和高度相等。 */
  .slogan {
    width: 28px;
    overflow: hidden;
  }

  .page {
    padding: 20px 16px 40px;
  }
}

@media (prefers-reduced-motion: reduce) {

  .card,
  .pulse::after {
    animation: none;
  }

  .card,
  .chevron {
    transition: none;
  }
}
</style>
