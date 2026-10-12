import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import process from "node:process";
import { fileURLToPath } from "node:url";
const host = process.env.TAURI_DEV_HOST;

// 配置说明：https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  resolve: {
    alias: [
      {
        // Monaco 的 JS/TS、CSS、HTML 语言服务换成空模块，它们各自好几 MB 的 worker 就不会被打包
        // （见 src/features/files/monaco.ts）。
        find: /^\.\/languages\/features\/(typescript|css|html)\/register\.js$/,
        replacement: fileURLToPath(new URL("./src/features/files/monacoNoService.ts", import.meta.url)),
      },
    ],
  },

  // 针对 Tauri 开发的 Vite 选项，只在 `tauri dev` 和 `tauri build` 时起作用
  //
  // 1. 避免 Vite 清屏盖住 Rust 的报错
  clearScreen: false,
  // 2. Tauri 需要固定端口，端口被占用时直接失败
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. 让 Vite 不监听 `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
