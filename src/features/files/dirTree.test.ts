import assert from "node:assert/strict";
import { test } from "node:test";
import type { FileEntry } from "../../shared/api.ts";
import { applyListing, ensurePath, expandable, findNode, pathChain, rootNode, visibleRows } from "./dirTree.ts";

function entry(path: string, kind: FileEntry["kind"] = "dir", isDir = kind === "dir"): FileEntry {
  return {
    name: path.slice(path.lastIndexOf("/") + 1), path, kind, isDir,
    size: 0, mtime: null, mode: null, uid: null, gid: null, owner: null, group: null, linkTarget: null,
  };
}

const paths = (root = rootNode(), expanded: string[] = [], showHidden = false, keep = "") =>
  visibleRows(root, new Set(expanded), showHidden, keep).map((row) => `${row.depth}:${row.node.path}`);

test("pathChain lists every folder from the root down", () => {
  assert.deepEqual(pathChain("/"), ["/"]);
  assert.deepEqual(pathChain("/var/www/app"), ["/", "/var", "/var/www", "/var/www/app"]);
  assert.deepEqual(pathChain("/var//log/"), ["/", "/var", "/var/log"]);
});

test("a listing keeps folders only, in natural name order", () => {
  const root = rootNode();
  applyListing(root, [entry("/dir10"), entry("/notes.md", "file"), entry("/dir2"), entry("/Dir1")]);
  assert.ok(root.loaded);
  assert.deepEqual(root.children.map((c) => c.name), ["Dir1", "dir2", "dir10"]);
});

test("a fresh listing keeps what was loaded below folders that are still there", () => {
  const root = rootNode();
  applyListing(root, [entry("/etc"), entry("/tmp")]);
  applyListing(findNode(root, "/etc")!, [entry("/etc/nginx")]);
  applyListing(root, [entry("/etc"), entry("/srv")]);
  assert.deepEqual(root.children.map((c) => c.path), ["/etc", "/srv"]);
  assert.ok(findNode(root, "/etc/nginx"));
  assert.equal(findNode(root, "/tmp"), undefined);
});

test("folders under a symlink are addressed through the link", () => {
  const root = rootNode();
  applyListing(root, [entry("/bin", "link", true), entry("/usr")]);
  // 服务器把 /bin 解析为 /usr/bin，并按那里的路径返回条目。
  applyListing(findNode(root, "/bin")!, [entry("/usr/bin/X11")]);
  assert.deepEqual(findNode(root, "/bin")!.children.map((c) => c.path), ["/bin/X11"]);
  assert.equal(findNode(root, "/bin")!.kind, "link");
});

test("ensurePath adds placeholders that a later listing fills in", () => {
  const root = rootNode();
  const leaf = ensurePath(root, "/home/zhang");
  assert.equal(leaf.path, "/home/zhang");
  assert.equal(findNode(root, "/home")!.loaded, false);
  assert.equal(ensurePath(root, "/home/zhang"), leaf);

  applyListing(findNode(root, "/home")!, [entry("/home/git"), entry("/home/zhang")]);
  assert.equal(findNode(root, "/home/zhang"), leaf);
  assert.deepEqual(findNode(root, "/home")!.children.map((c) => c.name), ["git", "zhang"]);
});

test("only expanded folders contribute rows", () => {
  const root = rootNode();
  applyListing(root, [entry("/etc"), entry("/var")]);
  applyListing(findNode(root, "/var")!, [entry("/var/log")]);
  assert.deepEqual(paths(root), ["0:/"]);
  assert.deepEqual(paths(root, ["/"]), ["0:/", "1:/etc", "1:/var"]);
  assert.deepEqual(paths(root, ["/", "/var"]), ["0:/", "1:/etc", "1:/var", "2:/var/log"]);
  // 折叠的文件夹里即使有已展开的文件夹，也不显示。
  assert.deepEqual(paths(root, ["/var"]), ["0:/"]);
});

test("hidden folders show on request, or when they lead to the current folder", () => {
  const root = rootNode();
  applyListing(root, [entry("/.cache"), entry("/.config"), entry("/srv")]);
  applyListing(findNode(root, "/.config")!, [entry("/.config/nvim")]);
  const open = ["/", "/.config"];
  assert.deepEqual(paths(root, open), ["0:/", "1:/srv"]);
  assert.deepEqual(paths(root, open, true), ["0:/", "1:/.cache", "1:/.config", "2:/.config/nvim", "1:/srv"]);
  assert.deepEqual(paths(root, open, false, "/.config/nvim"), ["0:/", "1:/.config", "2:/.config/nvim", "1:/srv"]);
});

test("the expand arrow goes away once a folder is known to have nothing to show", () => {
  const root = rootNode();
  assert.ok(expandable(root, false));
  applyListing(root, [entry("/.git"), entry("/readme.md", "file")]);
  assert.ok(!expandable(root, false));
  assert.ok(expandable(root, true));
});
