// 各区域的最小宽度及由此推出的窗口最小宽度（纯函数）。

/** 文件列表最小宽度：紧凑列宽之和 482（须与 FileManager.vue 一致）+ 滚动条 10。 */
export const FILE_LIST_MIN_WIDTH = 492;
/** 目录树默认宽度。 */
export const FILE_TREE_DEFAULT_WIDTH = 200;
/** 终端最小宽度：默认字号下 80 列（80 × 7.8 = 624）+ 边距 8 + 滚动条 14。 */
const TERMINAL_MIN_WIDTH = 650;
const SPLITTER_WIDTH = 5;

/** 须与 tauri.conf.json 一致。 */
export const WINDOW_MIN = { width: 900, height: 560 };

export function filePaneMinWidth(treeShown: boolean): number {
  return FILE_LIST_MIN_WIDTH + (treeShown ? FILE_TREE_DEFAULT_WIDTH : 0);
}

/** 当前布局下的窗口最小宽度，不超过屏幕宽度。 */
export function windowMinWidth(view: { showFiles: boolean; fileLayout: "bottom" | "right"; treeShown: boolean }, screenWidth = Infinity): number {
  const files = view.showFiles ? filePaneMinWidth(view.treeShown) : 0;
  const needed = files && view.fileLayout === "right" ? TERMINAL_MIN_WIDTH + SPLITTER_WIDTH + files : files;
  return Math.max(WINDOW_MIN.width, Math.min(needed, screenWidth));
}
