<script setup lang="ts">
import {
  ChevronRight, Copy, Fingerprint, GlobeLock, GripVertical, HeartPulse, KeyRound, LockKeyhole, MessageSquareText, MoreVertical, Moon, Pencil,
  Pin, PinOff, Plus, Search, SearchX, Sun, SunMoon, TerminalSquare, Trash2,
} from "@lucide/vue";
import draggable from "vuedraggable";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import { computed, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import logoUrl from "@/assets/logo.svg";
import type { AuthKind, Profile } from "@/shared/api";
import BrandSlogan from "@/shared/ui-kit/BrandSlogan.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { contextMenu, openContextMenu, type MenuEntry } from "@/shared/contextMenu";
import { t, unitGap } from "@/shared/i18n";
import { THEME_SETTINGS } from "@/shared/theme";
import { CUSTOM_TITLEBAR, OVERLAY_TITLEBAR } from "@/shared/titlebar";
import { confirm } from "@/shared/ui";
import { PINNED_GROUP, useApp } from "@/shared/store";
import { openTunnelDialog } from "@/features/tunnel/tunnelDialog";
import { hostHue, hostTint, resolveHostIcon } from "@/features/home/hostIcons";
import { accentText, dim, fg, glassBar, glassCard, keycap, line, okChip, plainChip, toolGroup } from "@/shared/ui-kit/styles";
import IconPicker from "@/features/home/IconPicker.vue";
import ProfileDialog from "@/features/home/ProfileDialog.vue";

const app = useApp();
const query = ref("");
const searchInput = ref<InstanceType<typeof UiInput>>();
const editing = ref<Profile | null>(null);
const showProfile = ref(false);
const picking = ref<Profile | null>(null);

interface ViewGroup {
  name: string;
  items: Profile[];
  pinned: boolean;
}

/** 无标题栏时可拖动窗口的区域。 */
const drag = OVERLAY_TITLEBAR || undefined;

/** SortableJS 选项：仅手柄可拖；forceFallback 自绘拖拽图像，webview 自带的不可靠。 */
const dragOptions = {
  animation: 180,
  handle: ".drag-handle",
  // Sortable 只接受单个类名。
  ghostClass: "opacity-30",
  dragClass: "shadow-2xl",
  forceFallback: true,
  fallbackOnBody: true,
  fallbackTolerance: 4,
};

/** 仅在页面打开时淡入，避免排序重新插入元素时重放动画。 */
const entering = ref(true);
onMounted(() => setTimeout(() => (entering.value = false), 700));

const AUTH: Record<AuthKind, { icon: Component; label: () => string }> = {
  password: { icon: LockKeyhole, label: () => t("profile.password") },
  key: { icon: KeyRound, label: () => t("profile.key") },
  agent: { icon: Fingerprint, label: () => "Agent" },
};

/** 未知登录方式回退为密码标记。 */
const authOf = (p: Profile) => AUTH[p.auth] ?? AUTH.password;

const HELP = ["connect", "drag", "menu"] as const;

const tintOf = (p: Profile) => hostTint(p.color ?? hostHue(p.id));

const chip = "inline-flex h-5 shrink-0 items-center gap-1 rounded-md text-xs whitespace-nowrap";

const address = (p: Profile) => `${p.username}@${p.host}${p.port === 22 ? "" : `:${p.port}`}`;

function applyCustomize(value: { icon: string | null; color: number | null }) {
  const id = picking.value?.id;
  // 读取最新副本，避免覆盖并发修改。
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

function openCardMenu(event: MouseEvent, p: Profile) {
  const entries: MenuEntry[] = [
    { label: t("sidebar.connect"), icon: TerminalSquare, action: () => app.openTab(p) },
    "divider",
    { label: t(p.pinned ? "sidebar.unpin" : "sidebar.pin"), icon: p.pinned ? PinOff : Pin, action: () => app.togglePinned(p.id) },
    { label: t(p.keepAlive ? "home.keepAliveOff" : "home.keepAliveOn"), icon: HeartPulse, action: () => app.toggleKeepAlive(p.id) },
    // 并非所有系统都支持隧道。
    ...(app.tunnelSupported ? [{ label: t("tunnel.menu"), icon: GlobeLock, action: () => openTunnelDialog(p.id) }] : []),
    { label: t("common.edit"), icon: Pencil, action: () => edit(p) },
    { label: t("sidebar.duplicate"), icon: Copy, action: () => duplicate(p) },
    "divider",
    { label: t("common.delete"), icon: Trash2, danger: true, action: () => remove(p) },
  ];
  // 菜单在 body 中，卡片失去 :hover，用 anchor 保持高亮。
  openContextMenu(event, entries, p.id);
}

/** 顺序跟随配置数组，搜索只过滤。 */
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

const sessions = computed(() => {
  const count = new Map<string, number>();
  for (const tab of app.tabs) {
    if (tab.status !== "closed") count.set(tab.profileId, (count.get(tab.profileId) ?? 0) + 1);
  }
  return count;
});
const connecting = computed(() => new Set(app.connecting.map((tab) => tab.profileId)));

const sessionCount = computed(() => [...sessions.value.values()].reduce((n, c) => n + c, 0));

function groupTitle(group: ViewGroup) {
  if (group.pinned) return t("home.pinned");
  // 只有未分组时标题为“全部主机”。
  return group.name || t(groups.value.length > 1 ? "home.ungrouped" : "home.all");
}

function isCollapsed(name: string) {
  // 搜索时忽略折叠状态。
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

const THEME_ICON = { light: Sun, dark: Moon, auto: SunMoon };

function cycleTheme() {
  const at = THEME_SETTINGS.indexOf(app.settings.theme);
  app.settings.theme = THEME_SETTINGS[(at + 1) % THEME_SETTINGS.length];
}

/** 按 / 或 ⌘K 聚焦搜索框。 */
function onShortcut(e: KeyboardEvent) {
  const typing = e.target instanceof HTMLElement && e.target.closest("input, textarea, [contenteditable]");
  const slash = e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !typing;
  const commandK = e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey;
  // 有对话框时不响应。
  if ((!slash && !commandK) || showProfile.value || picking.value || document.querySelector("[role='dialog']")) return;
  e.preventDefault();
  searchInput.value?.focus();
  searchInput.value?.select();
}

onMounted(() => window.addEventListener("keydown", onShortcut));
onBeforeUnmount(() => window.removeEventListener("keydown", onShortcut));

/** 搜索时禁用，避免子集打乱全局顺序。 */
const canDrag = computed(() => !query.value.trim());

/** vuedraggable 原地修改 group.items，需同步回全局顺序。 */
function onReorder(group: ViewGroup) {
  app.reorderProfiles(group.items.map((p) => p.id));
}
</script>

<template>
  <div class="relative flex h-full flex-col overflow-hidden">
    <!-- 作为窗口顶栏，macOS 红绿灯按钮位于其上方。 -->
    <header
      :class="[glassBar, line, 'relative z-10 flex shrink-0 items-center gap-4 border-b px-5 pb-3', 'pt-3', !app.tabs.length && 'lights:pt-9']"
      :data-tauri-drag-region="drag"
    >
      <!-- Windows 标题栏已有品牌标识，不重复；不接收鼠标以便拖动窗口。 -->
      <BrandSlogan v-if="!CUSTOM_TITLEBAR" class="pointer-events-none mr-2 h-7 max-md:w-7 max-md:overflow-hidden" />
      <div class="w-130 min-w-40 shrink">
        <UiInput ref="searchInput" v-model:value="query" round clearable autofocus :placeholder="t('home.search')" @keydown.esc="query = ''">
          <template #prefix>
            <Search :size="15" :class="dim" />
          </template>
          <template v-if="!query" #suffix><kbd :class="keycap">/</kbd></template>
        </UiInput>
      </div>
      <!-- 可拖动窗口的空白。 -->
      <span class="flex-1 self-stretch" :data-tauri-drag-region="drag" />
      <div class="flex shrink-0 items-center gap-2" :data-tauri-drag-region="drag">
        <UiButton type="primary" @click="edit(null)">
          <template #icon><Plus :size="16" /></template>
          {{ t("sidebar.new") }}
        </UiButton>
        <UiButton circle :title="t('theme.switch', { name: t(`theme.${app.settings.theme}`) })" @click="cycleTheme">
          <template #icon><component :is="THEME_ICON[app.settings.theme]" :size="16" /></template>
        </UiButton>
      </div>
    </header>

    <div class="relative min-h-0 flex-1 overflow-y-auto">
      <div class="px-8 pt-7 pb-12 max-md:px-4 max-md:pt-5 max-md:pb-10">
        <!-- 空状态引导。 -->
        <section v-if="!app.profiles.length" :class="[dim, 'mt-24 flex flex-col items-center gap-2 text-center']">
          <div :class="[glassCard, 'grid size-22 place-items-center rounded-3xl shadow-2xl shadow-blue-500/30']"><img :src="logoUrl" alt="" class="size-13 rounded-2xl" /></div>
          <h2 :class="[fg, 'm-0 mt-3 text-lg font-semibold']">{{ t("home.emptyTitle") }}</h2>
          <p class="m-0">{{ t("home.emptyBody") }}</p>
          <UiButton class="mt-3" type="primary" size="large" @click="edit(null)">
            <template #icon><Plus :size="16" /></template>
            {{ t("sidebar.new") }}
          </UiButton>
        </section>

        <template v-else>
          <section class="flex flex-wrap items-baseline gap-x-5 gap-y-2">
            <h1 :class="[fg, 'm-0 text-2xl font-bold tracking-tight']">{{ t("home.title") }}</h1>
            <p v-if="query.trim()" :class="[dim, 'm-0 flex flex-wrap gap-x-4 gap-y-1']">{{ t("home.found", { count: matchCount }) }}</p>
            <p v-else :class="[dim, 'm-0 flex flex-wrap gap-x-4 gap-y-1']">
              <span><b :class="[fg, 'font-semibold tabular-nums']">{{ app.profiles.length }}</b>{{ unitGap }}{{ t("home.stat.hosts") }}</span>
              <span v-if="groupCount"><b :class="[fg, 'font-semibold tabular-nums']">{{ groupCount }}</b>{{ unitGap }}{{ t("home.stat.groups") }}</span>
              <span v-if="sessionCount" class="inline-flex items-center gap-2">
                <i class="relative size-2 shrink-0 rounded-full bg-emerald-500"><i class="absolute inset-0 rounded-full bg-emerald-500 motion-safe:animate-ping" /></i>
                <span><b :class="[fg, 'font-semibold tabular-nums']">{{ sessionCount }}</b>{{ unitGap }}{{ t("home.stat.sessions") }}</span>
              </span>
            </p>
          </section>

          <section v-if="!groups.length" :class="[dim, 'mt-18 flex flex-col items-center gap-3 text-center']">
            <SearchX :size="36" :stroke-width="1.4" />
            <p class="m-0">{{ t("sidebar.noMatch") }}</p>
            <UiButton @click="query = ''">{{ t("home.clearSearch") }}</UiButton>
          </section>

          <section v-for="group in groups" :key="group.name" class="mt-7">
            <button
              :class="[dim, 'group/head mb-3 flex w-full cursor-pointer items-center gap-2 bg-transparent py-1 text-xs font-semibold']"
              :aria-expanded="!isCollapsed(group.name)"
              @click="app.toggleGroupCollapsed(group.name)"
            >
              <ChevronRight :size="14" class="transition-transform duration-150 motion-reduce:transition-none" :class="{ 'rotate-90': !isCollapsed(group.name) }" />
              <Pin v-if="group.pinned" :size="13" :class="[accentText, 'shrink-0']" />
              <span class="group-hover/head:text-zinc-800 dark:group-hover/head:text-zinc-100">{{ groupTitle(group) }}</span>
              <span class="min-w-5 rounded-full bg-zinc-900/5 px-2 py-px text-xs font-medium tabular-nums dark:bg-white/10">{{ group.items.length }}</span>
              <span class="ml-1 h-px flex-1 bg-linear-to-r from-zinc-900/10 to-transparent dark:from-white/10" />
            </button>
            <!-- 卡片不做位移：内部玻璃按钮带背景模糊，父元素 transform 会导致 macOS WebKit 画出白块。 -->
            <draggable
              v-if="!isCollapsed(group.name)"
              :list="group.items"
              item-key="id"
              class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5"
              :disabled="!canDrag"
              v-bind="dragOptions"
              @end="onReorder(group)"
            >
              <template #item="{ element }">
                <div
                  :class="[
                    glassCard,
                    tintOf(element).hover,
                    'group/card relative isolate flex min-w-0 flex-col gap-3 overflow-hidden rounded-2xl p-4 outline-none',
                    // 菜单打开期间用 card-menu-open 保持悬停态。
                    'transition-shadow duration-300 ease-out hover:shadow-xl [&.card-menu-open]:shadow-xl motion-reduce:transition-none',
                    connecting.has(element.id) && tintOf(element).line,
                    contextMenu.show && contextMenu.anchor === element.id && 'card-menu-open',
                    entering && 'motion-safe:starting:translate-y-1.5 motion-safe:starting:opacity-0',
                  ]"
                  :title="element.note ? `${address(element)}\n${element.note}` : address(element)"
                  tabindex="0"
                  @dblclick="app.openTab(element)"
                  @keydown.enter.self="app.openTab(element)"
                  @contextmenu="openCardMenu($event, element)"
                >
                  <div :class="[tintOf(element).glow, 'pointer-events-none absolute inset-0 -z-10 bg-linear-to-br to-transparent to-60%']" aria-hidden="true" />
                  <div class="flex min-w-0 items-center gap-3">
                    <div :class="[tintOf(element).avatar, 'relative grid size-11 shrink-0 place-items-center rounded-xl bg-linear-to-br text-lg font-bold text-white shadow-lg inset-shadow-xs inset-shadow-white/30']">
                      <component :is="resolveHostIcon(element.icon)" v-if="resolveHostIcon(element.icon)" :size="22" :stroke-width="1.9" />
                      <span v-else>{{ (element.name.trim()[0] || "?").toUpperCase() }}</span>
                      <!-- 有会话时显示绿点。 -->
                      <span v-if="sessions.has(element.id)" class="absolute -top-1 -right-1 size-3 rounded-full border-2 border-white bg-emerald-500 dark:border-zinc-800" />
                      <UiButton
                        class="absolute -right-2 -bottom-2 opacity-0 group-focus-within/card:opacity-100 group-hover/card:opacity-100 group-[.card-menu-open]/card:opacity-100"
                        circle
                        size="tiny"
                        type="primary"
                        :title="t('profile.customize')"
                        @click.stop="picking = element"
                        @dblclick.stop
                      >
                        <template #icon><Pencil :size="11" /></template>
                      </UiButton>
                    </div>
                    <div class="min-w-0 flex-1">
                      <!-- 为右上角工具按钮留位。 -->
                      <div class="flex items-center gap-2 pr-1">
                        <span :class="[fg, 'truncate text-sm/5 font-semibold']">{{ element.name }}</span>
                        <Pin v-if="element.pinned && !group.pinned" :size="12" :class="[accentText, 'shrink-0']" />
                      </div>
                      <div :class="[dim, 'mt-1 truncate font-mono text-xs/4']">
                        {{ element.username }}@{{ element.host }}<span v-if="element.port !== 22" :class="tintOf(element).text">:{{ element.port }}</span>
                      </div>
                    </div>
                  </div>

                  <div :class="[line, 'relative box-content flex h-6 min-w-0 items-center gap-1 border-t pt-3']">
                    <span :class="[chip, plainChip, 'px-1.5']">
                      <component :is="authOf(element).icon" :size="12" />{{ authOf(element).label() }}
                    </span>
                    <span v-if="element.keepAlive" :class="[chip, tintOf(element).chip, 'w-5 justify-center']" :title="t('home.keepAliveTip')">
                      <HeartPulse :size="12" />
                    </span>
                    <!-- 备注只显示图标，悬停查看全文。 -->
                    <span v-if="element.note" :class="[chip, plainChip, 'w-5 justify-center']" :title="element.note">
                      <MessageSquareText :size="12" />
                    </span>
                    <span v-if="app.tunnel?.profileId === element.id" :class="[chip, okChip, 'w-5 justify-center']" :title="t('tunnel.chip')">
                      <GlobeLock :size="12" />
                    </span>
                    <span v-if="sessions.has(element.id)" :class="[chip, okChip, 'px-1.5']">
                      <i class="size-1.5 shrink-0 rounded-full bg-emerald-500" />{{ t("home.sessions", { count: sessions.get(element.id)! }) }}
                    </span>
                    <!-- 置顶分组中注明原分组，同时把“连接”按钮推到右端。 -->
                    <span :class="[dim, 'ml-0.5 min-w-0 flex-1 truncate text-xs']">{{ group.pinned ? element.group : "" }}</span>
                    <!-- 悬停时出现；连接中常驻并显示转圈。 -->
                    <div
                      class="absolute right-0 bottom-0 flex h-6 items-center transition-opacity duration-150"
                      :class="connecting.has(element.id) ? 'opacity-100' : 'pointer-events-none opacity-0 group-focus-within/card:pointer-events-auto group-focus-within/card:opacity-100 group-hover/card:pointer-events-auto group-hover/card:opacity-100 group-[.card-menu-open]/card:pointer-events-auto group-[.card-menu-open]/card:opacity-100'"
                    >
                      <UiButton size="tiny" type="primary" :loading="connecting.has(element.id)" tabindex="-1" @click.stop="app.openTab(element)" @dblclick.stop>
                        {{ t(connecting.has(element.id) ? "home.connecting" : "sidebar.connect") }}
                      </UiButton>
                    </div>
                  </div>

                  <div
                    :class="[toolGroup, 'absolute top-2 right-2 opacity-0 transition-opacity duration-150 group-focus-within/card:opacity-100 group-hover/card:opacity-100 group-[.card-menu-open]/card:opacity-100']"
                    @dblclick.stop
                  >
                    <IconButton v-if="canDrag" class="drag-handle cursor-grab touch-none active:cursor-grabbing" size="tiny" tabindex="-1" :title="t('home.dragHandle')">
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

          <footer :class="[dim, 'mt-11 flex flex-wrap justify-center gap-x-6 gap-y-2 text-xs']">
            <span v-for="key in HELP" :key="key" class="inline-flex items-center gap-2 whitespace-nowrap"><kbd :class="keycap">{{ t(`home.help.${key}Kbd`) }}</kbd>{{ t(`home.help.${key}`) }}</span>
          </footer>
        </template>
      </div>
    </div>

    <ProfileDialog v-model:show="showProfile" :profile="editing" />
    <IconPicker :profile="picking" @save="applyCustomize" @close="picking = null" />
  </div>
</template>
