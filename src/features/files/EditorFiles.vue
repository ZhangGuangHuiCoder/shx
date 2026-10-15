<script setup lang="ts">
// 编辑器左侧文件栏。
import { ArrowUp, ChevronRight, ChevronsDownUp, Eye, EyeOff, FilePlus, FolderOpen, FolderPlus, LoaderCircle, RotateCw, Search } from "@lucide/vue";
import { computed, nextTick, reactive, ref, shallowReactive, watch } from "vue";
import { pathChain } from "@/features/files/dirTree";
import { sortEntries, treeRows, type Listing } from "@/features/files/editorTree";
import { api, errorMessage, type FileEntry } from "@/shared/api";
import { t } from "@/shared/i18n";
import { useApp } from "@/shared/store";
import { message, prompt } from "@/shared/ui";
import { joinPath, parentPath, ruleMatcher } from "@/shared/util";
import FileIcon from "@/shared/ui-kit/FileIcon.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import { dangerText, dim, field, fieldInput, line, lineFill, toolGroup, well } from "@/shared/ui-kit/styles";

const props = defineProps<{ sessionId: string; /** 高亮的当前文件。 */ active: string | null }>();
const root = defineModel<string>("root", { required: true });
const emit = defineEmits<{ open: [path: string] }>();
const app = useApp();
const editable = computed(() => ruleMatcher(app.settings.editRules));

const INDENT = 12;
const listings = shallowReactive(new Map<string, Listing>());
const expanded = reactive(new Set<string>());
const loading = reactive(new Set<string>());
const filter = ref("");
/** 初值取自文件管理器设置，切换只影响编辑器。 */
const showHidden = ref(app.settings.showHidden);

const rows = computed(() => treeRows(root.value, listings, expanded, { showHidden: showHidden.value, filter: filter.value }));
const rootListing = computed(() => listings.get(root.value));

async function load(dir: string) {
  loading.add(dir);
  try {
    const { entries } = await api.list(props.sessionId, dir);
    listings.set(dir, { entries: sortEntries(entries) });
  } catch (e) {
    listings.set(dir, { entries: listings.get(dir)?.entries ?? [], error: errorMessage(e) });
  } finally {
    loading.delete(dir);
  }
}

function reload() {
  for (const dir of [root.value, ...expanded]) void load(dir);
}

async function toggle(entry: FileEntry) {
  if (expanded.delete(entry.path)) return;
  expanded.add(entry.path);
  if (!listings.has(entry.path)) await load(entry.path);
}

function click(entry: FileEntry) {
  if (entry.isDir) void toggle(entry);
  else emit("open", entry.path);
}

/** 双击文件夹设为根。 */
function enter(entry: FileEntry) {
  if (entry.isDir) root.value = entry.path;
}

/** 在根目录新建文件（并打开）或文件夹（并展开）。 */
async function create(kind: "file" | "dir") {
  const dir = root.value;
  const taken = new Set((listings.get(dir)?.entries ?? []).map((e) => e.name));
  const answer = await prompt({
    title: t(kind === "dir" ? "fm.newFolder" : "fm.newFile"),
    detail: t("fm.inFolder", { path: dir }),
    icon: kind === "dir" ? FolderPlus : FilePlus,
    placeholder: t("common.name"),
    validate: (value: string) => (value.includes("/") ? t("fm.noSlash") : taken.has(value.trim()) ? t("fm.nameTaken") : null),
  });
  const name = answer?.value.trim();
  if (!name) return;
  const path = joinPath(dir, name);
  try {
    await (kind === "dir" ? api.mkdir(props.sessionId, path) : api.createFile(props.sessionId, path));
  } catch (e) {
    return void message.error(errorMessage(e));
  }
  await load(dir);
  if (kind === "file") return void emit("open", path);
  // 新文件夹为空，直接展开。
  listings.set(path, { entries: [] });
  expanded.add(path);
}

/** 路径栏：面包屑，点击空白处可编辑路径。 */
const crumbs = computed(() => pathChain(root.value).map((path) => ({ path, name: path === "/" ? "/" : path.slice(path.lastIndexOf("/") + 1) })));
const crumbBar = ref<HTMLElement>();
const editingPath = ref(false);
const pathText = ref("");
const pathInput = ref<HTMLInputElement>();

async function editPath() {
  pathText.value = root.value;
  editingPath.value = true;
  await nextTick();
  pathInput.value?.select();
}

function commitPath() {
  if (!editingPath.value) return;
  editingPath.value = false;
  const next = pathText.value.trim().replace(/\/+$/, "") || "/";
  if (next.startsWith("/") && next !== root.value) root.value = next;
}

// 路径过长时滚到最右，露出当前文件夹。
watch([root, editingPath], async () => {
  await nextTick();
  crumbBar.value?.scrollTo({ left: crumbBar.value.scrollWidth });
});

watch(root, (dir) => {
  filter.value = "";
  if (!listings.has(dir)) void load(dir);
}, { immediate: true });
</script>

<template>
  <div :class="[well, 'flex h-full min-w-0 flex-col']">
    <div :class="[line, 'flex shrink-0 flex-col gap-2 border-b p-2']">
      <div :class="[field, 'h-8 items-center gap-1.5 rounded-xl px-2.5 text-xs']" :title="editingPath ? undefined : t('fm.editPath')" @click="editingPath || editPath()">
        <FolderOpen :size="14" class="shrink-0 text-blue-500" />
        <input
          v-if="editingPath"
          ref="pathInput"
          v-model="pathText"
          :class="[fieldInput, 'font-mono text-xs']"
          spellcheck="false"
          @keydown.enter.prevent="commitPath"
          @keydown.esc.prevent="editingPath = false"
          @blur="commitPath"
        />
        <div v-else ref="crumbBar" class="flex min-w-0 flex-1 items-center overflow-x-auto font-mono" data-no-scrollbar>
          <template v-for="(crumb, i) in crumbs" :key="crumb.path">
            <ChevronRight v-if="i > 1" :size="11" :class="[dim, 'shrink-0']" />
            <button
              type="button"
              class="shrink-0 cursor-pointer rounded px-1 py-0.5 whitespace-nowrap hover:bg-zinc-900/6 dark:hover:bg-white/10"
              :class="i === crumbs.length - 1 ? 'font-semibold text-zinc-900 dark:text-white' : dim"
              @click.stop="root = crumb.path"
            >{{ crumb.name }}</button>
          </template>
        </div>
      </div>
      <!-- 变窄时只收缩筛选框。 -->
      <div class="flex items-center gap-1.5">
        <UiInput v-model:value="filter" size="small" round clearable :placeholder="t('editor.filter')" class="h-7.5 min-w-0 flex-1">
          <template #prefix><Search :size="13" :class="dim" /></template>
        </UiInput>
        <div :class="toolGroup">
          <IconButton size="tiny" :title="t('fm.up')" :disabled="root === '/'" @click="root = parentPath(root)"><ArrowUp :size="13" /></IconButton>
          <IconButton size="tiny" :title="t('common.refresh')" @click="reload"><RotateCw :size="13" :class="{ 'animate-spin': loading.size > 0 }" /></IconButton>
          <IconButton size="tiny" :title="t('fm.newFile')" @click="create('file')"><FilePlus :size="13" /></IconButton>
          <IconButton size="tiny" :title="t('fm.newFolder')" @click="create('dir')"><FolderPlus :size="13" /></IconButton>
          <IconButton size="tiny" :title="t('menu.showHidden')" :selected="showHidden" @click="showHidden = !showHidden"><component :is="showHidden ? Eye : EyeOff" :size="13" /></IconButton>
          <IconButton size="tiny" :title="t('fm.treeCollapse')" :disabled="!expanded.size" @click="expanded.clear()"><ChevronsDownUp :size="13" /></IconButton>
        </div>
      </div>
    </div>
    <div class="group/rows relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-1 pb-2">
      <div v-if="loading.has(root) && !rootListing" class="grid place-items-center py-6"><LoaderCircle :size="16" :class="[dim, 'animate-spin']" /></div>
      <div v-else-if="rootListing?.error" :class="[dangerText, 'px-3 py-4 text-xs break-all']">{{ rootListing.error }}</div>
      <div v-else-if="!rows.length" :class="[dim, 'px-3 py-4 text-center text-xs']">{{ filter.trim() ? t("editor.noMatch") : t("editor.empty") }}</div>
      <div
        v-for="{ entry, depth } in rows"
        :key="entry.path"
        class="relative flex h-6 cursor-pointer items-center gap-1 rounded-md pr-2 text-sm"
        :class="entry.path === active ? 'bg-blue-500/15 font-medium text-blue-700 dark:bg-blue-400/20 dark:text-blue-200' : 'hover:bg-zinc-900/5 dark:hover:bg-white/8'"
        :style="{ paddingLeft: `${4 + depth * INDENT}px` }"
        :title="listings.get(entry.path)?.error ?? entry.path"
        @click="click(entry)"
        @dblclick="enter(entry)"
      >
        <!-- 每层一条缩进线，对齐父级箭头。 -->
        <span v-if="depth" class="pointer-events-none absolute inset-0 opacity-0 transition-opacity duration-150 group-hover/rows:opacity-100 motion-reduce:transition-none">
          <span v-for="level in depth" :key="level" :class="[lineFill, 'absolute inset-y-0 w-px']" :style="{ left: `${4 + (level - 1) * INDENT + 7}px` }" />
        </span>
        <span :class="[dim, '-mr-1 grid h-full w-4 shrink-0 place-items-center']">
          <LoaderCircle v-if="loading.has(entry.path)" :size="12" class="animate-spin" />
          <ChevronRight v-else-if="entry.isDir" :size="13" class="transition-transform duration-150 motion-reduce:transition-none" :class="{ 'rotate-90': expanded.has(entry.path) }" />
        </span>
        <FileIcon :name="entry.name" :is-dir="entry.isDir" :kind="entry.kind" :size="15" :open="expanded.has(entry.path)" :text="editable(entry.name)" class="shrink-0" />
        <span class="truncate">{{ entry.name }}</span>
      </div>
    </div>
  </div>
</template>
