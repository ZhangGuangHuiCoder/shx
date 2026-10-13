<script setup lang="ts">
// 命令助手：左侧是分组，右侧是分组里的命令，选中一条就填到终端的命令行上。
// 每条命令都可以修改、删除和收藏，也可以新建。只在 App.vue 中挂载一次，通过 `openAssistant` 打开。
import { Pencil, Play, Plus, RotateCcw, Search, SearchX, SquareTerminal, Star, Trash2, X } from "@lucide/vue";
import { NButton, NModal } from "naive-ui";
import { computed, nextTick, ref, watch } from "vue";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { shortcut } from "../../shared/contextMenu";
import { locale, t } from "../../shared/i18n";
import { confirm } from "../../shared/ui";
import { useApp } from "../../stores/app";
import { assistant } from "./commandAssistant";
import CommandEditor from "./CommandEditor.vue";
import CommandGroupNav, { type NavGroup } from "./CommandGroupNav.vue";
import CommandText from "./CommandText.vue";
import {
  commandGroups, defaultsChanged, favoriteGroup, FAVORITES, filterGroups, hasPlaceholder, searchWords, toggleFavorite, UNGROUPED,
  type CommandItem, type CustomCommand,
} from "./commands";
import { ALL, groupIcon } from "./groupIcons";

const app = useApp();
const query = ref("");
const selected = ref(ALL);
/** 键盘操作的那条命令在当前显示的命令中的位置。 */
const active = ref(0);
/** 正在新建或修改的命令；不为 null 时显示编辑页。 */
const draft = ref<CustomCommand | null>(null);
const list = ref<HTMLElement>();
const searchInput = ref<HTMLInputElement>();
const editor = ref<InstanceType<typeof CommandEditor>>();

const all = computed(() => commandGroups(app.commands, locale.value, t("cmd.ungrouped")));
const total = computed(() => all.value.reduce((n, group) => n + group.items.length, 0));
const words = computed(() => searchWords(query.value));
/** 含有匹配搜索的命令的所有分组。 */
const matching = computed(() => filterGroups(all.value, query.value));
/** 收藏的命令（其中匹配搜索的）；没有时为 null。 */
const favorites = computed(() => favoriteGroup(matching.value, app.settings.favoriteCommands, t("cmd.favorites")));
const starred = computed(() => new Set(app.settings.favoriteCommands));
/** 当前显示的分组；它没有可显示的命令时，退回到全部分组。 */
const current = computed(() => {
  if (selected.value === FAVORITES) return favorites.value ? FAVORITES : ALL;
  return matching.value.some((group) => group.key === selected.value) ? selected.value : ALL;
});
const groups = computed(() => {
  if (current.value === FAVORITES) return [favorites.value!];
  if (current.value !== ALL) return matching.value.filter((group) => group.key === current.value);
  // 没在搜索时把收藏的命令排在最前：打开命令助手，常用的就在眼前。搜索时不重复列出。
  return favorites.value && !words.value.length ? [favorites.value, ...matching.value] : matching.value;
});
const shown = computed(() => groups.value.flatMap((group) => group.items));
/** 每个分组的第一条命令在 `shown` 中的位置。 */
const offsets = computed(() => {
  let next = 0;
  return groups.value.map((group) => (next += group.items.length) - group.items.length);
});
const nav = computed<NavGroup[]>(() => [
  { key: ALL, title: t("cmd.all"), count: matching.value.reduce((n, group) => n + group.items.length, 0), icon: groupIcon(ALL) },
  ...(favorites.value ? [favorites.value] : []).map((group) => ({ key: group.key, title: group.title, count: group.items.length, icon: groupIcon(group.key) })),
  ...matching.value.map((group) => ({ key: group.key, title: group.title, count: group.items.length, icon: groupIcon(group.key) })),
]);
/** 当前项还有待填写的部分：只能填入，不能直接执行。 */
const fillOnly = computed(() => !!shown.value[active.value] && hasPlaceholder(shown.value[active.value].command));
/** 可以把命令放进去的分组的标题。 */
const titles = computed(() => all.value.filter((group) => group.key !== UNGROUPED).map((group) => group.title));
const changed = computed(() => defaultsChanged(app.commands));

watch(
  () => assistant.open,
  async (open) => {
    if (!open) return;
    query.value = "";
    selected.value = ALL;
    active.value = 0;
    draft.value = null;
    // 立刻接管键盘：否则对话框出现之前敲的键会落到终端里。
    await nextTick();
    searchInput.value?.focus();
  },
);
watch([query, current], () => {
  active.value = 0;
  list.value?.scrollTo({ top: 0 });
});

function close() {
  assistant.open = false;
}

/** 换一个分组显示；键盘仍留在搜索框里，可以接着输入和上下选择。 */
function pick(key: string) {
  selected.value = key;
  searchInput.value?.focus();
}

/** 把命令交给终端。还有待填写部分的命令，绝不原样执行。 */
function send(item: CommandItem, run: boolean) {
  const use = assistant.use;
  if (!use) return;
  close();
  use(item.command, run && !hasPlaceholder(item.command));
}

/** 选中一条命令：有终端可填就填进去，否则打开它来修改。 */
function choose(item: CommandItem | undefined, run = false) {
  if (!item) return;
  if (assistant.use) send(item, run);
  else edit(item);
}

function edit(item: CommandItem) {
  draft.value = { id: item.id, name: item.name, command: item.command, group: item.group };
}

/** 新命令建在当前显示的分组里；`command` 是预先填好的内容。 */
function add(command = "") {
  const real = current.value !== ALL && current.value !== UNGROUPED && current.value !== FAVORITES;
  const group = groups.value.length === 1 && real ? groups.value[0].title : "";
  draft.value = { id: "", name: "", command, group };
}

/** 让某条命令出现在列表里并成为当前项。 */
async function reveal(id: string) {
  const inView = () => shown.value.findIndex((item) => item.id === id);
  if (inView() < 0) {
    query.value = "";
    selected.value = all.value.find((group) => group.items.some((item) => item.id === id))?.key ?? ALL;
  }
  // 等上面的变化引起的“回到第一条”先发生。
  await nextTick();
  active.value = Math.max(inView(), 0);
  await nextTick();
  list.value?.querySelector(".item.active")?.scrollIntoView({ block: "nearest" });
}

function save(next: CustomCommand) {
  const before = draft.value!;
  // 什么都没改就不保存：默认命令保存后就不再跟随界面语言。
  const same = before.id && next.name === before.name && next.command === before.command && next.group === before.group;
  const saved = same ? before : app.saveCommand(next);
  draft.value = null;
  void reveal(saved.id);
}

async function remove(item: Pick<CommandItem, "id" | "name">) {
  if (!(await confirm(t("cmd.deleteTitle"), t("cmd.deleteBody", { name: item.name }), { danger: true, ok: t("common.delete") }))) return;
  app.deleteCommand(item.id);
  draft.value = null;
  await nextTick();
  active.value = Math.max(Math.min(active.value, shown.value.length - 1), 0);
  searchInput.value?.focus();
}

/** 收藏或取消收藏。收藏的命令在全部命令里排在最前，列表会跟着增减一行。 */
async function favorite(item: CommandItem, event: MouseEvent) {
  const row = (event.currentTarget as HTMLElement).closest<HTMLElement>(".item")!;
  const top = row.getBoundingClientRect().top;
  app.settings.favoriteCommands = toggleFavorite(app.settings.favoriteCommands, item.id);
  await nextTick();
  if (row.isConnected) {
    // 点的那一行还在：让它留在指针下面，并且仍是当前项。
    list.value!.scrollTop += row.getBoundingClientRect().top - top;
    active.value = [...list.value!.querySelectorAll(".item")].indexOf(row);
  } else active.value = Math.max(Math.min(active.value, shown.value.length - 1), 0);
}

async function restore() {
  if (await confirm(t("cmd.restoreTitle"), t("cmd.restoreBody"), { icon: RotateCcw, ok: t("cmd.restore") })) app.restoreCommands();
  searchInput.value?.focus();
}

async function move(step: number) {
  if (!shown.value.length) return;
  active.value = (active.value + step + shown.value.length) % shown.value.length;
  await nextTick();
  list.value?.querySelector(".item.active")?.scrollIntoView({ block: "nearest" });
}

function onKey(e: KeyboardEvent) {
  if (e.isComposing) return;
  const mod = e.metaKey || e.ctrlKey;
  if (draft.value) {
    if (e.key === "Escape") draft.value = null;
    else if (e.key === "Enter" && mod) editor.value?.submit();
    else return;
  } else if (e.key === "Escape") close();
  else if (e.key === "ArrowDown") void move(1);
  else if (e.key === "ArrowUp") void move(-1);
  // 焦点在按钮上时，回车是按下那个按钮。
  else if (e.key === "Enter" && !(e.target as HTMLElement).closest("button")) choose(shown.value[active.value], mod);
  else return;
  e.preventDefault();
}
</script>

<template>
  <NModal :show="assistant.open" :auto-focus="false" :close-on-esc="false" @update:show="(open: boolean) => !open && close()">
    <div class="assistant" @keydown="onKey">
      <Transition name="page" mode="out-in" @after-enter="draft || searchInput?.focus()">
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

        <div v-else class="page">
          <DialogHeader :icon="SquareTerminal" :title="t('cmd.assistant')" :detail="t('cmd.summary', { count: total, groups: all.length })" class="head">
            <label class="search">
              <Search :size="16" />
              <input ref="searchInput" v-model="query" type="text" autofocus :placeholder="t('cmd.search')" />
              <IconButton v-if="query" size="tiny" :focusable="false" @click="query = ''"><X :size="13" /></IconButton>
            </label>
            <NButton type="primary" secondary @click="add()">
              <template #icon><Plus :size="15" /></template>
              {{ t("cmd.new") }}
            </NButton>
            <IconButton :title="t('common.close')" @click="close"><X :size="16" /></IconButton>
          </DialogHeader>

          <div class="panes">
            <CommandGroupNav :model-value="current" :groups="nav" @update:model-value="pick" />

            <div ref="list" class="list" :class="{ grouped: current === ALL }">
              <div v-if="!shown.length" class="none">
                <div class="none-tile"><SearchX :size="22" :stroke-width="1.7" /></div>
                <div class="none-title">{{ t(total ? "cmd.noMatch" : "cmd.empty") }}</div>
                <div v-if="total" class="dim">{{ t("cmd.noMatchHint") }}</div>
                <NButton class="none-new" type="primary" secondary @click="add(query.trim())">
                  <template #icon><Plus :size="15" /></template>
                  {{ query.trim() ? t("cmd.newFrom", { text: query.trim() }) : t("cmd.new") }}
                </NButton>
              </div>

              <section v-for="(group, g) in groups" :key="group.key">
                <h4 v-if="current === ALL">
                  <component :is="groupIcon(group.key)" :size="13" :stroke-width="2" />
                  {{ group.title }}
                  <span class="tally">{{ group.items.length }}</span>
                </h4>
                <!-- 列表有几百行。`v-memo`：指针每移到另一行，只重画变了的那两行；
                     各种按钮只画在当前行上，其余的行没有它们。 -->
                <div
                  v-for="(item, i) in group.items"
                  :key="item.id"
                  v-memo="[item, offsets[g] + i, offsets[g] + i === active, starred.has(item.id), words, assistant.use]"
                  class="item"
                  :class="{ active: offsets[g] + i === active }"
                  :title="item.command"
                  @mousemove="active = offsets[g] + i"
                  @click="choose(item)"
                >
                  <div class="glyph"><component :is="groupIcon(group.key)" :size="15" :stroke-width="1.9" /></div>
                  <CommandText :name="item.name" :command="item.command" :words="words" />
                  <!-- 宽度固定：按钮出现和消失时，左边的文字不跟着伸缩。 -->
                  <div class="actions">
                    <template v-if="offsets[g] + i === active">
                      <IconButton
                        v-if="assistant.use && !hasPlaceholder(item.command)"
                        type="primary"
                        :focusable="false"
                        :title="t('cmd.run')"
                        @click.stop="send(item, true)"
                      >
                        <Play :size="14" />
                      </IconButton>
                      <IconButton :focusable="false" :title="t('common.edit')" @click.stop="edit(item)"><Pencil :size="14" /></IconButton>
                      <IconButton type="error" :focusable="false" :title="t('common.delete')" @click.stop="remove(item)"><Trash2 :size="14" /></IconButton>
                    </template>
                    <!-- 放在最右边、各行对齐；收藏了的一直显示，其余的只在当前行上显示。 -->
                    <IconButton
                      v-if="offsets[g] + i === active || starred.has(item.id)"
                      :type="starred.has(item.id) ? 'primary' : 'default'"
                      :focusable="false"
                      :title="t(starred.has(item.id) ? 'cmd.unfavorite' : 'cmd.favorite')"
                      @click.stop="favorite(item, $event)"
                    >
                      <Star :size="14" :fill="starred.has(item.id) ? 'currentColor' : 'none'" />
                    </IconButton>
                  </div>
                </div>
              </section>
            </div>
          </div>

          <footer>
            <span class="keys"><kbd>↑</kbd><kbd>↓</kbd> {{ t("cmd.keyMove") }}</span>
            <template v-if="assistant.use">
              <span class="keys"><kbd>↵</kbd> {{ t("cmd.keyInsert") }}</span>
              <span v-if="fillOnly" class="keys">{{ t("cmd.fillOnly") }}</span>
              <span v-else class="keys"><kbd>{{ shortcut.mod("↵") }}</kbd> {{ t("cmd.keyRun") }}</span>
            </template>
            <span v-else class="keys"><kbd>↵</kbd> {{ t("cmd.keyEdit") }}</span>
            <span class="spacer" />
            <NButton v-if="changed" quaternary type="tertiary" size="tiny" @click="restore">
              <template #icon><RotateCcw :size="13" /></template>
              {{ t("cmd.restore") }}
            </NButton>
          </footer>
        </div>
      </Transition>
    </div>
  </NModal>
</template>

<style scoped>
.assistant {
  display: flex;
  flex-direction: column;
  width: 860px;
  max-width: calc(100vw - 48px);
  height: min(640px, calc(100vh - 72px));
  border: 1px solid var(--border);
  border-radius: 16px;
  background: var(--bg-raised);
  box-shadow: 0 28px 70px -16px var(--shadow);
  color: var(--text);
  overflow: hidden;
}

.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.page-enter-active,
.page-leave-active {
  transition:
    opacity 0.12s ease,
    transform 0.12s ease;
}

.page-enter-from {
  opacity: 0;
  transform: translateX(10px);
}

.page-leave-to {
  opacity: 0;
  transform: translateX(-10px);
}

.head {
  padding: 14px 14px 14px 18px;
}

/* 标题只占它需要的宽度，余下的都给搜索框。 */
.head :deep(.titles) {
  flex: none;
  margin-right: 8px;
}

/* ---- 搜索框 ---- */
.search {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 9px;
  min-width: 0;
  height: 36px;
  padding: 0 9px 0 12px;
  border: 1px solid transparent;
  border-radius: 10px;
  background: var(--fill);
  color: var(--text-dim);
  cursor: text;
  transition:
    border-color 0.15s,
    box-shadow 0.15s,
    background-color 0.15s;
}

.search:focus-within {
  border-color: var(--accent-border);
  background: var(--bg-raised);
  box-shadow: 0 0 0 3px var(--accent-soft);
}

.search:focus-within > svg {
  color: var(--accent);
}

.search input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  outline: 0;
  background: none;
  color: var(--text);
  font: inherit;
}

.search input::placeholder {
  color: var(--text-dim);
  opacity: 0.8;
}

/* ---- 分组和命令列表 ---- */
.panes {
  display: flex;
  flex: 1;
  min-height: 0;
  border-top: 1px solid var(--border);
}

.list {
  flex: 1;
  min-width: 0;
  padding: 8px 8px 10px;
  overflow-y: auto;
}

/* 分组标题吸顶：键盘移到被它挡住的命令时，要多滚出标题的高度。 */
.list.grouped {
  padding-top: 0;
  scroll-padding-top: 34px;
}

h4 {
  position: sticky;
  top: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  padding: 11px 10px 7px;
  background: var(--bg-raised);
  color: var(--text-dim);
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: 0.3px;
}

/* 标题右侧的细线一直延伸到边缘。 */
h4::after {
  content: "";
  flex: 1;
  height: 1px;
  margin-left: 4px;
  background: var(--border);
}

.tally {
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  opacity: 0.75;
}

.item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 8px 8px 12px;
  border-radius: 10px;
  cursor: pointer;
}

.glyph {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  flex-shrink: 0;
  border-radius: 8px;
  background: var(--fill);
  color: var(--text-dim);
  transition:
    background-color 0.12s,
    color 0.12s;
}

.item.active {
  background: var(--accent-soft);
}

.item.active .glyph {
  background: var(--accent);
  color: #fff;
}

.actions {
  display: flex;
  flex-shrink: 0;
  justify-content: flex-end;
  gap: 2px;
  /* 四个按钮的宽度。 */
  width: 118px;
}

/* ---- 没有命令可显示 ---- */
.none {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 72px 24px 0;
  text-align: center;
}

.none-tile {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  margin-bottom: 6px;
  border-radius: 14px;
  background: var(--fill);
  color: var(--text-dim);
}

.none-title {
  font-size: 14px;
  font-weight: 600;
}

.none-new {
  max-width: 100%;
  margin-top: 12px;
  overflow: hidden;
}

/* ---- 底栏 ---- */
footer {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 40px;
  flex-shrink: 0;
  padding: 0 10px 0 18px;
  border-top: 1px solid var(--border);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-raised));
  color: var(--text-dim);
  font-size: 12px;
}

.keys {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.spacer {
  flex: 1;
}

</style>
