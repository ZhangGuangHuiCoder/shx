import { describe, expect, it } from "vitest";
import type { FileEntry } from "@/shared/api.ts";
import {
  applyListing,
  compareNames,
  ensurePath,
  expandable,
  findNode,
  pathChain,
  rootNode,
  visibleRows,
  type DirNode,
} from "@/features/files/dirTree.ts";

/** 只关心名称和类型，其余字段取空值。 */
function dirEntry(name: string, kind: FileEntry["kind"] = "dir"): FileEntry {
  return {
    name,
    path: "/" + name,
    kind,
    isDir: kind === "dir" || kind === "link",
    size: 0,
    mtime: null,
    mode: null,
    uid: null,
    gid: null,
    owner: null,
    group: null,
    linkTarget: null,
  };
}

function fileEntry(name: string): FileEntry {
  return { ...dirEntry(name, "file"), isDir: false };
}

const pathsOf = (rows: { node: DirNode }[]) => rows.map((r) => r.node.path);

describe("pathChain", () => {
  it("根目录只有自己", () => {
    expect(pathChain("/")).toEqual(["/"]);
  });

  it("包含根和沿途每一级", () => {
    expect(pathChain("/etc/nginx/conf.d")).toEqual(["/", "/etc", "/etc/nginx", "/etc/nginx/conf.d"]);
    expect(pathChain("/etc/")).toEqual(["/", "/etc"]);
  });
});

describe("compareNames", () => {
  it("数字按数值比较（file2 在 file10 前）", () => {
    expect(compareNames("file2", "file10")).toBeLessThan(0);
    expect(compareNames("file10", "file2")).toBeGreaterThan(0);
    expect(compareNames("a", "a")).toBe(0);
  });
});

describe("ensurePath / findNode", () => {
  it("沿途补上占位节点，节点可被 findNode 找到", () => {
    const root = rootNode();
    const node = ensurePath(root, "/etc/nginx/conf.d");
    expect(node.path).toBe("/etc/nginx/conf.d");
    expect(node.loaded).toBe(false);
    expect(findNode(root, "/etc/nginx")?.path).toBe("/etc/nginx");
    expect(findNode(root, "/etc/nginx/conf.d")?.name).toBe("conf.d");
  });

  it("已存在的节点复用而不是重建", () => {
    const root = rootNode();
    const first = ensurePath(root, "/etc/nginx");
    first.loaded = true;
    const second = ensurePath(root, "/etc/nginx");
    expect(second).toBe(first);
    expect(second.loaded).toBe(true);
  });

  it("findNode 走不通时返回 undefined", () => {
    const root = rootNode();
    expect(findNode(root, "/missing/deep")).toBeUndefined();
  });

  it("插入的兄弟节点按名称排序", () => {
    const root = rootNode();
    ensurePath(root, "/file10");
    ensurePath(root, "/file2");
    expect(root.children.map((c) => c.name)).toEqual(["file2", "file10"]);
  });
});

describe("applyListing", () => {
  it("只收目录，按名称排序并标记 loaded", () => {
    const root = rootNode();
    applyListing(root, [dirEntry("b"), fileEntry("note.txt"), dirEntry("a")]);
    expect(root.children.map((c) => c.name)).toEqual(["a", "b"]);
    expect(root.loaded).toBe(true);
  });

  it("路径由父节点拼出，忽略条目自带的解析路径（避免符号链接出现两次）", () => {
    const root = rootNode();
    const link = ensurePath(root, "/srv/www");
    applyListing(link, [dirEntry("htdocs")]);
    expect(link.children[0].path).toBe("/srv/www/htdocs");
  });

  it("再次列表时保留仍存在的旧节点（连同其已加载的子树）", () => {
    const root = rootNode();
    applyListing(root, [dirEntry("etc"), dirEntry("var")]);
    const etc = root.children[0];
    ensurePath(root, "/etc/nginx");
    applyListing(root, [dirEntry("etc"), dirEntry("tmp")]);
    const etcAfter = root.children.find((c) => c.name === "etc")!;
    expect(etcAfter).toBe(etc);
    expect(etcAfter.children.map((c) => c.name)).toEqual(["nginx"]);
    expect(root.children.map((c) => c.name)).toEqual(["etc", "tmp"]);
  });

  it("清除上一次的错误标记", () => {
    const root = rootNode();
    root.error = "permission denied";
    applyListing(root, [dirEntry("a")]);
    expect(root.error).toBeUndefined();
  });
});

describe("expandable", () => {
  it("尚未列出的节点永远可展开", () => {
    expect(expandable(rootNode(), false)).toBe(true);
  });

  it("已列出且为空不可展开", () => {
    const node = rootNode();
    applyListing(node, []);
    expect(expandable(node, false)).toBe(false);
  });

  it("只有隐藏目录时：显示隐藏文件才可展开", () => {
    const node = rootNode();
    applyListing(node, [dirEntry(".secret")]);
    expect(expandable(node, false)).toBe(false);
    expect(expandable(node, true)).toBe(true);
  });
});

describe("visibleRows", () => {
  function builtTree() {
    const root = rootNode();
    applyListing(root, [dirEntry("a"), dirEntry(".hidden-root")]);
    applyListing(findNode(root, "/a")!, [dirEntry("normal"), dirEntry(".secret")]);
    applyListing(findNode(root, "/a/.secret")!, [dirEntry("c")]);
    return root;
  }

  it("全部折叠时只有根一行", () => {
    const rows = visibleRows(builtTree(), new Set(), false);
    expect(pathsOf(rows)).toEqual(["/"]);
  });

  it("展开后按深度列出，隐藏目录默认不出现", () => {
    const expanded = new Set(["/", "/a"]);
    const rows = visibleRows(builtTree(), expanded, false);
    expect(pathsOf(rows)).toEqual(["/", "/a", "/a/normal"]);
    expect(rows.map((r) => r.depth)).toEqual([0, 1, 2]);
  });

  it("showHidden 时隐藏目录也列出", () => {
    const expanded = new Set(["/", "/a", "/.hidden-root", "/a/.secret"]);
    const rows = visibleRows(builtTree(), expanded, true);
    expect(pathsOf(rows)).toContain("/.hidden-root");
    expect(pathsOf(rows)).toContain("/a/.secret/c");
  });

  it("通往当前位置的隐藏链即使没开 showHidden 也保留", () => {
    const expanded = new Set(["/", "/a", "/a/.secret"]);
    const rows = visibleRows(builtTree(), expanded, false, "/a/.secret/c");
    expect(pathsOf(rows)).toContain("/a/.secret");
    expect(pathsOf(rows)).toContain("/a/.secret/c");
    // 与当前位置无关的隐藏目录仍不出现
    expect(pathsOf(rows)).not.toContain("/.hidden-root");
  });
});
