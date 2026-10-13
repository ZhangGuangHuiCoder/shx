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
import { NButton, NInput, NSpin } from "naive-ui";
import { computed, h, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api, errorMessage, type Conflict, type FileEntry, type Transfer } from "../../shared/api";
import FileIcon from "../../shared/components/FileIcon.vue";
import IconButton from "../../shared/components/IconButton.vue";
import { openContextMenu, shortcut, type MenuEntry } from "../../shared/contextMenu";
import { t, type MessageKey } from "../../shared/i18n";
import { choose, confirm, message, prompt } from "../../shared/ui";
import { baseName, formatMode, formatOctal, formatSize, formatTime, isArchive, joinPath, parentPath, ruleMatcher, shellQuote } from "../../shared/util";
import { FILE_TREE_WIDTH, useApp } from "../../stores/app";
import ArchiveDialog from "./ArchiveDialog.vue";
import { compareNames, type DirNode } from "./dirTree";
import DirTree from "./DirTree.vue";
import EditorModal from "./EditorModal.vue";
import PropsDialog from "./PropsDialog.vue";

const props = defineProps<{ sessionId: string; /** 会话所连的主机：收藏的目录按主机保存。 */ profileId: string; active: boolean }>();
const emit = defineEmits<{ send: [data: string] }>();
const app = useApp();

const ROW_HEIGHT = 26;

/** 文件名是否匹配“在编辑器中打开”的规则。 */
const editable = computed(() => ruleMatcher(app.settings.editRules));

// ---- 目录状态 --------------------------------------------------------------

const path = ref("");
const entries = ref<FileEntry[]>([]);
const loading = ref(false);
const back = ref<string[]>([]);
const forward = ref<string[]>([]);
const filter = ref("");
/** 不为 null 时显示的是搜索结果，而不是目录内容。 */
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

/**
 * 文件夹的大小，按需计算：路径 → 字节数；服务器还在统计时为 null。
 * 列表切换到别的文件夹之前一直保留。
 */
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

/** 条目目前已知的大小：文件夹在统计出来之前不计大小。 */
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
  // 文件夹始终排在前面；相等时按名称排，保证顺序稳定。
  return list.sort((a, b) => Number(b.isDir) - Number(a.isDir) || dir * (by[sortKey.value](a, b) || by.name(a, b)));
});

function sortBy(key: SortKey) {
  if (sortKey.value === key) sortAsc.value = !sortAsc.value;
  else [sortKey.value, sortAsc.value] = [key, true];
}

/** `navigate` 的列表还没返回时，它正要前往的位置。 */
const pendingDir = ref<string | null>(null);
let navigation = 0;

async function navigate(target: string, record = true) {
  // 只认最新的请求：点击文件夹的速度快过服务器响应时，
  // 不能停在碰巧最后到达的那份列表上。
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

/** 重新列出当前目录，保留选中项和滚动位置。 */
async function reload() {
  const [session, dir] = [props.sessionId, path.value];
  const listing = await api.list(session, dir);
  // 等待期间列表去了别的文件夹（或换了连接）：这份结果已经不是当前显示的内容。
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

// 已重连：视图在新会话上接着显示原来的文件夹，历史、选中项和目录树都保持原样。
// 只有那个文件夹已经不存在时，才从主目录重新开始。
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

// ---- 地址栏 ----------------------------------------------------------------

const editingPath = ref(false);
const pathText = ref("");
const pathInput = ref<InstanceType<typeof NInput>>();

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

// ---- 收藏 ------------------------------------------------------------------

/** 这台主机上收藏的目录，按收藏的先后排列。 */
const favorites = computed(() => app.profiles.find((p) => p.id === props.profileId)?.favoriteDirs ?? []);
const isFavorite = (dir: string) => favorites.value.includes(dir);

function toggleFavorite(dir: string) {
  // 第一份列表还没回来时，还不知道当前目录是哪个。
  if (!dir) return;
  const was = isFavorite(dir);
  app.toggleFavoriteDir(props.profileId, dir);
  // 主机在标签页开着时被删掉了的话，收藏无处可存。
  if (isFavorite(dir) !== was) message.success(t(was ? "fm.unfavorited" : "fm.favorited"));
}

/** 右键菜单里收藏或取消收藏 `dir` 的那一项；`current` 表示它是正在显示的目录。 */
function favoriteEntry(dir: string, current = false): MenuEntry {
  const on = isFavorite(dir);
  return {
    label: t(`fm.${on ? "unfavorite" : "favorite"}${current ? "Current" : ""}`),
    icon: on ? StarOff : Star,
    hint: current ? shortcut.mod("D") : undefined,
    action: () => toggleFavorite(dir),
  };
}

/** 工具栏上的收藏夹：列出收藏的目录，点一下就过去。 */
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

// ---- 目录树 ----------------------------------------------------------------

const tree = ref<InstanceType<typeof DirTree>>();
const body = ref<HTMLElement>();
let stopTreeResize: (() => void) | undefined;

const loadDir = (dir: string) => api.list(props.sessionId, dir).then((listing) => listing.entries);

/** 正在拖动目录树的边缘时，拖到的宽度。松手才记进偏好：偏好每改一次都要存盘。 */
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

// ---- 选择 ------------------------------------------------------------------

const selected = ref(new Set<string>());
let anchor = -1;
const scroller = ref<HTMLElement>();
const scrollTop = ref(0);
/** 面板窄到放不下所有列时，列表横向滚动了多少；表头跟着平移同样的距离。 */
const scrollLeft = ref(0);
const viewHeight = ref(300);

function onScroll() {
  scrollTop.value = scroller.value!.scrollTop;
  scrollLeft.value = scroller.value!.scrollLeft;
}

/**
 * 列表顶部的 ".." 一行（回上级目录）占的高度；根目录和搜索结果里没有这一行。
 * 它不是列表里的条目：不能选中，不计入项数，其余各行只是整体下移一行。
 */
const lead = computed(() => (path.value && path.value !== "/" && found.value === null ? ROW_HEIGHT : 0));

const selection = computed(() => rows.value.filter((e) => selected.value.has(e.path)));
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
  // 回到第一项时一直滚到顶，把 ".." 也露出来。
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

// ---- 操作 ------------------------------------------------------------------

const busy = ref(false);
const showProps = ref(false);
const propsItems = ref<FileEntry[]>([]);
const clipboard = ref<{ cut: boolean; paths: string[] } | null>(null);
let lastDownloadDir: string | null = null;

/** 执行一个修改操作；无论成功与否，之后都重新加载列表。 */
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

/** 编辑器中打开的文件的路径（如果有）。 */
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

/** 询问下载到本机的哪个位置；关闭时返回 null。 */
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

/** 用户没有另外指定时，这些 `dir` 下的条目打成的压缩包叫什么。 */
const archiveStem = (dir: string, names: string[]) => (names.length === 1 ? names[0] : baseName(dir) || "archive");

/**
 * 把一个文件夹下的条目打成一个压缩包下载：传输的是一个文件，而不是成千上万个。
 * 在服务器上打包是传输任务的第一阶段，从这时起它就显示在传输列表里。
 */
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

/**
 * 检查为当前文件夹的新条目或重命名输入的名称：返回问题所在，没有问题时返回 null。
 * `own` 是条目现有的名字，允许保持不变。
 */
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
  // 目录树里可能正显示着刚有条目移入或移出的文件夹。
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

// ---- 跟随终端的工作目录 ----------------------------------------------------

const follow = ref(false);
let injected = false;

// 让 bash 和 zsh 在每次显示提示符前报告工作目录（OSC 7）。在 tmux（保持会话）里，
// 序列要再包一层，tmux 才会把它转交给终端，而不是自己吞掉。
// 开头的空格在多数配置下能让这一行不进入 shell 历史。
const CWD_HOOK = String.raw` __shx_cwd() { if [ -n "$TMUX" ]; then printf '\033Ptmux;\033\033]7;file://%s%s\033\033\\\033\\' "${"$"}{HOST:-$HOSTNAME}" "$PWD"; else printf '\033]7;file://%s%s\033\\' "${"$"}{HOST:-$HOSTNAME}" "$PWD"; fi; }; if [ -n "$ZSH_VERSION" ]; then precmd_functions+=(__shx_cwd); elif [ -n "$BASH_VERSION" ]; then PROMPT_COMMAND="__shx_cwd${"$"}{PROMPT_COMMAND:+;$PROMPT_COMMAND}"; fi`;

/** 钩子最近一次报告的终端工作目录。 */
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

/**
 * 重连后启动的是新 shell，它不知道钩子的存在。处于跟随状态时，重新安装钩子，并让
 * shell 回到之前所在的目录。清屏用温和的方式（不用 `clear`，它会连回滚缓冲区一起
 * 清掉）：屏幕上的内容上滚进入历史，断开之前的输出仍然可以查看。
 */
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

// ---- 右键菜单 --------------------------------------------------------------

function openMenu(e: MouseEvent, index: number | null) {
  if (index !== null && !selected.value.has(rows.value[index].path)) selectOnly(index);
  if (index === null) clearSelection();
  const items = selection.value;
  const one = items.length === 1 ? items[0] : null;
  const searching = found.value !== null;
  const paths = items.map((item) => item.path);
  const clip = clipboard.value;
  /** 保留条件成立的菜单项；不带条件的菜单项始终保留。 */
  const when = (...list: (MenuEntry | false | null | undefined)[]) => list.filter((x): x is MenuEntry => !!x);

  const size = items.reduce((n, item) => n + Math.max(0, sizeOf(item)), 0);
  const entries: MenuEntry[] = items.length
    ? when(
        // 下面的命令所作用的对象：选中多项时很容易搞不清。
        one
          ? {
              heading: one.name,
              detail: one.isDir ? t(`kind.${one.kind === "link" ? "link" : "dir"}`) : formatSize(one.size),
              icon: () => h(FileIcon, { name: one.name, isDir: one.isDir, kind: one.kind, size: 18, text: editable.value(one.name) }),
            }
          : { heading: t("fm.menuSelected", { count: items.length }), detail: size ? formatSize(size) : undefined, icon: () => h(Files, { size: 18, style: "color: var(--text-dim)" }) },
        one && { label: t(one.isDir ? "fm.open" : "fm.openByRule"), icon: FolderOpen, hint: shortcut.enter, action: () => open(one) },
        one && !one.isDir && { label: t("common.edit"), icon: SquarePen, action: () => edit(one) },
        one && searching && { label: t("fm.reveal"), icon: FolderSearch, action: () => navigate(parentPath(one.path)) },
        { label: t("fm.download"), icon: Download, action: () => download(items) },
        // 针对文件夹：一个压缩包比一大堆小文件传得快得多。
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

// ---- 事件接线 --------------------------------------------------------------

const dropping = ref(false);
const unlisten: (() => void)[] = [];
let unmounted = false;
let observer: ResizeObserver | undefined;
let refreshTimer: ReturnType<typeof setTimeout> | undefined;

/** 登记一个事件监听；注册完成时组件已经卸载的，立刻注销。 */
async function subscribe(registering: Promise<() => void>) {
  const off = await registering;
  if (unmounted) off();
  else unlisten.push(off);
}

onMounted(() => {
  observer = new ResizeObserver(([box]) => (viewHeight.value = box.contentRect.height));
  observer.observe(scroller.value!);
  void navigate(".");

  // 从系统拖入的文件上传到当前显示的目录。
  const drops = getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (!props.active || editing.value) return void (dropping.value = false);
    dropping.value = payload.type === "enter" || payload.type === "over";
    if (payload.type === "drop") void upload(payload.paths);
  });
  const transfers = listen<Transfer>("transfer-update", ({ payload: t }) => {
    const here = t.sessionId === props.sessionId && t.direction === "upload" && t.destDir === path.value;
    if (!here || (t.state !== "done" && t.state !== "running")) return;
    // 合并刷新：往同一个文件夹上传很多文件时，不应每个文件都刷新一次。
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
  <div class="fm" :class="{ side: app.settings.fileLayout === 'right' }">
    <div class="toolbar">
      <IconButton :title="t('fm.back')" :disabled="!back.length" @click="goBack"><ArrowLeft :size="15" /></IconButton>
      <IconButton :title="t('fm.forward')" :disabled="!forward.length" @click="goForward"><ArrowRight :size="15" /></IconButton>
      <IconButton :title="t('fm.up')" :disabled="path === '/'" @click="navigate(parentPath(path))"><ArrowUp :size="15" /></IconButton>
      <IconButton :title="t('fm.home')" @click="navigate('.')"><House :size="15" /></IconButton>
      <IconButton :title="t('common.refresh')" @click="refresh"><RefreshCw :size="14" /></IconButton>
      <IconButton :type="isFavorite(path) ? 'primary' : 'default'" :title="t('fm.favorites')" @click="openFavorites">
        <Star :size="15" :fill="isFavorite(path) ? 'currentColor' : 'none'" />
      </IconButton>

      <div class="address" @click.self="editPath">
        <NInput
          v-if="editingPath"
          ref="pathInput"
          v-model:value="pathText"
          size="tiny"
          @keydown.enter="submitPath"
          @keydown.esc="editingPath = false"
          @blur="editingPath = false"
        />
        <template v-else>
          <span class="crumb" @click="navigate('/')">/</span>
          <template v-for="(c, i) in crumbs" :key="c.path">
            <ChevronRight v-if="i > 0" :size="12" class="dim" />
            <span class="crumb ellipsis" @click="navigate(c.path)">{{ c.name }}</span>
          </template>
          <span class="grow" :title="t('fm.editPath')" @click="editPath" />
        </template>
      </div>

      <NInput v-model:value="filter" size="tiny" :placeholder="t('fm.filter')" clearable class="filter" />
      <IconButton :title="t('fm.upload')" @click="pickUpload(false)"><Upload :size="15" /></IconButton>
      <IconButton :title="t('fm.downloadSelected')" :disabled="!selection.length" @click="download(selection)"><Download :size="15" /></IconButton>
      <IconButton :title="t('fm.newFile')" @click="create('file')"><FilePlus :size="15" /></IconButton>
      <IconButton :title="t('fm.newFolder')" @click="create('dir')"><FolderPlus :size="15" /></IconButton>
      <IconButton :title="t('fm.deleteSelected')" :disabled="!selection.length" @click="remove(selection)"><Trash2 :size="15" /></IconButton>
      <IconButton :title="t('fm.search')" @click="search"><Search :size="15" /></IconButton>
      <IconButton :type="app.settings.showHidden ? 'primary' : 'default'" :title="t('menu.showHidden')" @click="app.settings.showHidden = !app.settings.showHidden">
        <component :is="app.settings.showHidden ? Eye : EyeOff" :size="15" />
      </IconButton>
      <IconButton :type="follow ? 'primary' : 'default'" :title="t('fm.follow')" @click="toggleFollow"><LocateFixed :size="15" /></IconButton>
      <IconButton :type="app.fileTree.show ? 'primary' : 'default'" :title="t('fm.tree')" @click="app.fileTree.show = !app.fileTree.show"><PanelLeft :size="15" /></IconButton>
    </div>

    <div ref="body" class="body">
      <!-- 隐藏而不是移除，目录树已加载和已展开的内容得以保留。 -->
      <div v-show="app.fileTree.show" class="tree" :style="{ width: `${treeWidth}px` }">
        <DirTree
          ref="tree"
          :current="path"
          :pending="pendingDir"
          :show-hidden="app.settings.showHidden"
          :load="loadDir"
          @open="navigate($event)"
          @menu="openTreeMenu"
        />
        <div class="tree-resizer" :title="t('fm.treeResize')" @mousedown="resizeTree" @dblclick="app.fileTree.width = FILE_TREE_WIDTH.initial" />
      </div>

      <div class="table" :class="{ dropping }">
        <div v-if="found" class="banner">
          <span>{{ t("fm.searchResults", { query: foundFor, count: found.length }) }}{{ found.length >= 500 ? t("fm.searchLimited") : "" }}</span>
          <NButton quaternary type="tertiary" size="tiny" @click="found = null">
            <template #icon><X :size="13" /></template>
            {{ t("fm.exitSearch") }}
          </NButton>
        </div>
        <div class="head">
          <div class="grid header" :style="{ transform: `translateX(${-scrollLeft}px)` }">
            <div v-for="c in COLUMNS" :key="c.key" class="cell" @click="sortBy(c.key)">
              {{ t(c.label) }}<span v-if="sortKey === c.key" class="dim"> {{ sortAsc ? "▲" : "▼" }}</span>
            </div>
          </div>
        </div>
        <div ref="scroller" class="rows" tabindex="0" @scroll="onScroll" @keydown="onKey" @contextmenu="openMenu($event, null)" @click.self="clearSelection">
          <div class="spacer" :style="{ height: `${lead + rows.length * ROW_HEIGHT}px` }" @click.self="clearSelection">
            <div v-if="lead" class="grid row parent" :title="t('fm.up')" @click="clearSelection" @dblclick="navigate(parentPath(path))">
              <div class="cell name">
                <FileIcon name=".." is-dir kind="dir" :size="15" />
                <span>..</span>
              </div>
            </div>
            <div
              v-for="(e, i) in visible"
              :key="e.path"
              class="grid row"
              :class="{ selected: selected.has(e.path), cut: clipboard?.cut && clipboard.paths.includes(e.path) }"
              :style="{ top: `${lead + (windowStart + i) * ROW_HEIGHT}px` }"
              @click="clickRow($event, windowStart + i)"
              @dblclick="open(e)"
              @contextmenu.stop="openMenu($event, windowStart + i)"
            >
              <div class="cell name" :title="e.linkTarget ? `${e.name} → ${e.linkTarget}` : e.name">
                <FileIcon
                  :name="e.name"
                  :is-dir="e.isDir"
                  :kind="e.kind"
                  :size="15"
                  :text="editable(e.name)"
                />
                <span class="ellipsis">{{ e.name }}</span>
                <span v-if="e.linkTarget" class="ellipsis dim">→ {{ e.linkTarget }}</span>
              </div>
              <div class="cell num">
                <span v-if="!e.isDir" class="mono">{{ formatSize(e.size) }}</span>
                <span v-else-if="typeof dirSizes.get(e.path) === 'number'" class="mono">{{ formatSize(dirSizes.get(e.path) as number) }}</span>
                <NSpin v-else-if="dirSizes.has(e.path)" :size="12" />
                <NButton v-else text size="tiny" @click.stop="measure(e)" @dblclick.stop>{{ t("props.calculate") }}</NButton>
              </div>
              <div class="cell mono">{{ formatTime(e.mtime) }}</div>
              <div class="cell mono" :title="formatMode(e.mode, e.kind)">{{ formatOctal(e.mode) }}</div>
              <div class="cell mono ellipsis" :title="ownerText(e)">{{ ownerText(e) }}</div>
            </div>
          </div>
          <div v-if="!rows.length && !loading" class="none dim">{{ t(found ? "fm.noFound" : filter ? "fm.noMatch" : "fm.emptyFolder") }}</div>
        </div>
        <div v-if="loading || busy" class="loading"><NSpin :size="20" /></div>
        <div v-if="dropping" class="drop-hint">{{ t("fm.dropHint", { path }) }}</div>
        <div class="statusbar dim">
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

<style scoped>
.fm {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg);
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 34px;
  padding: 0 6px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--border);
  background: var(--bg-raised);
}

.address {
  flex: 1;
  min-width: 80px;
  display: flex;
  align-items: center;
  height: 24px;
  margin: 0 6px;
  padding: 0 4px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--bg);
  overflow: hidden;
}

.crumb {
  flex-shrink: 1;
  min-width: 14px;
  padding: 1px 4px;
  border-radius: 4px;
  cursor: pointer;
}

.crumb:hover {
  background: var(--bg-hover);
}

.grow {
  flex: 1;
  align-self: stretch;
  min-width: 20px;
  cursor: text;
}

.filter {
  width: 120px;
  margin-right: 4px;
}

.body {
  flex: 1;
  min-height: 0;
  display: flex;
}

.tree {
  position: relative;
  flex-shrink: 0;
  /* 无论拖到多宽，都给文件列表留够显示全部列的宽度（session/layout.ts 的 FILE_LIST_MIN_WIDTH）。 */
  max-width: calc(100% - 610px);
  border-right: 1px solid var(--border);
}

.tree-resizer {
  position: absolute;
  inset: 0 -3px 0 auto;
  z-index: 1;
  width: 5px;
  cursor: col-resize;
  transition: background 0.12s;
}

.tree-resizer:hover,
.tree-resizer:active {
  background: var(--accent);
}

.table {
  position: relative;
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.table.dropping {
  outline: 2px dashed var(--accent);
  outline-offset: -3px;
}

.banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 6px 2px 10px;
  background: var(--bg-active);
}

.grid {
  display: grid;
  /* 后四列按内容定宽：大小一列放得下英文的“Calculate”，时间是 16 个字符，权限四位数字，所有者到 `www-data:www-data`。 */
  grid-template-columns: minmax(178px, 1fr) 82px 136px 64px 140px;
  align-items: center;
}

/* 所有列始终都在。窗口的最小宽度保证了列表放得下它们（见 session/layout.ts）；
   万一还是比各列最小宽度之和（上面几个数相加）窄，列表横向滚动。 */
.grid,
.spacer {
  min-width: 600px;
}

.cell {
  padding: 0 8px;
  overflow: hidden;
  white-space: nowrap;
}

/* 表头不在滚动区域里，横向跟着列表平移（见 `scrollLeft`）。 */
.head {
  flex-shrink: 0;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
}

.header {
  height: 24px;
  color: var(--text-dim);
  font-size: 12px;
}

.header .cell {
  cursor: pointer;
}

.header .cell:hover {
  color: var(--text);
}

.rows {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: auto;
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
  height: 26px;
}

.row:hover {
  background: var(--bg-hover);
}

.row.selected {
  background: var(--bg-active);
}

.row.cut {
  opacity: 0.45;
}

.row.parent {
  top: 0;
}

.name {
  display: flex;
  align-items: center;
  gap: 6px;
}

.name svg {
  flex-shrink: 0;
}

.name .dim {
  flex-shrink: 2;
}

.num {
  text-align: right;
}

/* 名称之外的几列用等宽字体：每一行一样长，上下对得齐。 */
.mono {
  font-family: var(--font-mono);
  font-size: 12px;
}

.none {
  position: absolute;
  inset: 40px 0 auto;
  text-align: center;
  pointer-events: none;
}

.loading {
  position: absolute;
  top: 34px;
  right: 16px;
}

.drop-hint {
  position: absolute;
  inset: auto 0 34px;
  text-align: center;
  color: var(--accent);
  pointer-events: none;
}

.statusbar {
  display: flex;
  gap: 16px;
  height: 22px;
  padding: 0 10px;
  flex-shrink: 0;
  line-height: 22px;
  font-size: 11px;
  border-top: 1px solid var(--border);
}

/* 位于终端右侧时：工具栏换行，地址栏独占一行、排在按钮下面。 */
.side .toolbar {
  flex-wrap: wrap;
  /* 按钮仍需第二行时，那一行同样两端对齐。 */
  justify-content: space-between;
  height: auto;
  padding: 4px 6px;
}

/* 地址栏独占一行后，筛选框占满剩余空间，
   导航按钮靠左，操作按钮靠右。 */
.side .filter {
  flex: 1 1 120px;
  width: auto;
  min-width: 90px;
}

.side .address {
  order: 1;
  flex-basis: 100%;
  margin: 4px 0 0;
}
</style>
