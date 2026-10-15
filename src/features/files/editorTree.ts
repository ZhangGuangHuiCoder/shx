// 编辑器文件栏的树模型与标签切换规则（纯函数）。
import { compareNames } from "@/features/files/dirTree";
import type { FileEntry } from "@/shared/api";

/** 已按 sortEntries 排序；未列出时 entries 为 null。 */
export interface Listing {
  entries: FileEntry[] | null;
  error?: string;
}

export interface TreeRow {
  entry: FileEntry;
  depth: number;
}

/** 文件夹在前，同类按自然排序。 */
export function sortEntries(entries: FileEntry[]): FileEntry[] {
  return [...entries].sort((a, b) => Number(b.isDir) - Number(a.isDir) || compareNames(a.name, b.name));
}

/** 生成可见行。有 filter 时保留名称匹配的条目，以及包含匹配项的文件夹。 */
export function treeRows(
  root: string,
  listings: ReadonlyMap<string, Listing>,
  expanded: ReadonlySet<string>,
  opts: { showHidden: boolean; filter?: string },
): TreeRow[] {
  const needle = opts.filter?.trim().toLowerCase() ?? "";
  const matches = (entry: FileEntry) => entry.name.toLowerCase().includes(needle);
  const rows: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    for (const entry of listings.get(dir)?.entries ?? []) {
      if (!opts.showHidden && entry.name.startsWith(".")) continue;
      const at = rows.push({ entry, depth });
      if (entry.isDir && expanded.has(entry.path)) walk(entry.path, depth + 1);
      // 自身不匹配且无子项留下时撤销。
      if (needle && !matches(entry) && rows.length === at) rows.pop();
    }
  };
  walk(root, 0);
  return rows;
}

/** 关闭标签后的活动标签：优先右侧，其次左侧。 */
export function nextActive(paths: readonly string[], closing: string, active: string | null): string | null {
  if (closing !== active) return active;
  const i = paths.indexOf(closing);
  const rest = paths.filter((p) => p !== closing);
  return rest.length ? rest[Math.min(Math.max(i, 0), rest.length - 1)] : null;
}
