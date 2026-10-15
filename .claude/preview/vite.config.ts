// 仅用于在浏览器里预览界面：注入一个假的 Tauri IPC。用完即删。
import { defineConfig, mergeConfig } from "vite";
import base from "../../vite.config";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const mock = () => readFileSync(fileURLToPath(new URL("./mock.js", import.meta.url)), "utf8");
export default mergeConfig((base as any)(), defineConfig({
  root: fileURLToPath(new URL("../..", import.meta.url)),
  server: { port: 1430, strictPort: true },
  plugins: [{ name: "fake-ipc", transformIndexHtml: (html) => html.replace("<head>", `<head><script>${mock()}</script>`) }],
}));
