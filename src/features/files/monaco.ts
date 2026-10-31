// 体积大，首次打开文件时才动态导入。
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import CssWorker from "monaco-editor/language/css/css.worker.js?worker";
import HtmlWorker from "monaco-editor/language/html/html.worker.js?worker";
import JsonWorker from "monaco-editor/language/json/json.worker.js?worker";
import TsWorker from "monaco-editor/language/typescript/ts.worker.js?worker";

// label 为语言服务名，其余语言只用通用 worker。
self.MonacoEnvironment = {
  getWorker(_id, label) {
    switch (label) {
      case "json":
        return new JsonWorker();
      case "css":
      case "scss":
      case "less":
        return new CssWorker();
      case "html":
      case "handlebars":
      case "razor":
        return new HtmlWorker();
      case "typescript":
      case "javascript":
        return new TsWorker();
      default:
        return new EditorWorker();
    }
  },
};

// 沿用 vs/vs-dark 语法高亮，底色等与应用界面统一。
const defineTheme = (name: string, base: monaco.editor.BuiltinTheme, colors: monaco.editor.IColors) =>
  monaco.editor.defineTheme(name, { base, inherit: true, rules: [], colors });

defineTheme("shx-dark", "vs-dark", {
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
});
defineTheme("shx-light", "vs", {
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
});

export { monaco };
