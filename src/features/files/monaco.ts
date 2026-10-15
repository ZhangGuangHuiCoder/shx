// 体积大，首次打开文件时才动态导入。
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import JsonWorker from "monaco-editor/language/json/json.worker.js?worker";

self.MonacoEnvironment = {
  getWorker: (_id, label) => (label === "json" ? new JsonWorker() : new EditorWorker()),
};

// 沿用 vs/vs-dark 语法高亮，底色等与应用界面统一。
monaco.editor.defineTheme("shx-dark", {
  base: "vs-dark",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#18181b",
    "editorGutter.background": "#18181b",
    "editor.lineHighlightBackground": "#1f232b",
    "editor.lineHighlightBorder": "#00000000",
    "editorLineNumber.foreground": "#5b6270",
    "editorLineNumber.activeForeground": "#b4bac6",
    "editor.selectionBackground": "#3a5a8c",
    "editor.inactiveSelectionBackground": "#2c3f5e",
    "editorWidget.background": "#1c1f26",
    "editorWidget.border": "#2c3038",
    "input.background": "#14161a",
    "input.border": "#3a3f4a",
    "focusBorder": "#409eff",
  },
});
monaco.editor.defineTheme("shx-light", {
  base: "vs",
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#ffffff",
    "editorGutter.background": "#ffffff",
    "editor.lineHighlightBackground": "#f2f6fb",
    "editor.lineHighlightBorder": "#00000000",
    "editorLineNumber.foreground": "#9aa5b3",
    "editorLineNumber.activeForeground": "#3b4a5c",
    "editor.selectionBackground": "#b9d7fb",
    "editor.inactiveSelectionBackground": "#dcecff",
    "editorWidget.background": "#ffffff",
    "editorWidget.border": "#d5dde8",
    "input.border": "#c1cbd8",
    "focusBorder": "#2584e6",
  },
});

export { monaco };
