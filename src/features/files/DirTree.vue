<script setup lang="ts">
// 文件列表旁的目录树。只有屏幕上可见的行在 DOM 里，树本身是普通数据（见 dirTree.ts）：
// 一个文件夹有上千个子文件夹时，代价只是遍历一遍，而不是每个都建一个响应式对象。
import { ChevronRight, ChevronsDownUp, LoaderCircle, Locate } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { errorMessage, type FileEntry } from "../../shared/api";
import FileIcon from "../../shared/components/FileIcon.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { t } from "../../shared/i18n";
import { applyListing, ensurePath, expandable, findNode, pathChain, rootNode, visibleRows, type DirNode } from "./dirTree";

const props = defineProps<{
  /** 文件列表正在显示的文件夹。 */
  current: string;
  /** 文件列表正要前往的文件夹（如果有）。 */
  pending: string | null;
  showHidden: boolean;
  /** 列出服务器上某个文件夹的内容。 */
  load: (path: string) => Promise<FileEntry[]>;
}>();
const emit = defineEmits<{ open: [path: string]; menu: [event: MouseEvent, node: DirNode] }>();

const ROW_HEIGHT = 24;
const INDENT = 14;

// ---- 数据模型 --------------------------------------------------------------

const root = rootNode();
/** 普通数据的树每次变化后递增，因为 Vue 自己察觉不到这些变化。 */
const version = ref(0);
const expanded = shallowRef<ReadonlySet<string>>(new Set(["/"]));
const loading = shallowRef<ReadonlySet<string>>(new Set());
/** 键盘命令所作用的行。 */
const cursor = ref("/");

const rows = computed(() => {
  void version.value;
  return visibleRows(root, expanded.value, props.showHidden, props.current);
});

/** 进行中的列表请求，保证同一个文件夹不会同时请求两次。 */
const jobs = new Map<string, Promise<void>>();

function loadNode(node: DirNode, force = false): Promise<void> {
  if (node.loaded && !force) return Promise.resolve();
  const running = jobs.get(node.path);
  if (running) return running;
  const mark = (on: boolean) => {
    const next = new Set(loading.value);
    if (on) next.add(node.path);
    else next.delete(node.path);
    loading.value = next;
  };
  mark(true);
  const job = props
    .load(node.path)
    .then(
      (entries) => applyListing(node, entries),
      (e) => {
        // 不自动重试：否则读不了的文件夹会被一遍遍地请求。
        node.loaded = true;
        node.error = errorMessage(e);
      },
    )
    .finally(() => {
      jobs.delete(node.path);
      mark(false);
      version.value++;
    });
  jobs.set(node.path, job);
  return job;
}

function setExpanded(paths: string[], open: boolean) {
  const next = new Set(expanded.value);
  for (const path of paths) {
    if (open) next.add(path);
    else next.delete(path);
  }
  expanded.value = next;
}

function toggle(node: DirNode, open = !expanded.value.has(node.path)) {
  setExpanded([node.path], open);
  if (open) void loadNode(node);
}

/** 接收文件列表本来就要获取的列表，省去目录树自己再请求一次。 */
function sync(path: string, entries: FileEntry[]) {
  applyListing(ensurePath(root, path), entries);
  version.value++;
}

/** 如果目录树正显示着某个文件夹的内容，就重新列出它。 */
function reload(path: string) {
  const node = findNode(root, path);
  if (node?.loaded) void loadNode(node, true);
}

/** 把目录树展开到 `path`，并让那一行滚动到可见位置。 */
async function reveal(path: string) {
  if (!path) return;
  const chain = pathChain(path);
  // 这一行可以立即显示；同级的其他行等上级目录列出后再补上。
  ensurePath(root, path);
  setExpanded(chain, true);
  cursor.value = path;
  version.value++;
  await nextTick();
  scrollToPath(path, true);

  const unlisted = chain.map((step) => findNode(root, step)).filter((node): node is DirNode => !!node && !node.loaded);
  if (!unlisted.length) return;
  await Promise.all(unlisted.map((node) => loadNode(node)));
  if (props.current !== path) return;
  await nextTick();
  scrollToPath(path, true);
}

/** 重试所有列出失败的文件夹；通常是在重连之后。 */
function retry() {
  const walk = (node: DirNode) => {
    if (node.error) {
      node.loaded = false;
      delete node.error;
      if (expanded.value.has(node.path)) void loadNode(node);
    }
    node.children.forEach(walk);
  };
  walk(root);
  version.value++;
}

function collapseAll() {
  expanded.value = new Set(["/"]);
  cursor.value = "/";
  scroller.value?.scrollTo({ top: 0 });
}

watch(() => props.current, reveal, { immediate: true });

// ---- 可视区域 --------------------------------------------------------------

const scroller = ref<HTMLElement>();
const scrollTop = ref(0);
const viewHeight = ref(0);
let observer: ResizeObserver | undefined;

const windowStart = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - 6));
const visible = computed(() =>
  rows.value.slice(windowStart.value, windowStart.value + Math.ceil(viewHeight.value / ROW_HEIGHT) + 12),
);

/** 把某一行滚动到可见位置；已在屏幕上的行保持不动。 */
function scrollToRow(index: number, center = false) {
  const el = scroller.value;
  if (!el || index < 0 || !el.clientHeight) return;
  const top = index * ROW_HEIGHT;
  if (top >= el.scrollTop && top + ROW_HEIGHT <= el.scrollTop + el.clientHeight) return;
  if (center) el.scrollTop = top - (el.clientHeight - ROW_HEIGHT) / 2;
  else el.scrollTop = top < el.scrollTop ? top : top + ROW_HEIGHT - el.clientHeight;
}

function scrollToPath(path: string, center = false) {
  scrollToRow(rows.value.findIndex((row) => row.node.path === path), center);
}

onMounted(() => {
  viewHeight.value = scroller.value!.clientHeight;
  observer = new ResizeObserver(([box]) => {
    const shown = !viewHeight.value && box.contentRect.height > 0;
    viewHeight.value = box.contentRect.height;
    // 从隐藏恢复显示：期间当前文件夹可能已经变了。
    if (shown) void nextTick(() => scrollToPath(props.current, true));
  });
  observer.observe(scroller.value!);
});

onBeforeUnmount(() => observer?.disconnect());

// ---- 鼠标和键盘 ------------------------------------------------------------

function clickRow(node: DirNode) {
  cursor.value = node.path;
  if (node.path === props.current) toggle(node);
  else if (node.path !== props.pending) emit("open", node.path);
}

function openMenu(event: MouseEvent, node: DirNode) {
  cursor.value = node.path;
  emit("menu", event, node);
}

function onKey(e: KeyboardEvent) {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const list = rows.value;
  const at = Math.max(0, list.findIndex((row) => row.node.path === cursor.value));
  const { node, depth } = list[at];
  const open = expanded.value.has(node.path) && expandable(node, props.showHidden);
  const move = (index: number) => {
    const target = Math.min(list.length - 1, Math.max(0, index));
    cursor.value = list[target].node.path;
    scrollToRow(target);
  };

  if (e.key === "ArrowDown") move(at + 1);
  else if (e.key === "ArrowUp") move(at - 1);
  else if (e.key === "Home") move(0);
  else if (e.key === "End") move(list.length - 1);
  else if (e.key === "ArrowRight") {
    // 和所有文件树一样：先展开文件夹，再进入其中。
    if (open) move(at + 1);
    else if (expandable(node, props.showHidden)) toggle(node, true);
  } else if (e.key === "ArrowLeft") {
    if (open) toggle(node, false);
    else if (depth > 0) {
      let parent = at - 1;
      while (list[parent].depth >= depth) parent--;
      move(parent);
    }
  } else if (e.key === "Enter" || e.key === " ") emit("open", node.path);
  else return;
  e.preventDefault();
}

defineExpose({ sync, reload, retry, isExpanded: (path: string) => expanded.value.has(path), toggle });
</script>

<template>
  <div class="dir-tree">
    <div class="head">
      <span class="ellipsis title">{{ t("fm.tree") }}</span>
      <IconButton size="tiny" :title="t('fm.treeLocate')" @click="reveal(current)"><Locate :size="13" /></IconButton>
      <IconButton size="tiny" :title="t('fm.treeCollapse')" @click="collapseAll"><ChevronsDownUp :size="13" /></IconButton>
    </div>
    <div ref="scroller" class="rows" tabindex="0" @scroll="scrollTop = scroller!.scrollTop" @keydown="onKey">
      <div class="spacer" :style="{ height: `${rows.length * ROW_HEIGHT}px` }">
        <div
          v-for="({ node, depth }, i) in visible"
          :key="node.path"
          class="row"
          :class="{ current: node.path === current, cursor: node.path === cursor, failed: !!node.error }"
          :style="{ top: `${(windowStart + i) * ROW_HEIGHT}px`, paddingLeft: `${4 + depth * INDENT}px` }"
          :title="node.error ?? node.path"
          @click="clickRow(node)"
          @contextmenu.stop="openMenu($event, node)"
        >
          <span v-if="depth" class="guides" :style="{ width: `${depth * INDENT}px` }" />
          <span class="twist" @click.stop="toggle(node)">
            <LoaderCircle v-if="loading.has(node.path) || node.path === pending" :size="12" class="spin" />
            <ChevronRight v-else-if="expandable(node, showHidden)" :size="13" class="chevron" :class="{ open: expanded.has(node.path) }" />
          </span>
          <FileIcon :name="node.name" is-dir :kind="node.kind === 'link' ? 'link' : 'dir'" :size="15" :open="expanded.has(node.path)" />
          <span class="ellipsis name">{{ node.name }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dir-tree {
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.head {
  display: flex;
  align-items: center;
  height: 24px;
  flex-shrink: 0;
  padding: 0 2px 0 10px;
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
  font-size: 12px;
}

.title {
  flex: 1;
}

.rows {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden auto;
  outline: none;
}

.spacer {
  position: relative;
  min-height: 100%;
}

.row {
  position: absolute;
  left: 0;
  right: 0;
  display: flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding-right: 8px;
  cursor: pointer;
}

.row:hover {
  background: var(--bg-hover);
}

.row.current {
  background: var(--bg-active);
  box-shadow: inset 2px 0 0 var(--accent);
}

/* 只有目录树拥有键盘焦点时，才显示键盘所在的位置。 */
.rows:focus .row.cursor {
  outline: 1px solid var(--accent-border);
  outline-offset: -1px;
}

.row.failed .name {
  color: var(--text-dim);
  text-decoration: line-through;
}

/* 每一层一条细线，位于该层所属文件夹的箭头正下方。 */
.guides {
  position: absolute;
  inset: 0 auto 0 4px;
  background: repeating-linear-gradient(90deg, transparent 0 7px, var(--border) 7px 8px, transparent 8px 14px);
  opacity: 0;
  transition: opacity 0.15s;
  pointer-events: none;
}

.rows:hover .guides,
.rows:focus .guides {
  opacity: 1;
}

.twist {
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 16px;
  height: 100%;
  margin-right: -3px;
  color: var(--text-dim);
}

.twist:hover {
  color: var(--text);
}

.chevron {
  transition: transform 0.12s;
}

.chevron.open {
  transform: rotate(90deg);
}

.spin {
  animation: dir-tree-spin 0.8s linear infinite;
}

.row :deep(svg) {
  flex-shrink: 0;
}

@keyframes dir-tree-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .chevron,
  .guides {
    transition: none;
  }
}
</style>
