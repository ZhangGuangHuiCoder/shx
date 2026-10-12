import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_FORMAT, FORMATS, levelFor, levelNumber, mayGrow, splitName } from "./archive.ts";

const format = (ext: string) => FORMATS.find((f) => f.ext === ext)!;

test("an archive's name splits into what comes before the extension and its format", () => {
  const split = (name: string) => [splitName(name).stem, splitName(name).format?.ext ?? null];
  assert.deepEqual(split("site.tar.gz"), ["site", ".tar.gz"]);
  assert.deepEqual(split("SITE.TGZ"), ["SITE", ".tar.gz"]);
  assert.deepEqual(split("a.b.tar.gz"), ["a.b", ".tar.gz"]);
  assert.deepEqual(split("a.tar"), ["a", ".tar"]);
  assert.deepEqual(split("a.zip"), ["a", ".zip"]);
  assert.deepEqual(split("backup.tar.zip"), ["backup.tar", ".zip"]);
  // 没有压缩包扩展名时名称整体保留："v1.2" 不是 "v1" 加 ".2"。
  assert.deepEqual(split("release-v1.2"), ["release-v1.2", null]);
  assert.deepEqual(split("a.gz"), ["a.gz", null]);
  // 对话框不提供的格式，扩展名只是名称的一部分。
  assert.deepEqual(split("a.tar.xz"), ["a.tar.xz", null]);
  assert.deepEqual(split("a.tbz2"), ["a.tbz2", null]);
  assert.deepEqual(split("my site"), ["my site", null]);
  // 只有扩展名不算名称。
  assert.deepEqual(split(".zip"), [".zip", null]);
});

test("zip, tar and tar.gz are offered, tar.gz unless chosen otherwise", () => {
  assert.deepEqual(FORMATS.map((f) => f.ext), [".zip", ".tar", ".tar.gz"]);
  assert.equal(DEFAULT_FORMAT.ext, ".tar.gz");
});

test("only .tar can still grow into another format's extension", () => {
  assert.deepEqual(FORMATS.filter(mayGrow).map((f) => f.ext), [".tar"]);
});

test("every format offers levels, and a level it lacks falls back sensibly", () => {
  for (const f of FORMATS) assert.ok(f.levels.length > 0, f.ext);
  assert.equal(levelFor(format(".zip"), "store"), "store");
  assert.equal(levelFor(format(".tar.gz"), "store"), "normal");
  assert.equal(levelFor(format(".tar.gz"), "max"), "max");
  assert.equal(levelFor(format(".tar"), "max"), "store");
  assert.deepEqual((["store", "fast", "normal", "max"] as const).map(levelNumber), [0, 1, null, 9]);
});
