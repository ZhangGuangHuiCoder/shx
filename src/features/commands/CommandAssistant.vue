<script setup lang="ts">
// 命令助手：虚拟列表（Listbox + ListboxVirtualizer），分组摊平为“组标题 + 命令”行，吸顶标题用浮层模拟。
import { Pencil, Play, Plus, RotateCcw, Search, SearchX, SquareTerminal, Star, Trash2, X } from "@lucide/vue";
import { ListboxContent, ListboxFilter, ListboxItem, ListboxRoot, ListboxVirtualizer } from "reka-ui";
import { computed, nextTick, ref, watch } from "vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { shortcut } from "@/shared/contextMenu";
import { locale, t } from "@/shared/i18n";
import { confirm } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { assistant } from "@/features/commands/commandAssistant";
import CommandEditor from "@/features/commands/CommandEditor.vue";
import CommandGroupLabel from "@/features/commands/CommandGroupLabel.vue";
import CommandGroupNav, { type NavGroup } from "@/features/commands/CommandGroupNav.vue";
import CommandText from "@/features/commands/CommandText.vue";
import {
  commandGroups, defaultsChanged, favoriteGroup, FAVORITES, filterGroups, hasPlaceholder, searchWords, toggleFavorite, UNGROUPED,
  type CommandGroup, type CommandItem, type CustomCommand,
} from "@/features/commands/commands";
import { ALL, groupIcon } from "@/features/commands/groupIcons";
import { dim, fg, keycap, line, well } from "@/shared/ui-kit/styles";

// 须与模板中的行高一致，虚拟列表据此排版。
const ROW = 52;
const HEADER = 40;
const PAD = 8;

/** 虚拟行。同一命令可能同时在收藏区和普通组，key 需带组。 */
type Row = { key: string; group: CommandGroup; top: number } & ({ item: CommandItem } | { item?: undefined });
const rowKey = (group: CommandGroup, item: CommandItem) => `${group.key}\u0000${item.id}`;

const app = useApp();
const query = ref("");
const selected = ref(ALL);
const active = ref<string | null>(null);
const draft = ref<CustomCommand | null>(null);
/** 关闭编辑器后要定位到的命令。 */
let revealId: string | null = null;
const content = ref<InstanceType<typeof ListboxContent>>();
const filter = ref<InstanceType<typeof ListboxFilter>>();
const editor = ref<InstanceType<typeof CommandEditor>>();
const scrollTop = ref(0);

const all = computed(() => commandGroups(app.commands, locale.value, t("cmd.ungrouped")));
const total = computed(() => all.value.reduce((n, group) => n + group.items.length, 0));
const words = computed(() => searchWords(query.value));
const matching = computed(() => filterGroups(all.value, query.value));
const favorites = computed(() => favoriteGroup(matching.value, app.settings.favoriteCommands, t("cmd.favorites")));
const starred = computed(() => new Set(app.settings.favoriteCommands));
const current = computed(() => {
  if (selected.value === FAVORITES) return favorites.value ? FAVORITES : ALL;
  return matching.value.some((group) => group.key === selected.value) ? selected.value : ALL;
});
const groups = computed(() => {
  if (current.value === FAVORITES) return [favorites.value!];
  if (current.value !== ALL) return matching.value.filter((group) => group.key === current.value);
  return favorites.value && !words.value.length ? [favorites.value, ...matching.value] : matching.value;
});
const headed = computed(() => current.value === ALL);
const rows = computed(() => {
  const out: Row[] = [];
  let top = headed.value ? 0 : PAD;
  for (const group of groups.value) {
    if (headed.value) out.push({ key: `h:${group.key}`, group, top }), (top += HEADER);
    for (const item of group.items) out.push({ key: rowKey(group, item), group, item, top }), (top += ROW);
  }
  return out;
});
/** 第一条命令的行号。 */
const first = computed(() => rows.value.findIndex((row) => row.item));
const activeItem = computed(() => rows.value.find((row) => row.key === active.value)?.item);
const headRows = computed(() => (headed.value ? rows.value.filter((row) => !row.item) : []));
/** 吸顶标题，被下一组标题推走。 */
const stuck = computed(() => {
  const heads = headRows.value;
  const at = heads.filter((row) => row.top <= scrollTop.value).length - 1;
  const next = heads[at + 1];
  return at < 0 ? null : { group: heads[at].group, offset: next ? Math.min(0, next.top - scrollTop.value - HEADER) : 0 };
});
const navItem = (group: CommandGroup): NavGroup => ({ key: group.key, title: group.title, count: group.items.length, icon: groupIcon(group.key) });
const nav = computed<NavGroup[]>(() => [
  { key: ALL, title: t("cmd.all"), count: matching.value.reduce((n, group) => n + group.items.length, 0), icon: groupIcon(ALL) },
  ...(favorites.value ? [navItem(favorites.value)] : []),
  ...matching.value.map(navItem),
]);
const fillOnly = computed(() => !!activeItem.value && hasPlaceholder(activeItem.value.command));
const titles = computed(() => all.value.filter((group) => group.key !== UNGROUPED).map((group) => group.title));
const changed = computed(() => defaultsChanged(app.commands));

const scroller = () => content.value?.$el as HTMLElement | undefined;
const focusSearch = () => (filter.value?.$el as HTMLInputElement | undefined)?.focus();

/** 滚动到指定行并高亮。Listbox 无按值高亮虚拟行的接口，借助该行的 pointermove 实现。 */
async function highlightRow(index: number, to?: number) {
  const el = scroller();
  const row = rows.value[index];
  if (!el || !row) return;
  to ??= Math.min(Math.max(el.scrollTop, row.top + ROW - el.clientHeight), row.top - (headed.value ? HEADER : PAD));
  el.scrollTop = Math.max(0, to);
  await new Promise(requestAnimationFrame);
  await nextTick();
  el.querySelector(`[data-index="${index}"]`)?.dispatchEvent(new PointerEvent("pointermove"));
}

watch(() => assistant.open, (open) => {
  if (!open) return;
  query.value = "";
  selected.value = ALL;
  active.value = null;
  draft.value = null;
  scrollTop.value = 0;
});
// 换组或搜索后高亮首条；不用 highlightFirstItem：虚拟行复用节点时会取错。
watch([query, current], () => nextTick(() => highlightRow(first.value)));

function close() {
  assistant.open = false;
}
function pick(key: string) {
  selected.value = key;
  focusSearch();
}
function send(item: CommandItem, run: boolean) {
  const use = assistant.use;
  if (!use) return;
  close();
  use(item.command, run && !hasPlaceholder(item.command));
}
function choose(item: CommandItem | undefined, run = false) {
  if (!item) return;
  if (assistant.use) send(item, run);
  else edit(item);
}
/** 阻止 Listbox 记录选中值，否则会把该行滚到顶部。 */
function onSelect(event: Event, item: CommandItem) {
  event.preventDefault();
  choose(item);
}
function edit(item: CommandItem) {
  draft.value = { id: item.id, name: item.name, command: item.command, group: item.group };
}
function add(command = "") {
  const real = current.value !== ALL && current.value !== UNGROUPED && current.value !== FAVORITES;
  draft.value = { id: "", name: "", command, group: groups.value.length === 1 && real ? groups.value[0].title : "" };
}

function save(next: CustomCommand) {
  const before = draft.value!;
  const same = before.id && next.name === before.name && next.command === before.command && next.group === before.group;
  revealId = (same ? before : app.saveCommand(next)).id;
  if (!rows.value.some((row) => row.item?.id === revealId)) {
    query.value = "";
    selected.value = all.value.find((group) => group.items.some((item) => item.id === revealId))?.key ?? ALL;
  }
  draft.value = null;
}
function onListEnter() {
  const index = rows.value.findIndex((row) => row.item?.id === revealId);
  revealId = null;
  if (index >= 0) void highlightRow(index);
}
async function remove(item: Pick<CommandItem, "id" | "name">) {
  if (!(await confirm(t("cmd.deleteTitle"), t("cmd.deleteBody", { name: item.name }), { danger: true, ok: t("common.delete") }))) return;
  const was = rows.value.findIndex((row) => row.key === active.value);
  app.deleteCommand(item.id);
  draft.value = null;
  await nextTick();
  focusSearch();
  // 保持原位；若该处变成组标题则取下一条。
  const index = Math.min(Math.max(was, 0), rows.value.length - 1);
  await highlightRow(rows.value[index]?.item ? index : index + 1);
}
/** 收藏增减行时补偿滚动，使被点的行留在原处。 */
async function favorite(row: Row, event: MouseEvent) {
  const el = scroller()!;
  const y = (event.currentTarget as HTMLElement).closest("[role=option]")!.getBoundingClientRect().top - el.getBoundingClientRect().top;
  app.settings.favoriteCommands = toggleFavorite(app.settings.favoriteCommands, row.item!.id);
  await nextTick();
  const index = rows.value.findIndex((r) => r.key === row.key);
  if (index >= 0) await highlightRow(index, rows.value[index].top - y);
}
async function restore() {
  if (await confirm(t("cmd.restoreTitle"), t("cmd.restoreBody"), { icon: RotateCcw, ok: t("cmd.restore") })) app.restoreCommands();
  focusSearch();
}
/** 上下键与回车由 Listbox 处理，这里只处理其余按键。 */
function onKey(e: KeyboardEvent) {
  if (e.isComposing) return;
  const mod = e.metaKey || e.ctrlKey;
  if (draft.value) {
    if (e.key === "Escape") draft.value = null;
    else if (e.key === "Enter" && mod) editor.value?.submit();
    else return;
  } else if (e.key === "Escape") close();
  // 自行处理首尾回绕：首行为组标题或虚拟行复用节点时 Listbox 会出错。
  else if (e.key === "ArrowUp" && active.value === rows.value[first.value]?.key) void highlightRow(rows.value.length - 1);
  else if (e.key === "ArrowDown" && active.value === rows.value[rows.value.length - 1]?.key) void highlightRow(first.value);
  else if (e.key === "Enter" && mod && !(e.target as HTMLElement).closest("button")) choose(activeItem.value, true);
  else return;
  e.preventDefault();
}
</script>

<template>
  <UiModal raw :show="assistant.open" :auto-focus="false" :close-on-esc="false" @update:show="(open: boolean) => !open && close()">
    <!-- 面积大，不用背景模糊，否则 WebKit 上打开动画会卡顿。 -->
    <div
      :class="[fg, 'flex h-160 max-h-full min-h-0 w-215 max-w-full shrink flex-col overflow-hidden rounded-3xl border border-white/70 bg-white/95 text-sm shadow-2xl shadow-zinc-900/20 inset-shadow-xs inset-shadow-white dark:border-white/10 dark:bg-zinc-800/95 dark:shadow-black/50 dark:inset-shadow-white/10']"
      @keydown="onKey"
    >
      <Transition
        mode="out-in"
        enter-active-class="transition duration-150 ease-in-out"
        leave-active-class="transition duration-150 ease-in-out"
        enter-from-class="opacity-0 translate-x-3"
        leave-to-class="opacity-0 -translate-x-3"
        @after-enter="draft || onListEnter()">
        <CommandEditor
          v-if="draft"
          ref="editor"
          :key="draft.id"
          :command="draft"
          :groups="titles"
          @save="save"
          @back="draft = null"
          @close="close"
          @remove="remove(draft)"
        />

        <ListboxRoot
          v-else
          by="key"
          highlight-on-hover
          loop
          class="flex flex-1 flex-col min-h-0"
          @highlight="(e) => (active = (e?.value as Row | undefined)?.key ?? null)"
        >
          <DialogHeader :icon="SquareTerminal" :title="t('cmd.assistant')" :detail="t('cmd.summary', { count: total, groups: all.length })" class="py-4 pr-4 pl-5">
            <label
              :class="[dim, 'flex h-9 min-w-0 flex-1 cursor-text items-center gap-2 rounded-xl border border-transparent bg-zinc-900/5 pr-2 pl-3 transition duration-150 focus-within:border-blue-500/50 focus-within:bg-white/80 focus-within:text-blue-500 focus-within:ring-3 focus-within:ring-blue-500/15 dark:bg-white/5 dark:focus-within:bg-white/10']"
            >
              <Search :size="16" />
              <ListboxFilter
                ref="filter"
                v-model="query"
                auto-focus
                :placeholder="t('cmd.search')"
                :class="[fg, 'h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-zinc-400 dark:placeholder:text-zinc-500']"
              />
              <IconButton v-if="query" size="tiny" tabindex="-1" @click="query = ''"><X :size="13" /></IconButton>
            </label>
            <UiButton type="primary" @click="add()">
              <template #icon><Plus :size="15" /></template>
              {{ t("cmd.new") }}
            </UiButton>
            <IconButton :title="t('common.close')" @click="close"><X :size="16" /></IconButton>
          </DialogHeader>

          <div :class="[line, 'flex min-h-0 flex-1 border-t']">
            <CommandGroupNav :model-value="current" :groups="nav" @update:model-value="pick" />

            <!-- 虚拟列表的滚动容器；scroll-pt-10 让键盘滚动避开吸顶标题。 -->
            <ListboxContent
              ref="content"
              class="relative min-w-0 flex-1 overflow-y-auto px-2 pb-3 outline-none [overflow-anchor:none]"
              :class="headed ? 'scroll-pt-10' : 'pt-2'"
              @scroll.passive="scrollTop = ($event.target as HTMLElement).scrollTop"
            >
              <div v-if="!rows.length" class="flex flex-col items-center gap-2 px-6 pt-18 pb-0 text-center">
                <SearchX :size="22" :stroke-width="1.7" :class="[dim, 'mb-1']" />
                <div class="text-sm font-semibold">{{ t(total ? "cmd.noMatch" : "cmd.empty") }}</div>
                <div v-if="total" :class="dim">{{ t("cmd.noMatchHint") }}</div>
                <UiButton class="mt-3 max-w-full overflow-hidden" type="primary" @click="add(query.trim())">
                  <template #icon><Plus :size="15" /></template>
                  {{ query.trim() ? t("cmd.newFrom", { text: query.trim() }) : t("cmd.new") }}
                </UiButton>
              </div>

              <!-- 零高度 sticky 层，覆盖在同名组标题上。 -->
              <div v-if="stuck" class="sticky top-0 z-10 h-0">
                <CommandGroupLabel :group="stuck.group" class="bg-white/95 dark:bg-zinc-800/95" :style="{ transform: `translateY(${stuck.offset}px)` }" />
              </div>

              <ListboxVirtualizer
                v-slot="{ option }"
                :options="rows"
                :estimate-size="(i: number) => (rows[i].item ? ROW : HEADER)"
                :text-content="(row: Row) => row.item?.name.toLowerCase() ?? ''"
                :overscan="6"
              >
                <CommandGroupLabel v-if="!option.item" :group="option.group" />
                <ListboxItem
                  v-else
                  :value="option"
                  class="flex h-[52px] w-full cursor-pointer items-center gap-3 overflow-hidden rounded-lg py-1.5 pr-2 pl-3 outline-none data-highlighted:bg-blue-500/10 dark:data-highlighted:bg-blue-400/15"
                  :title="option.item.command"
                  @select="onSelect($event, option.item)"
                >
                  <CommandText :name="option.item.name" :command="option.item.command" :words="words" />
                  <div class="flex w-30 shrink-0 justify-end gap-1">
                    <template v-if="option.key === active">
                      <IconButton
                        v-if="assistant.use && !hasPlaceholder(option.item.command)"
                        type="primary"
                        tabindex="-1"
                        :title="t('cmd.run')"
                        @click.stop="send(option.item, true)"
                      >
                        <Play :size="14" />
                      </IconButton>
                      <IconButton tabindex="-1" :title="t('common.edit')" @click.stop="edit(option.item)"><Pencil :size="14" /></IconButton>
                      <IconButton type="error" tabindex="-1" :title="t('common.delete')" @click.stop="remove(option.item)"><Trash2 :size="14" /></IconButton>
                    </template>
                    <IconButton
                      v-if="option.key === active || starred.has(option.item.id)"
                      :type="starred.has(option.item.id) ? 'primary' : 'default'"
                      tabindex="-1"
                      :title="t(starred.has(option.item.id) ? 'cmd.unfavorite' : 'cmd.favorite')"
                      @click.stop="favorite(option, $event)"
                    >
                      <Star :size="14" :fill="starred.has(option.item.id) ? 'currentColor' : 'none'" />
                    </IconButton>
                  </div>
                </ListboxItem>
              </ListboxVirtualizer>
            </ListboxContent>
          </div>

          <footer :class="[line, well, dim, 'flex h-10 shrink-0 items-center gap-4 border-t *:inline-flex *:items-center *:gap-1 pr-3 pl-5 text-xs']">
            <span><kbd :class="keycap">↑</kbd><kbd :class="keycap">↓</kbd> {{ t("cmd.keyMove") }}</span>
            <template v-if="assistant.use">
              <span><kbd :class="keycap">↵</kbd> {{ t("cmd.keyInsert") }}</span>
              <span v-if="fillOnly">{{ t("cmd.fillOnly") }}</span>
              <span v-else><kbd :class="keycap">{{ shortcut.mod("↵") }}</kbd> {{ t("cmd.keyRun") }}</span>
            </template>
            <span v-else><kbd :class="keycap">↵</kbd> {{ t("cmd.keyEdit") }}</span>
            <span class="flex-1" />
            <UiButton v-if="changed" ghost size="tiny" @click="restore">
              <template #icon><RotateCcw :size="13" /></template>
              {{ t("cmd.restore") }}
            </UiButton>
          </footer>
        </ListboxRoot>
      </Transition>
    </div>
  </UiModal>
</template>