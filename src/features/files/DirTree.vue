<script setup lang="ts">
// 目录树：虚拟滚动，树为非响应式的普通数据（dirTree.ts），避免大目录创建大量响应式对象。
import { ChevronRight, ChevronsDownUp, LoaderCircle, Locate } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { errorMessage, type FileEntry } from "@/shared/api";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { t } from "@/shared/i18n";
import { applyListing, ensurePath, expandable, findNode, pathChain, rootNode, visibleRows, type DirNode } from "@/features/files/dirTree";
import { dim, line, lineFill } from "@/shared/ui-kit/styles";

const props = defineProps<{
  current: string;
  /** 正在前往的文件夹。 */
  pending: string | null;
  showHidden: boolean;
  load: (path: string) => Promise<FileEntry[]>;
}>();
const emit = defineEmits<{ open: [path: string]; menu: [event: MouseEvent, node: DirNode] }>();

const ROW_HEIGHT = 24;
const INDENT = 14;

const root = rootNode();
/** 树为普通数据，变化后递增以触发更新。 */
const version = ref(0);
const expanded = shallowRef<ReadonlySet<string>>(new Set(["/"]));
const loading = shallowRef<ReadonlySet<string>>(new Set());
const cursor = ref("/");

const rows = computed(() => {
  void version.value;
  return visibleRows(root, expanded.value, props.showHidden, props.current);
});

/** 进行中的请求，避免重复请求同一文件夹。 */
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
        // 不自动重试，避免反复请求不可读的文件夹。
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

/** 复用文件列表已获取的数据，省去重复请求。 */
function sync(path: string, entries: FileEntry[]) {
  applyListing(ensurePath(root, path), entries);
  version.value++;
}

function reload(path: string) {
  const node = findNode(root, path);
  if (node?.loaded) void loadNode(node, true);
}

/** 展开到 path 并滚动到可见。 */
async function reveal(path: string) {
  if (!path) return;
  const chain = pathChain(path);
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

/** 重试列出失败的文件夹（通常在重连后）。 */
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

const scroller = ref<HTMLElement>();
const scrollTop = ref(0);
const viewHeight = ref(0);
let observer: ResizeObserver | undefined;

const windowStart = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - 6));
const visible = computed(() =>
  rows.value.slice(windowStart.value, windowStart.value + Math.ceil(viewHeight.value / ROW_HEIGHT) + 12),
);

/** 滚动到可见；已可见则不动。 */
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
    // 隐藏期间当前文件夹可能已变化。
    if (shown) void nextTick(() => scrollToPath(props.current, true));
  });
  observer.observe(scroller.value!);
});

onBeforeUnmount(() => observer?.disconnect());

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
    // 先展开，再进入。
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
  <div class="flex h-full min-w-0 flex-col">
    <div :class="[line, dim, 'flex h-7 shrink-0 items-center border-b pr-1 pl-3 text-xs']">
      <span class="flex-1 truncate font-medium">{{ t("fm.tree") }}</span>
      <IconButton size="tiny" :title="t('fm.treeLocate')" @click="reveal(current)"><Locate :size="13" /></IconButton>
      <IconButton size="tiny" :title="t('fm.treeCollapse')" @click="collapseAll"><ChevronsDownUp :size="13" /></IconButton>
    </div>
    <div ref="scroller" class="group/rows relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-1 outline-none" tabindex="0" @scroll="scrollTop = scroller!.scrollTop" @keydown="onKey">
      <div class="relative min-h-full" :style="{ height: `${rows.length * ROW_HEIGHT}px` }">
        <div
          v-for="({ node, depth }, i) in visible"
          :key="node.path"
          class="absolute inset-x-0 flex h-6 cursor-pointer items-center gap-1 rounded-md pr-2 text-sm"
          :class="[
            node.path === current ? 'bg-blue-500/15 font-medium text-blue-700 dark:bg-blue-400/20 dark:text-blue-200' : 'hover:bg-zinc-900/5 dark:hover:bg-white/8',
            node.path === cursor && 'group-focus/rows:ring-1 group-focus/rows:ring-blue-500/60 group-focus/rows:ring-inset',
          ]"
          :style="{ top: `${(windowStart + i) * ROW_HEIGHT}px`, paddingLeft: `${4 + depth * INDENT}px` }"
          :title="node.error ?? node.path"
          @click="clickRow(node)"
          @contextmenu.stop="openMenu($event, node)"
        >
          <!-- 每层一条缩进线，对齐父级箭头。 -->
          <span v-if="depth" class="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-150 group-hover/rows:opacity-100 group-focus/rows:opacity-100 motion-reduce:transition-none">
            <span v-for="level in depth" :key="level" :class="[lineFill, 'absolute inset-y-0 w-px']" :style="{ left: `${4 + (level - 1) * INDENT + 7}px` }" />
          </span>
          <span :class="[dim, '-mr-1 grid h-full w-4 shrink-0 place-items-center hover:text-zinc-800 dark:hover:text-zinc-100']" @click.stop="toggle(node)">
            <LoaderCircle v-if="loading.has(node.path) || node.path === pending" :size="12" class="animate-spin" />
            <ChevronRight v-else-if="expandable(node, showHidden)" :size="13" class="transition-transform duration-150 motion-reduce:transition-none" :class="{ 'rotate-90': expanded.has(node.path) }" />
          </span>
          <FileIcon :name="node.name" is-dir :kind="node.kind === 'link' ? 'link' : 'dir'" :size="15" :open="expanded.has(node.path)" class="shrink-0" />
          <span class="truncate" :class="node.error && [dim, 'line-through']">{{ node.name }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
