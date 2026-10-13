import assert from "node:assert/strict";
import { test } from "node:test";
import { filePaneMinWidth, windowMinWidth, WINDOW_MIN } from "./layout.ts";

const view = (over: Partial<Parameters<typeof windowMinWidth>[0]> = {}) => ({ showFiles: true, fileLayout: "bottom" as const, treeShown: true, ...over });

test("the file manager needs room for every column, plus the folder tree when it is shown", () => {
  assert.equal(filePaneMinWidth(false), 610);
  assert.equal(filePaneMinWidth(true), 810);
});

test("below the terminal the file manager spans the window, which is already wide enough", () => {
  assert.equal(windowMinWidth(view()), WINDOW_MIN.width);
  assert.equal(windowMinWidth(view({ treeShown: false })), WINDOW_MIN.width);
});

test("beside the terminal the window has to hold both", () => {
  assert.equal(windowMinWidth(view({ fileLayout: "right" })), 650 + 5 + 810);
  assert.equal(windowMinWidth(view({ fileLayout: "right", treeShown: false })), 650 + 5 + 610);
  // 文件管理器隐藏时，终端独占窗口。
  assert.equal(windowMinWidth(view({ fileLayout: "right", showFiles: false })), WINDOW_MIN.width);
});

test("the window is never forced wider than the screen, nor narrower than its base minimum", () => {
  assert.equal(windowMinWidth(view({ fileLayout: "right" }), 1280), 1280);
  assert.equal(windowMinWidth(view({ fileLayout: "right" }), 1920), 1465);
  assert.equal(windowMinWidth(view({ fileLayout: "right" }), 800), WINDOW_MIN.width);
});
