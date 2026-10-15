import { describe, expect, it } from "vitest";
import type { FileEntry } from "@/shared/api.ts";
import { nextActive, sortEntries, treeRows, type Listing } from "@/features/files/editorTree.ts";

/** 只关心路径和类型，其余字段取空值。 */
function entry(path: string, isDir = false): FileEntry {
  const name = path.slice(path.lastIndexOf("/") + 1);
  return { name, path, kind: isDir ? "dir" : "file", isDir, size: 0, mtime: null, mode: null, uid: null, gid: null, owner: null, group: null, linkTarget: null };
}

const listings = new Map<string, Listing>([
  ["/etc", { entries: sortEntries([entry("/etc/hosts"), entry("/etc/nginx", true), entry("/etc/.hidden"), entry("/etc/caddy", true)]) }],
  ["/etc/nginx", { entries: sortEntries([entry("/etc/nginx/nginx.conf"), entry("/etc/nginx/mime.types")]) }],
]);

const names = (rows: ReturnType<typeof treeRows>) => rows.map((r) => `${"  ".repeat(r.depth)}${r.entry.name}`);

describe("sortEntries", () => {
  it("文件夹在前，同类按名称自然排序", () => {
    const sorted = sortEntries([entry("/b10"), entry("/z", true), entry("/b2"), entry("/a", true)]);
    expect(sorted.map((e) => e.name)).toEqual(["a", "z", "b2", "b10"]);
  });
});

describe("treeRows", () => {
  it("只展开 expanded 里的文件夹，隐藏文件按设置决定", () => {
    expect(names(treeRows("/etc", listings, new Set(), { showHidden: false }))).toEqual(["caddy", "nginx", "hosts"]);
    expect(names(treeRows("/etc", listings, new Set(["/etc/nginx"]), { showHidden: true }))).toEqual([
      "caddy",
      "nginx",
      "  mime.types",
      "  nginx.conf",
      ".hidden",
      "hosts",
    ]);
  });

  it("筛选时保留名称匹配的文件，以及里面有匹配项的展开文件夹", () => {
    const rows = treeRows("/etc", listings, new Set(["/etc/nginx"]), { showHidden: false, filter: "CONF" });
    expect(names(rows)).toEqual(["nginx", "  nginx.conf"]);
  });

  it("没列出过的文件夹没有内容", () => {
    expect(treeRows("/var", listings, new Set(), { showHidden: false })).toEqual([]);
  });
});

describe("nextActive", () => {
  const paths = ["/a", "/b", "/c"];
  it("关的不是当前标签页时不变", () => expect(nextActive(paths, "/a", "/b")).toBe("/b"));
  it("关当前的换成右边的", () => expect(nextActive(paths, "/b", "/b")).toBe("/c"));
  it("最右边的换成左边的", () => expect(nextActive(paths, "/c", "/c")).toBe("/b"));
  it("最后一个关掉后没有", () => expect(nextActive(["/a"], "/a", "/a")).toBeNull());
});
