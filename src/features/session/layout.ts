// 会话区域里各部分至少要多宽，以及由此得出的窗口最小宽度。纯函数，不依赖框架。
//
// 每一部分的宽度都按“默认状态下内容显示得全”来定：终端 80 列，目录树是它的默认宽度，
// 文件列表每一列都不截断。窗口的最小宽度是它们加起来的结果，而不是反过来。

/**
 * 文件列表显示全部列所需的宽度：各列最小宽度之和（FileManager.vue 的 `.grid`）加上纵向滚动条。
 * 列表不窄于它，就不会出现横向滚动条。
 */
export const FILE_LIST_MIN_WIDTH = 610;
/** 目录树的默认宽度，文件管理器里至少给它留这么多。拖得更宽的，地方不够时自己让出来。 */
export const FILE_TREE_DEFAULT_WIDTH = 200;
/**
 * 文件管理器在右侧时留给终端的宽度：默认字号下放得下 80 列。一列最宽 7.8（字号的 0.6 倍），
 * 80 列是 624，再加左边距 8 和滚动条 14。
 */
const TERMINAL_MIN_WIDTH = 650;
/** 终端和文件管理器之间那条分隔条。 */
const SPLITTER_WIDTH = 5;

/** 窗口最小的尺寸（逻辑像素）；和 tauri.conf.json 里窗口的初始设定一致。 */
export const WINDOW_MIN = { width: 900, height: 560 };

/** 文件管理器至少要多宽，它的列表才不用横向滚动。 */
export function filePaneMinWidth(treeShown: boolean): number {
  return FILE_LIST_MIN_WIDTH + (treeShown ? FILE_TREE_DEFAULT_WIDTH : 0);
}

/**
 * 当前布局下窗口的最小宽度：文件管理器在右侧时，要同时放得下终端和文件管理器。
 * 不超过屏幕的宽度 `screenWidth`：小屏幕上宁可让列表横向滚动，也不能让窗口伸到屏幕外面。
 */
export function windowMinWidth(view: { showFiles: boolean; fileLayout: "bottom" | "right"; treeShown: boolean }, screenWidth = Infinity): number {
  const files = view.showFiles ? filePaneMinWidth(view.treeShown) : 0;
  const needed = files && view.fileLayout === "right" ? TERMINAL_MIN_WIDTH + SPLITTER_WIDTH + files : files;
  return Math.max(WINDOW_MIN.width, Math.min(needed, screenWidth));
}
