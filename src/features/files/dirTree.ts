// 目录树的纯数据模型，不做 I/O。
import type { FileEntry } from "@/shared/api.ts";

export interface DirNode {
  path: string;
  name: string;
  kind: FileEntry["kind"];
  /** 子文件夹，loaded 后才完整。 */
  children: DirNode[];
  /** children 是否已列出。 */
  loaded: boolean;
  error?: string;
}

export interface DirRow {
  node: DirNode;
  depth: number;
}

const collator = new Intl.Collator(undefined, { numeric: true });

/** 自然排序（file2 在 file10 之前）。 */
export const compareNames = (a: string, b: string) => collator.compare(a, b);

export function rootNode(): DirNode {
  return { path: "/", name: "/", kind: "dir", children: [], loaded: false };
}

/** 从 / 到 path 的各级路径（含自身）。 */
export function pathChain(path: string): string[] {
  const out = ["/"];
  let at = "";
  for (const part of path.split("/")) {
    if (!part) continue;
    at += `/${part}`;
    out.push(at);
  }
  return out;
}

export function findNode(root: DirNode, path: string): DirNode | undefined {
  let node: DirNode | undefined = root;
  for (const step of pathChain(path).slice(1)) {
    node = node.children.find((child) => child.path === step);
    if (!node) return undefined;
  }
  return node;
}

/** 返回 path 的节点，途中缺失的文件夹用占位节点补齐。 */
export function ensurePath(root: DirNode, path: string): DirNode {
  let node = root;
  for (const step of pathChain(path).slice(1)) {
    let next = node.children.find((child) => child.path === step);
    if (!next) {
      next = { path: step, name: step.slice(step.lastIndexOf("/") + 1), kind: "dir", children: [], loaded: false };
      node.children.push(next);
      node.children.sort((a, b) => compareNames(a.name, b.name));
    }
    node = next;
  }
  return node;
}

/** 用新列表更新子节点，保留仍存在的节点及其已加载内容。 */
export function applyListing(node: DirNode, entries: FileEntry[]) {
  const known = new Map(node.children.map((child) => [child.path, child]));
  const prefix = node.path === "/" ? "/" : `${node.path}/`;
  node.children = entries
    .filter((e) => e.isDir)
    .map((e) => {
      // 用父路径拼接而非条目路径：符号链接目录下返回的是解析后的位置，会导致重复。
      const path = prefix + e.name;
      const child = known.get(path);
      if (!child) return { path, name: e.name, kind: e.kind, children: [], loaded: false };
      child.kind = e.kind;
      return child;
    })
    .sort((a, b) => compareNames(a.name, b.name));
  node.loaded = true;
  delete node.error;
}

const isHidden = (node: DirNode) => node.name.startsWith(".");

/** 未列出或有可见子文件夹时显示展开箭头。 */
export function expandable(node: DirNode, showHidden: boolean): boolean {
  if (!node.loaded) return true;
  return showHidden ? node.children.length > 0 : node.children.some((child) => !isHidden(child));
}

/** 生成可见行。隐藏文件夹不显示，但通往 keep（当前目录）的路径除外。 */
export function visibleRows(root: DirNode, expanded: ReadonlySet<string>, showHidden: boolean, keep = ""): DirRow[] {
  const kept = showHidden || !keep.includes("/.") ? null : new Set(pathChain(keep));
  const rows: DirRow[] = [];
  const walk = (node: DirNode, depth: number) => {
    rows.push({ node, depth });
    if (!expanded.has(node.path)) return;
    for (const child of node.children) {
      if (showHidden || !isHidden(child) || kept?.has(child.path)) walk(child, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}
