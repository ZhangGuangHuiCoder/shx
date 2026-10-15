<script setup lang="ts">
import {
  ArrowLeft, ArrowRight, ArrowUp, ChevronRight, ChevronsDownUp, ChevronsUpDown, ClipboardCopy, ClipboardPaste, Copy, Download, Eye, EyeOff,
  FileArchive, FilePlus, Files, Folder, FolderArchive, FolderOpen, FolderPlus, FolderSearch, FolderUp, House, LocateFixed,
  PackageOpen, PanelLeft, Pencil, RefreshCw, Scissors, Search, SlidersHorizontal, SquarePen, SquareTerminal, Star, StarOff, Trash2, Upload, X,
} from "@lucide/vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiSpin from "@/shared/ui-kit/UiSpin.vue";
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api, errorMessage, type Conflict, type FileEntry, type Transfer } from "@/shared/api";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import { openContextMenu, shortcut, type MenuEntry } from "@/shared/contextMenu";
import { t, type MessageKey } from "@/shared/i18n";
import { choose, confirm, message, prompt } from "@/shared/ui";
import { baseName, formatMode, formatOctal, formatShortTime, formatSize, formatTime, isArchive, joinPath, parentPath, ruleMatcher, shellQuote } from "@/shared/util";
import { uiFamily } from "@/shared/fonts";
import { FILE_TREE_WIDTH, useApp } from "@/shared/store";
import ArchiveDialog from "@/features/files/ArchiveDialog.vue";
import { compareNames, type DirNode } from "@/features/files/dirTree";
import DirTree from "@/features/files/DirTree.vue";
import EditorModal from "@/features/files/EditorModal.vue";
import PropsDialog from "@/features/files/PropsDialog.vue";
import { accentText, capsule, dim, glassBar, line, selectedFill, toolGroup, well } from "@/shared/ui-kit/styles";

const crumb = "cursor-pointer rounded-sm px-1 py-px hover:bg-zinc-900/5 dark:hover:bg-white/10";
const rowHover = "hover:bg-zinc-900/5 dark:hover:bg-white/5";

const props = defineProps<{ sessionId: string; /** 收藏目录按主机保存。 */ profileId: string; active: boolean }>();
const emit = defineEmits<{ send: [data: string] }>();
const app = useApp();
const side = computed(() => app.settings.fileLayout === "right");

const ROW_HEIGHT = 26;

const editable = computed(() => ruleMatcher(app.settings.editRules));

const path = ref("");
const entries = ref<FileEntry[]>([]);
const loading = ref(false);
const back = ref<string[]>([]);
const forward = ref<string[]>([]);
const filter = ref("");
/** 非 null 时显示搜索结果。 */
const found = ref<FileEntry[] | null>(null);
const foundFor = ref("");

type SortKey = "name" | "size" | "mtime" | "mode" | "owner";
const sortKey = ref<SortKey>("name");
const sortAsc = ref(true);

const COLUMNS: { key: SortKey; label: MessageKey }[] = [
  { key: "name", label: "col.name" },
  { key: "size", label: "col.size" },
  { key: "mtime", label: "col.mtime" },
  { key: "mode", label: "col.mode" },
  { key: "owner", label: "col.owner" },
];

/** 按需统计的文件夹大小（路径 → 字节），统计中为 null；切换目录前保留。 */
const dirSizes = ref(new Map<string, number | null>());

async function measure(entry: FileEntry) {
  dirSizes.value.set(entry.path, null);
  try {
    dirSizes.value.set(entry.path, await api.du(props.sessionId, entry.path));
  } catch (e) {
    dirSizes.value.delete(entry.path);
    message.error(errorMessage(e));
  }
}

/** 已知大小；文件夹未统计时不计。 */
const sizeOf = (e: FileEntry) => (e.isDir ? dirSizes.value.get(e.path) ?? -1 : e.size);

const ownerText = (e: FileEntry) => `${e.owner ?? e.uid ?? ""}:${e.group ?? e.gid ?? ""}`;

const rows = computed(() => {
  const needle = filter.value.trim().toLowerCase();
  const list = (found.value ?? entries.value).filter(
    (e) =>
      (found.value !== null || app.settings.showHidden || !e.name.startsWith(".")) &&
      (!needle || e.name.toLowerCase().includes(needle)),
  );
  const dir = sortAsc.value ? 1 : -1;
  const by: Record<SortKey, (a: FileEntry, b: FileEntry) => number> = {
    name: (a, b) => compareNames(a.name, b.name),
    size: (a, b) => sizeOf(a) - sizeOf(b),
    mtime: (a, b) => (a.mtime ?? 0) - (b.mtime ?? 0),
    mode: (a, b) => (a.mode ?? 0) - (b.mode ?? 0),
    owner: (a, b) => ownerText(a).localeCompare(ownerText(b)),
  };
  // 文件夹在前；相等时按名称，保证稳定。
  return list.sort((a, b) => Number(b.isDir) - Number(a.isDir) || dir * (by[sortKey.value](a, b) || by.name(a, b)));
});

function sortBy(key: SortKey) {
  if (sortKey.value === key) sortAsc.value = !sortAsc.value;
  else [sortKey.value, sortAsc.value] = [key, true];
}

/** navigate 等待响应时的目标路径。 */
const pendingDir = ref<string | null>(null);
let navigation = 0;

async function navigate(target: string, record = true) {
  // 只采用最新请求的结果，忽略乱序到达的旧响应。
  const turn = ++navigation;
  loading.value = true;
  pendingDir.value = target;
  try {
    const listing = await api.list(props.sessionId, target);
    if (turn !== navigation) return false;
    if (record && path.value && path.value !== listing.path) {
      back.value.push(path.value);
      forward.value = [];
    }
    if (path.value !== listing.path) dirSizes.value.clear();
    path.value = listing.path;
    entries.value = listing.entries;
    found.value = null;
    filter.value = "";
    clearSelection();
    scroller.value?.scrollTo({ top: 0 });
    tree.value?.sync(listing.path, listing.entries);
    return true;
  } catch (e) {
    if (turn === navigation) message.error(errorMessage(e));
    return false;
  } finally {
    if (turn === navigation) {
      loading.value = false;
      pendingDir.value = null;
    }
  }
}

async function reload() {
  const [session, dir] = [props.sessionId, path.value];
  const listing = await api.list(session, dir);
  // 等待期间已切换目录或连接，丢弃结果。
  if (session !== props.sessionId || dir !== path.value) return;
  entries.value = listing.entries;
  const alive = new Set(listing.entries.map((e) => e.path));
  selected.value = new Set([...selected.value].filter((p) => alive.has(p)));
  tree.value?.sync(listing.path, listing.entries);
}

async function refresh() {
  if (!path.value) return;
  await reload().catch((e) => message.error(errorMessage(e)));
}

// 重连后保持原视图；原目录不存在时回到主目录。
watch(
  () => props.sessionId,
  async () => {
    navigation++;
    loading.value = busy.value = false;
    pendingDir.value = null;
    tree.value?.retry();
    if (!path.value || !(await reload().then(() => true, () => false))) await navigate(".", false);
  },
);

async function goBack() {
  const target = back.value.pop();
  if (!target) return;
  const from = path.value;
  if (await navigate(target, false)) forward.value.push(from);
  else back.value.push(target);
}

async function goForward() {
  const target = forward.value.pop();
  if (!target) return;
  const from = path.value;
  if (await navigate(target, false)) back.value.push(from);
  else forward.value.push(target);
}

const editingPath = ref(false);
const pathText = ref("");
const pathInput = ref<InstanceType<typeof UiInput>>();

const crumbs = computed(() => {
  const parts = path.value.split("/").filter(Boolean);
  return parts.map((name, i) => ({ name, path: `/${parts.slice(0, i + 1).join("/")}` }));
});

async function editPath() {
  pathText.value = path.value;
  editingPath.value = true;
  await nextTick();
  pathInput.value?.select();
}

async function submitPath() {
  editingPath.value = false;
  const target = pathText.value.trim();
  if (target && target !== path.value) await navigate(target);
}

const favorites = computed(() => app.profiles.find((p) => p.id === props.profileId)?.favoriteDirs ?? []);
const isFavorite = (dir: string) => favorites.value.includes(dir);

function toggleFavorite(dir: string) {
  // 首次列表返回前当前目录未知。
  if (!dir) return;
  const was = isFavorite(dir);
  app.toggleFavoriteDir(props.profileId, dir);
  // 主机已被删除。
  if (isFavorite(dir) !== was) message.success(t(was ? "fm.unfavorited" : "fm.favorited"));
}

/** 收藏/取消收藏菜单项；current 表示 dir 为当前目录。 */
function favoriteEntry(dir: string, current = false): MenuEntry {
  const on = isFavorite(dir);
  return {
    label: t(`fm.${on ? "unfavorite" : "favorite"}${current ? "Current" : ""}`),
    icon: on ? StarOff : Star,
    hint: current ? shortcut.mod("D") : undefined,
    action: () => toggleFavorite(dir),
  };
}

function openFavorites(e: MouseEvent) {
  const dirs = favorites.value;
  openContextMenu(e, [
    { group: t(dirs.length ? "fm.favorites" : "fm.noFavorites") },
    ...dirs.map((dir): MenuEntry => ({
      label: baseName(dir) || "/",
      hint: dir === "/" ? undefined : parentPath(dir),
      icon: dir === path.value ? FolderOpen : Folder,
      action: () => navigate(dir),
      aside: { icon: X, title: t("fm.unfavorite"), action: () => toggleFavorite(dir) },
    })),
    "divider",
    favoriteEntry(path.value, true),
  ]);
}

const tree = ref<InstanceType<typeof DirTree>>();
const body = ref<HTMLElement>();
let stopTreeResize: (() => void) | undefined;

const loadDir = (dir: string) => api.list(props.sessionId, dir).then((listing) => listing.entries);

/** 拖动中的宽度，松手才写入偏好，避免频繁存盘。 */
const draggedTreeWidth = ref<number | null>(null);
const treeWidth = computed(() => draggedTreeWidth.value ?? app.fileTree.width);

function resizeTree(e: MouseEvent) {
  e.preventDefault();
  const left = body.value!.getBoundingClientRect().left;
  const move = (ev: MouseEvent) => {
    draggedTreeWidth.value = Math.round(Math.min(FILE_TREE_WIDTH.max, Math.max(FILE_TREE_WIDTH.min, ev.clientX - left)));
  };
  stopTreeResize = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", stopTreeResize!);
    stopTreeResize = undefined;
    if (draggedTreeWidth.value !== null) app.fileTree.width = draggedTreeWidth.value;
    draggedTreeWidth.value = null;
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", stopTreeResize);
}

function openTreeMenu(e: MouseEvent, node: DirNode) {
  const dir = node.path;
  const clip = clipboard.value;
  const open = tree.value?.isExpanded(dir);
  openContextMenu(e, [
    { heading: node.name, detail: dir === "/" ? undefined : dir, icon: () => h(FileIcon, { name: node.name, isDir: true, kind: node.kind === "link" ? "link" : "dir", size: 18 }) },
    { label: t("fm.open"), icon: FolderOpen, disabled: dir === path.value, action: () => navigate(dir) },
    { label: t(open ? "fm.treeCollapseOne" : "fm.treeExpandOne"), icon: open ? ChevronsDownUp : ChevronsUpDown, action: () => tree.value?.toggle(node) },
    { label: t("common.refresh"), icon: RefreshCw, action: () => (dir === path.value ? refresh() : tree.value?.reload(dir)) },
    "divider",
    { label: t("fm.newFolder"), icon: FolderPlus, action: () => create("dir", dir) },
    { label: clip ? t("fm.pasteCount", { count: clip.paths.length }) : t("common.paste"), icon: ClipboardPaste, disabled: !clip, action: () => paste(dir) },
    { label: t("fm.download"), icon: Download, disabled: dir === "/", action: () => download([{ name: node.name, path: dir }]) },
    { label: t("fm.packDownload"), icon: FolderArchive, disabled: dir === "/", action: () => packDownload([{ path: dir }]) },
    "divider",
    { label: t("fm.copyPath"), icon: ClipboardCopy, action: () => copyText(dir) },
    { label: t("fm.cd"), icon: SquareTerminal, action: () => cdTerminal(dir) },
    favoriteEntry(dir),
  ]);
}

const selected = ref(new Set<string>());
let anchor = -1;
const scroller = ref<HTMLElement>();
const scrollTop = ref(0);
/** 列表横向滚动量，表头同步平移。 */
const scrollLeft = ref(0);
const viewHeight = ref(300);

function onScroll() {
  scrollTop.value = scroller.value!.scrollTop;
  scrollLeft.value = scroller.value!.scrollLeft;
}

/** 顶部“..”行的高度；该行不是条目，不可选中、不计数。根目录和搜索结果中没有。 */
const lead = computed(() => (path.value && path.value !== "/" && found.value === null ? ROW_HEIGHT : 0));

const selection = computed(() => rows.value.filter((e) => selected.value.has(e.path)));

/**
 * 名称列按最长名称测量定宽，多余宽度留在最右，避免名称与其他列间出现大段空白。
 * 每行是独立网格无法自动定宽，故用 canvas 测量；宽度有上下限。
 */
const NAME_MIN = 178;
const NAME_MAX = 420;
/** 图标、间距、内边距及余量。 */
const NAME_EXTRA = 15 + 8 + 16 + 4;
const LINK_GAP = 8;

/** canvas 测量用字体；字体变化时测量结果失效。 */
const listFont = ref("");
const fontEpoch = ref(0);
const widthCache = new Map<string, number>();
let measurer: CanvasRenderingContext2D | null = null;

function readListFont() {
  if (!scroller.value) return;
  const style = getComputedStyle(scroller.value);
  listFont.value = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
  widthCache.clear();
  fontEpoch.value++;
}

function textWidth(text: string, font = listFont.value): number {
  const key = `${font}\n${text}`;
  let width = widthCache.get(key);
  if (width === undefined) {
    measurer ??= document.createElement("canvas").getContext("2d");
    if (!measurer) return 0;
    measurer.font = font;
    width = measurer.measureText(text).width;
    widthCache.set(key, width);
  }
  return width;
}

const nameWidth = computed(() => {
  void fontEpoch.value;
  if (!listFont.value) return NAME_MIN;
  let widest = textWidth("..");
  for (const e of rows.value) {
    const width = textWidth(e.name) + (e.linkTarget ? LINK_GAP + textWidth(`→ ${e.linkTarget}`) : 0);
    if (width > widest) widest = width;
  }
  return Math.round(Math.min(NAME_MAX, Math.max(NAME_MIN, widest + NAME_EXTRA)));
});

/** 宽度不足时改用紧凑列（短时间、只显示用户），完整内容放在悬停提示中。 */
const listWidth = ref(0);
/** 完整写法下四个定宽列的总宽，须与 cells 一致。 */
const FULL_COLUMNS = 76 + 136 + 52 + 110;
const compact = computed(() => listWidth.value > 0 && listWidth.value < nameWidth.value + FULL_COLUMNS);

/**
 * 列：名称 | 大小 | 时间 | 权限 | 所有者 | 空白。名称列宽度为测量值，其余定宽；
 * 最小总宽须与 session/layout.ts 一致，多余宽度留给最后的空白列。
 */
const CELL = "shrink-0 overflow-hidden px-2 whitespace-nowrap";
const cells = computed(() => {
  const narrow = compact.value;
  const name = "min-w-44.5 shrink overflow-hidden px-2 whitespace-nowrap";
  const rest = [`${CELL} w-19`, `${CELL} ${narrow ? "w-24" : "w-34"}`, `${CELL} w-13`, `${CELL} ${narrow ? "w-20" : "w-27.5"}`];
  // 预先拼好类名，避免滚动时逐格重复拼接。
  const mono = `${dim} font-mono text-xs`;
  return {
    name,
    rest,
    row: narrow ? "flex min-w-120.5 items-center" : "flex min-w-138 items-center",
    nameCell: `${name} flex items-center gap-2`,
    size: `${rest[0]} text-right`,
    time: `${rest[1]} ${mono}`,
    perm: `${rest[2]} ${mono}`,
    owner: `${rest[3]} ${mono} truncate`,
  };
});
const nameStyle = computed(() => ({ width: `${nameWidth.value}px` }));

watch(uiFamily, () => void nextTick(readListFont));
const onFontsLoaded = () => readListFont();
document.fonts?.addEventListener("loadingdone", onFontsLoaded);
onBeforeUnmount(() => document.fonts?.removeEventListener("loadingdone", onFontsLoaded));
const windowStart = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - 6));
const visible = computed(() =>
  rows.value.slice(windowStart.value, windowStart.value + Math.ceil(viewHeight.value / ROW_HEIGHT) + 12),
);

function clearSelection() {
  selected.value = new Set();
  anchor = -1;
}

function selectOnly(index: number) {
  const entry = rows.value[index];
  if (!entry) return;
  selected.value = new Set([entry.path]);
  anchor = index;
  const top = lead.value + index * ROW_HEIGHT;
  const el = scroller.value;
  if (!el) return;
  // 回到首项时滚到顶，露出“..”行。
  if (top < el.scrollTop) el.scrollTop = index ? top : 0;
  else if (top + ROW_HEIGHT > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW_HEIGHT - el.clientHeight;
}

function clickRow(e: MouseEvent, index: number) {
  const entry = rows.value[index];
  if (e.shiftKey && anchor >= 0) {
    const [from, to] = anchor < index ? [anchor, index] : [index, anchor];
    selected.value = new Set(rows.value.slice(from, to + 1).map((r) => r.path));
  } else if (e.metaKey || e.ctrlKey) {
    const next = new Set(selected.value);
    if (!next.delete(entry.path)) next.add(entry.path);
    selected.value = next;
    anchor = index;
  } else {
    selected.value = new Set([entry.path]);
    anchor = index;
  }
}

const busy = ref(false);
const showProps = ref(false);
const propsItems = ref<FileEntry[]>([]);
const clipboard = ref<{ cut: boolean; paths: string[] } | null>(null);
let lastDownloadDir: string | null = null;

/** 执行修改操作，无论成败都刷新列表。 */
async function run(op: () => Promise<unknown>, done?: string) {
  busy.value = true;
  try {
    await op();
    if (done) message.success(done);
  } catch (e) {
    message.error(errorMessage(e));
  } finally {
    busy.value = false;
    await refresh();
  }
}

const editing = ref<string | null>(null);

function edit(entry: FileEntry) {
  editing.value = entry.path;
}

function open(entry: FileEntry) {
  if (entry.isDir) return void navigate(entry.path);
  if (editable.value(entry.name) || app.settings.otherDoubleClick === "edit") edit(entry);
  else if (app.settings.otherDoubleClick === "download") void download([entry]);
}

const CONFLICTS: Conflict[] = ["overwrite", "skip", "rename", "resume"];

async function askConflict(clashes: string[]): Promise<Conflict | null> {
  if (!clashes.length) return "overwrite";
  const options = CONFLICTS.map((value) => ({ value, label: t(`conflict.${value}`), hint: t(`conflict.${value}Hint`) }));
  return choose({
    title: t("conflict.title"),
    icon: Copy,
    content: t("conflict.text", { count: clashes.length }),
    items: clashes.map((name) => ({ name })),
    options,
    initial: "overwrite",
  });
}

async function upload(localPaths: string[]) {
  if (!localPaths.length || !path.value) return;
  const existing = new Set(entries.value.map((e) => e.name));
  const conflict = await askConflict(localPaths.map(baseName).filter((n) => existing.has(n)));
  if (!conflict) return;
  await api.transferStart(props.sessionId, "upload", localPaths, path.value, conflict).catch((e) => message.error(errorMessage(e)));
}

async function pickUpload(directory: boolean) {
  const picked = await openDialog({ multiple: true, directory, title: t(directory ? "fm.pickUploadFolders" : "fm.pickUploadFiles") });
  if (picked) await upload(Array.isArray(picked) ? picked : [picked]);
}

/** 选择下载位置，关闭时返回 null。 */
async function pickDownloadDir(): Promise<string | null> {
  const dir = await openDialog({
    directory: true,
    title: t("fm.downloadTo"),
    defaultPath: lastDownloadDir ?? (await api.defaultDownloadDir()) ?? undefined,
  });
  if (typeof dir !== "string") return null;
  lastDownloadDir = dir;
  return dir;
}

async function download(items: Pick<FileEntry, "name" | "path">[]) {
  if (!items.length) return;
  const dir = await pickDownloadDir();
  if (!dir) return;
  try {
    const conflict = await askConflict(await api.localExisting(dir, items.map((e) => e.name.split("/").pop()!)));
    if (conflict) await api.transferStart(props.sessionId, "download", items.map((e) => e.path), dir, conflict);
  } catch (e) {
    message.error(errorMessage(e));
  }
}

const archiveDialog = ref<InstanceType<typeof ArchiveDialog>>();

/** 默认压缩包名。 */
const archiveStem = (dir: string, names: string[]) => (names.length === 1 ? names[0] : baseName(dir) || "archive");

/** 服务器端打包后下载单个文件；打包阶段已计入传输任务。 */
async function packDownload(items: Pick<FileEntry, "path">[]) {
  if (!items.length) return;
  const from = parentPath(items[0].path);
  const names = items.map((e) => baseName(e.path));
  const how = await archiveDialog.value?.ask({ mode: "download", dir: from, names, suggested: archiveStem(from, names) });
  if (!how) return;
  const dir = await pickDownloadDir();
  if (!dir) return;
  try {
    const conflict = await askConflict(await api.localExisting(dir, [how.archive]));
    if (conflict) await api.transferPack(props.sessionId, from, names, how.archive, how.level, dir, conflict);
  } catch (e) {
    message.error(errorMessage(e));
  }
}

/** 校验新建或重命名的名称，合法时返回 null；own 为原名，允许不变。 */
function nameProblem(dir: string, own = "") {
  const taken = new Set(dir === path.value ? entries.value.map((e) => e.name) : []);
  return (value: string) => {
    const name = value.trim();
    if (name.includes("/")) return t("fm.noSlash");
    return name !== own && taken.has(name) ? t("fm.nameTaken") : null;
  };
}

async function create(kind: "file" | "dir", dir = path.value) {
  const answer = await prompt({
    title: t(kind === "dir" ? "fm.newFolder" : "fm.newFile"),
    detail: t("fm.inFolder", { path: dir }),
    icon: kind === "dir" ? FolderPlus : FilePlus,
    placeholder: t("common.name"),
    validate: nameProblem(dir),
  });
  const name = answer?.value.trim();
  if (!name) return;
  const target = joinPath(dir, name);
  await run(() => (kind === "dir" ? api.mkdir(props.sessionId, target) : api.createFile(props.sessionId, target)));
  if (dir === path.value) selected.value = new Set([target]);
  else tree.value?.reload(dir);
}

async function rename(entry: FileEntry) {
  const name = baseName(entry.path);
  const dot = name.lastIndexOf(".");
  const answer = await prompt({
    title: t("fm.rename"),
    detail: name,
    icon: Pencil,
    initial: name,
    selectTo: dot > 0 && !entry.isDir ? dot : name.length,
    validate: nameProblem(parentPath(entry.path), name),
  });
  const next = answer?.value.trim();
  if (!next || next === name) return;
  const target = joinPath(parentPath(entry.path), next);
  await run(() => api.rename(props.sessionId, entry.path, target));
  selected.value = new Set([target]);
}

async function remove(items: FileEntry[]) {
  if (!items.length) return;
  const sure = await confirm(t("fm.deleteTitle"), t("fm.deleteText", { count: items.length }), {
    danger: true,
    icon: Trash2,
    ok: t("common.delete"),
    items: items.map((e) => ({ name: e.name, isDir: e.isDir, kind: e.kind })),
  });
  if (!sure) return;
  await run(() => api.remove(props.sessionId, items.map((e) => e.path)));
  if (found.value) found.value = found.value.filter((e) => !items.includes(e));
}

async function paste(dir = path.value) {
  const clip = clipboard.value;
  if (!clip || !dir) return;
  await run(() => (clip.cut ? api.move : api.copy)(props.sessionId, clip.paths, dir));
  if (clip.cut) clipboard.value = null;
  // 目录树中相关文件夹也需刷新。
  const touched = new Set([dir, ...(clip.cut ? clip.paths.map(parentPath) : [])]);
  touched.delete(path.value);
  touched.forEach((changed) => tree.value?.reload(changed));
}

async function compress(items: FileEntry[]) {
  const dir = path.value;
  const names = items.map((e) => e.name);
  const how = await archiveDialog.value?.ask({
    mode: "compress",
    dir,
    names,
    suggested: archiveStem(dir, names),
    existing: entries.value.map((e) => e.name),
  });
  if (!how) return;
  await run(() => api.compress(props.sessionId, dir, names, how.archive, how.level, how.removeSource), t("fm.compressed"));
  if (dir === path.value && entries.value.some((e) => e.name === how.archive)) selected.value = new Set([joinPath(dir, how.archive)]);
}

async function search() {
  const answer = await prompt({
    title: t("fm.searchTitle"),
    detail: t("fm.inFolder", { path: path.value }),
    icon: Search,
    placeholder: t("fm.searchPlaceholder"),
    ok: t("common.search"),
  });
  const pattern = answer?.value.trim();
  if (!pattern) return;
  loading.value = true;
  try {
    found.value = await api.search(props.sessionId, path.value, pattern);
    foundFor.value = pattern;
    clearSelection();
  } catch (e) {
    message.error(errorMessage(e));
  } finally {
    loading.value = false;
  }
}

function showProperties(items: FileEntry[]) {
  propsItems.value = items;
  showProps.value = true;
}

async function copyText(text: string) {
  await writeText(text);
  message.success(t("fm.copied"));
}

const follow = ref(false);
let injected = false;

// 让 bash/zsh 每次提示符前用 OSC 7 报告工作目录；tmux 中需额外包装才能透传。
// 开头的空格可使该命令不进入 shell 历史。
const CWD_HOOK = String.raw` __shx_cwd() { if [ -n "$TMUX" ]; then printf '\033Ptmux;\033\033]7;file://%s%s\033\033\\\033\\' "${"$"}{HOST:-$HOSTNAME}" "$PWD"; else printf '\033]7;file://%s%s\033\\' "${"$"}{HOST:-$HOSTNAME}" "$PWD"; fi; }; if [ -n "$ZSH_VERSION" ]; then precmd_functions+=(__shx_cwd); elif [ -n "$BASH_VERSION" ]; then PROMPT_COMMAND="__shx_cwd${"$"}{PROMPT_COMMAND:+;$PROMPT_COMMAND}"; fi`;

let shellCwd = "";

function toggleFollow() {
  follow.value = !follow.value;
  if (follow.value && !injected) {
    injected = true;
    emit("send", `${CWD_HOOK}; clear\r`);
  }
}

function terminalCwd(dir: string) {
  shellCwd = dir;
  if (follow.value && dir !== path.value && !loading.value) void navigate(dir);
}

/** 重连后新 shell 没有钩子：重新安装并回到原目录。不用 clear，以免清掉回滚缓冲区。 */
function shellRestarted() {
  injected = false;
  if (!follow.value) return;
  injected = true;
  const back = shellCwd ? ` cd ${shellQuote(shellCwd)} 2>/dev/null;` : "";
  emit("send", `${back}${CWD_HOOK}; printf '\\033[H\\033[2J'\r`);
}

function cdTerminal(dir: string) {
  emit("send", ` cd ${shellQuote(dir)}\r`);
}

function openMenu(e: MouseEvent, index: number | null) {
  if (index !== null && !selected.value.has(rows.value[index].path)) selectOnly(index);
  if (index === null) clearSelection();
  const items = selection.value;
  const one = items.length === 1 ? items[0] : null;
  const searching = found.value !== null;
  const paths = items.map((item) => item.path);
  const clip = clipboard.value;
  /** 过滤掉条件为假的菜单项。 */
  const when = (...list: (MenuEntry | false | null | undefined)[]) => list.filter((x): x is MenuEntry => !!x);

  const size = items.reduce((n, item) => n + Math.max(0, sizeOf(item)), 0);
  const entries: MenuEntry[] = items.length
    ? when(
        // 标明作用对象，多选时尤其需要。
        one
          ? {
              heading: one.name,
              detail: one.isDir ? t(`kind.${one.kind === "link" ? "link" : "dir"}`) : formatSize(one.size),
              icon: () => h(FileIcon, { name: one.name, isDir: one.isDir, kind: one.kind, size: 18, text: editable.value(one.name) }),
            }
          : { heading: t("fm.menuSelected", { count: items.length }), detail: size ? formatSize(size) : undefined, icon: () => h(Files, { size: 18, class: "text-zinc-500 dark:text-zinc-400" }) },
        one && { label: t(one.isDir ? "fm.open" : "fm.openByRule"), icon: FolderOpen, hint: shortcut.enter, action: () => open(one) },
        one && !one.isDir && { label: t("common.edit"), icon: SquarePen, action: () => edit(one) },
        one && searching && { label: t("fm.reveal"), icon: FolderSearch, action: () => navigate(parentPath(one.path)) },
        { label: t("fm.download"), icon: Download, action: () => download(items) },
        // 打包下载比逐个传小文件快得多。
        !searching && items.some((item) => item.isDir) && { label: t("fm.packDownload"), icon: FolderArchive, action: () => packDownload(items) },
        "divider",
        { label: t("common.copy"), icon: Copy, hint: shortcut.mod("C"), action: () => (clipboard.value = { cut: false, paths }) },
        { label: t("common.cut"), icon: Scissors, hint: shortcut.mod("X"), action: () => (clipboard.value = { cut: true, paths }) },
        one && { label: t("fm.rename"), icon: Pencil, hint: "F2", action: () => rename(one) },
        "divider",
        !searching && { label: t("fm.compress"), icon: FileArchive, action: () => compress(items) },
        one && !one.isDir && isArchive(one.name) && {
          label: t("fm.extract"),
          icon: PackageOpen,
          action: () => run(() => api.extract(props.sessionId, one.path, path.value), t("fm.extracted")),
        },
        { label: t("fm.copyPath"), icon: ClipboardCopy, action: () => copyText(paths.join("\n")) },
        one?.isDir && { label: t("fm.cd"), icon: SquareTerminal, action: () => cdTerminal(one.path) },
        one?.isDir && favoriteEntry(one.path),
        { label: t("fm.properties"), icon: SlidersHorizontal, action: () => showProperties(items) },
        "divider",
        { label: t("common.delete"), icon: Trash2, danger: true, hint: shortcut.delete, action: () => remove(items) },
      )
    : [
        { label: t("fm.newFile"), icon: FilePlus, action: () => create("file") },
        { label: t("fm.newFolder"), icon: FolderPlus, action: () => create("dir") },
        "divider",
        { label: t("fm.uploadFiles"), icon: Upload, action: () => pickUpload(false) },
        { label: t("fm.uploadFolder"), icon: FolderUp, action: () => pickUpload(true) },
        { label: clip ? t("fm.pasteCount", { count: clip.paths.length }) : t("common.paste"), icon: ClipboardPaste, disabled: !clip, hint: shortcut.mod("V"), action: () => paste() },
        "divider",
        { label: t("common.refresh"), icon: RefreshCw, hint: shortcut.mod("R"), action: refresh },
        { label: t("fm.copyCurrentPath"), icon: ClipboardCopy, action: () => copyText(path.value) },
        { label: t("fm.cd"), icon: SquareTerminal, action: () => cdTerminal(path.value) },
        favoriteEntry(path.value, true),
      ];
  openContextMenu(e, entries);
}

function onKey(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey;
  const items = selection.value;
  const key = e.key.toLowerCase();
  const handlers: [boolean, () => unknown][] = [
    [e.key === "ArrowDown", () => selectOnly(Math.min(rows.value.length - 1, anchor + 1))],
    [e.key === "ArrowUp", () => selectOnly(Math.max(0, anchor - 1))],
    [e.key === "Enter" && items.length === 1, () => open(items[0])],
    [e.key === "Delete" || (mod && e.key === "Backspace"), () => remove(items)],
    [e.key === "Backspace" && !mod, () => navigate(parentPath(path.value))],
    [e.key === "F2" && items.length === 1, () => rename(items[0])],
    [e.key === "F5" || (mod && key === "r"), refresh],
    [mod && key === "a", () => (selected.value = new Set(rows.value.map((r) => r.path)))],
    [mod && key === "c" && items.length > 0, () => (clipboard.value = { cut: false, paths: items.map((i) => i.path) })],
    [mod && key === "x" && items.length > 0, () => (clipboard.value = { cut: true, paths: items.map((i) => i.path) })],
    [mod && key === "v", () => paste()],
    [mod && key === "d", () => toggleFavorite(path.value)],
    [e.key === "Escape", clearSelection],
  ];
  const hit = handlers.find(([when]) => when);
  if (!hit) return;
  e.preventDefault();
  void hit[1]();
}

const dropping = ref(false);
const unlisten: (() => void)[] = [];
let unmounted = false;
let observer: ResizeObserver | undefined;
let refreshTimer: ReturnType<typeof setTimeout> | undefined;

/** 注册监听；若注册完成时组件已卸载则立即注销。 */
async function subscribe(registering: Promise<() => void>) {
  const off = await registering;
  if (unmounted) off();
  else unlisten.push(off);
}

onMounted(() => {
  readListFont();
  observer = new ResizeObserver(([box]) => {
    viewHeight.value = box.contentRect.height;
    listWidth.value = box.contentRect.width;
  });
  observer.observe(scroller.value!);
  void navigate(".");

  const drops = getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (!props.active || editing.value) return void (dropping.value = false);
    dropping.value = payload.type === "enter" || payload.type === "over";
    if (payload.type === "drop") void upload(payload.paths);
  });
  const transfers = listen<Transfer>("transfer-update", ({ payload: t }) => {
    const here = t.sessionId === props.sessionId && t.direction === "upload" && t.destDir === path.value;
    if (!here || (t.state !== "done" && t.state !== "running")) return;
    // 合并刷新，避免批量上传时逐个刷新。
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(refresh, t.state === "done" ? 150 : 1500);
  });
  for (const registering of [drops, transfers]) void subscribe(registering).catch(() => {});
});

onBeforeUnmount(() => {
  unmounted = true;
  stopTreeResize?.();
  observer?.disconnect();
  clearTimeout(refreshTimer);
  unlisten.forEach((off) => off());
});

defineExpose({ terminalCwd, shellRestarted });
</script>

<template>
  <div class="flex h-full flex-col bg-white/55 text-sm dark:bg-zinc-900/50">
    <!-- 位于终端右侧时工具栏换行，地址栏独占一行。 -->
    <div :class="[glassBar, line, 'flex shrink-0 items-center gap-2 border-b px-2', side ? 'flex-wrap justify-between py-1.5' : 'h-12']">
      <!-- 地址栏、筛选框与按钮组同高（h-8.5）。 -->
      <div :class="toolGroup">
        <IconButton :title="t('fm.back')" :disabled="!back.length" @click="goBack"><ArrowLeft :size="15" /></IconButton>
        <IconButton :title="t('fm.forward')" :disabled="!forward.length" @click="goForward"><ArrowRight :size="15" /></IconButton>
        <IconButton :title="t('fm.up')" :disabled="path === '/'" @click="navigate(parentPath(path))"><ArrowUp :size="15" /></IconButton>
        <IconButton :title="t('fm.home')" @click="navigate('.')"><House :size="15" /></IconButton>
        <IconButton :title="t('common.refresh')" @click="refresh"><RefreshCw :size="14" /></IconButton>
        <IconButton :selected="isFavorite(path)" :title="t('fm.favorites')" @click="openFavorites">
          <Star :size="15" :fill="isFavorite(path) ? 'currentColor' : 'none'" />
        </IconButton>
      </div>

      <!-- 地址栏：点击空白处可编辑。 -->
      <div :class="[capsule, 'flex h-8.5 items-center overflow-hidden rounded-full px-2', side ? 'order-1 basis-full' : 'min-w-20 flex-1']" @click.self="editPath">
        <UiInput
          v-if="editingPath"
          ref="pathInput"
          v-model:value="pathText"
          bare
          class="h-6 px-1"
          @keydown.enter="submitPath"
          @keydown.esc="editingPath = false"
          @blur="editingPath = false"
        />
        <template v-else>
          <span :class="[crumb, 'min-w-4 shrink']" @click="navigate('/')">/</span>
          <template v-for="(c, i) in crumbs" :key="c.path">
            <ChevronRight v-if="i > 0" :size="12" :class="[dim, 'shrink-0']" />
            <span :class="[crumb, 'min-w-4 shrink truncate']" @click="navigate(c.path)">{{ c.name }}</span>
          </template>
          <span class="min-w-5 flex-1 cursor-text self-stretch" :title="t('fm.editPath')" @click="editPath" />
        </template>
      </div>

      <UiInput v-model:value="filter" size="small" round clearable :placeholder="t('fm.filter')" :class="['h-8.5', side ? 'min-w-24 grow basis-30' : 'w-32! shrink-0']">
        <template #prefix><Search :size="13" :class="dim" /></template>
      </UiInput>
      <div :class="toolGroup">
        <IconButton :title="t('fm.upload')" @click="pickUpload(false)"><Upload :size="15" /></IconButton>
        <IconButton :title="t('fm.downloadSelected')" :disabled="!selection.length" @click="download(selection)"><Download :size="15" /></IconButton>
        <IconButton :title="t('fm.newFile')" @click="create('file')"><FilePlus :size="15" /></IconButton>
        <IconButton :title="t('fm.newFolder')" @click="create('dir')"><FolderPlus :size="15" /></IconButton>
        <IconButton :title="t('fm.deleteSelected')" :disabled="!selection.length" @click="remove(selection)"><Trash2 :size="15" /></IconButton>
        <IconButton :title="t('fm.search')" @click="search"><FolderSearch :size="15" /></IconButton>
      </div>
      <div :class="toolGroup">
        <IconButton :selected="app.settings.showHidden" :title="t('menu.showHidden')" @click="app.settings.showHidden = !app.settings.showHidden">
          <component :is="app.settings.showHidden ? Eye : EyeOff" :size="15" />
        </IconButton>
        <IconButton :selected="follow" :title="t('fm.follow')" @click="toggleFollow"><LocateFixed :size="15" /></IconButton>
        <IconButton :selected="app.fileTree.show" :title="t('fm.tree')" @click="app.fileTree.show = !app.fileTree.show"><PanelLeft :size="15" /></IconButton>
      </div>
    </div>

    <div ref="body" class="flex min-h-0 flex-1">
      <!-- 用隐藏而非移除，保留目录树状态。空间不足时优先收缩目录树。 -->
      <div v-show="app.fileTree.show" :class="[line, 'relative min-w-0 border-r', well]" :style="{ width: `${treeWidth}px` }">
        <DirTree ref="tree" :current="path" :pending="pendingDir" :show-hidden="app.settings.showHidden" :load="loadDir" @open="navigate($event)" @menu="openTreeMenu" />
        <div
          class="absolute inset-y-0 -right-0.5 z-10 w-1 cursor-col-resize transition-colors duration-150 hover:bg-blue-500 active:bg-blue-500"
          :title="t('fm.treeResize')"
          @mousedown="resizeTree"
          @dblclick="app.fileTree.width = FILE_TREE_WIDTH.initial"
        />
      </div>

      <div
        class="relative flex min-w-0 flex-1 flex-col"
        :class="[app.fileTree.show && 'min-w-123 shrink-0', dropping && 'outline-2 -outline-offset-4 outline-blue-500 outline-dashed']"
      >
        <div v-if="found" class="flex items-center justify-between bg-blue-500/12 py-1 pr-2 pl-3 dark:bg-blue-400/15">
          <span>{{ t("fm.searchResults", { query: foundFor, count: found.length }) }}{{ found.length >= 500 ? t("fm.searchLimited") : "" }}</span>
          <UiButton ghost size="tiny" @click="found = null">
            <template #icon><X :size="13" /></template>
            {{ t("fm.exitSearch") }}
          </UiButton>
        </div>
        <!-- 表头随列表横向平移（scrollLeft）。 -->
        <div :class="[line, 'shrink-0 overflow-hidden border-b']">
          <div :class="[cells.row, dim, 'h-7 text-xs font-medium']" :style="{ transform: `translateX(${-scrollLeft}px)` }">
            <!-- 表头不截断，排序箭头可伸入相邻列内边距。 -->
            <div
              v-for="(c, i) in COLUMNS"
              :key="c.key"
              :class="[i ? cells.rest[i - 1] : cells.name, 'cursor-pointer overflow-visible! hover:text-zinc-800 dark:hover:text-zinc-100']"
              :style="i ? undefined : nameStyle"
              @click="sortBy(c.key)"
            >
              {{ t(c.label) }}<span v-if="sortKey === c.key" :class="accentText"> {{ sortAsc ? "▲" : "▼" }}</span>
            </div>
          </div>
        </div>
        <div ref="scroller" class="relative min-h-0 flex-1 overflow-auto outline-none" tabindex="0" @scroll="onScroll" @keydown="onKey" @contextmenu="openMenu($event, null)" @click.self="clearSelection">
          <div class="relative min-h-full" :class="compact ? 'min-w-120.5' : 'min-w-138'" :style="{ height: `${lead + rows.length * ROW_HEIGHT}px` }" @click.self="clearSelection">
            <div v-if="lead" :class="[cells.row, rowHover, 'absolute inset-x-0 top-0 h-6.5']" :title="t('fm.up')" @click="clearSelection" @dblclick="navigate(parentPath(path))">
              <div :class="cells.nameCell" :style="nameStyle">
                <FileIcon name=".." is-dir kind="dir" :size="15" class="shrink-0" />
                <span>..</span>
              </div>
            </div>
            <div
              v-for="(e, i) in visible"
              :key="e.path"
              :class="[
                cells.row,
                'absolute inset-x-0 h-6.5',
                selected.has(e.path) ? selectedFill : rowHover,
                clipboard?.cut && clipboard.paths.includes(e.path) && 'opacity-45',
              ]"
              :style="{ top: `${lead + (windowStart + i) * ROW_HEIGHT}px` }"
              @click="clickRow($event, windowStart + i)"
              @dblclick="open(e)"
              @contextmenu.stop="openMenu($event, windowStart + i)"
            >
              <div :class="cells.nameCell" :style="nameStyle" :title="e.linkTarget ? `${e.name} → ${e.linkTarget}` : e.name">
                <FileIcon :name="e.name" :is-dir="e.isDir" :kind="e.kind" :size="15" :text="editable(e.name)" class="shrink-0" />
                <span class="truncate">{{ e.name }}</span>
                <span v-if="e.linkTarget" :class="[dim, 'flex-1 truncate']">→ {{ e.linkTarget }}</span>
              </div>
              <!-- 等宽字体便于上下对齐。 -->
              <div :class="cells.size">
                <span v-if="!e.isDir" class="font-mono text-xs">{{ formatSize(e.size) }}</span>
                <span v-else-if="typeof dirSizes.get(e.path) === 'number'" class="font-mono text-xs">{{ formatSize(dirSizes.get(e.path) as number) }}</span>
                <UiSpin v-else-if="dirSizes.has(e.path)" :size="12" class="ml-auto" />
                <UiButton v-else ghost size="tiny" @click.stop="measure(e)" @dblclick.stop>{{ t("props.calculate") }}</UiButton>
              </div>
              <div :class="cells.time" :title="compact ? formatTime(e.mtime) : undefined">{{ compact ? formatShortTime(e.mtime) : formatTime(e.mtime) }}</div>
              <div :class="cells.perm" :title="formatMode(e.mode, e.kind)">{{ formatOctal(e.mode) }}</div>
              <div :class="cells.owner" :title="ownerText(e)">{{ compact ? `${e.owner ?? e.uid ?? ""}` : ownerText(e) }}</div>
            </div>
          </div>
          <div v-if="!rows.length && !loading" :class="[dim, 'pointer-events-none absolute inset-x-0 top-10 text-center']">{{ t(found ? "fm.noFound" : filter ? "fm.noMatch" : "fm.emptyFolder") }}</div>
        </div>
        <div v-if="loading || busy" class="absolute top-9 right-4"><UiSpin :size="20" /></div>
        <div v-if="dropping" :class="[accentText, 'pointer-events-none absolute inset-x-0 bottom-9 text-center font-medium']">{{ t("fm.dropHint", { path }) }}</div>
        <div :class="[line, dim, 'flex h-6 shrink-0 items-center gap-4 border-t px-3 text-xs']">
          <span>{{ t("fm.count", { count: rows.length }) }}</span>
          <span v-if="selection.length">{{ t("fm.selected", { count: selection.length, size: formatSize(selection.reduce((n, e) => n + Math.max(0, sizeOf(e)), 0)) }) }}</span>
          <span v-if="clipboard">{{ t(clipboard.cut ? "fm.clipCut" : "fm.clipCopy", { count: clipboard.paths.length }) }}</span>
        </div>
      </div>
    </div>

    <EditorModal v-if="editing" :session-id="sessionId" :path="editing" @close="editing = null" @saved="refresh" />
    <ArchiveDialog ref="archiveDialog" />
    <PropsDialog v-model:show="showProps" :session-id="sessionId" :items="propsItems" @changed="refresh" />
  </div>
</template>
