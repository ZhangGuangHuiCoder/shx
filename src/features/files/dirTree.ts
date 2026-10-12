// 文件列表旁的目录树，以普通数据表示。不依赖框架，不做 I/O：
// 组件把列表喂给它，再绘制它返回的行。
import type { FileEntry } from "../../shared/api.ts";

export interface DirNode {
  path: string;
  name: string;
  kind: FileEntry["kind"];
  /** 子文件夹，按显示顺序排列。`loaded` 之后才完整。 */
  children: DirNode[];
  /** `children` 是否来自对该文件夹的一次列表。 */
  loaded: boolean;
  /** 上一次列出失败的原因（如果失败了）。 */
  error?: string;
}

/** 目录树中可见的一行。 */
export interface DirRow {
  node: DirNode;
  depth: number;
}

const collator = new Intl.Collator(undefined, { numeric: true });

/** 目录树和文件列表共用的名称排序（"file2" 排在 "file10" 前面）。 */
export const compareNames = (a: string, b: string) => collator.compare(a, b);

export function rootNode(): DirNode {
  return { path: "/", name: "/", kind: "dir", children: [], loaded: false };
}

/** `/` 以及一路到 `path` 的每个文件夹，包含 `path` 本身。 */
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

/**
 * 返回 `path` 对应的节点；途中树还不认识的文件夹（上级目录尚未列出，或无法列出）
 * 用占位节点补上。
 */
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

/**
 * 用新列表中的文件夹替换某个文件夹的子节点。仍然存在的子节点保留原节点，
 * 其下已加载的内容也随之保留。
 */
export function applyListing(node: DirNode, entries: FileEntry[]) {
  const known = new Map(node.children.map((child) => [child.path, child]));
  const prefix = node.path === "/" ? "/" : `${node.path}/`;
  node.children = entries
    .filter((e) => e.isDir)
    .map((e) => {
      // 路径由节点自身的路径拼出，而不用条目里的：在符号链接指向的文件夹下，
      // 列表返回的是解析后的位置，那样它会在树里出现两次。
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

/** 一行是否显示展开箭头：尚未列出，或已列出且有文件夹可显示。 */
export function expandable(node: DirNode, showHidden: boolean): boolean {
  if (!node.loaded) return true;
  return showHidden ? node.children.length > 0 : node.children.some((child) => !isHidden(child));
}

/**
 * 把树转成要绘制的行：所有祖先都已展开的节点。除非 `showHidden`，隐藏文件夹不列出；
 * 但通往 `keep`（当前显示的文件夹）的除外，这样当前位置不会从树里消失。
 */
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
