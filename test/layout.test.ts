import { describe, expect, it } from "vitest";
import { FILE_LIST_MIN_WIDTH, FILE_TREE_DEFAULT_WIDTH, WINDOW_MIN, filePaneMinWidth, windowMinWidth } from "@/features/session/layout.ts";

describe("filePaneMinWidth", () => {
  it("隐藏目录树时只留列表宽度", () => {
    expect(filePaneMinWidth(false)).toBe(FILE_LIST_MIN_WIDTH);
  });

  it("显示目录树时加上树的默认宽度", () => {
    expect(filePaneMinWidth(true)).toBe(FILE_LIST_MIN_WIDTH + FILE_TREE_DEFAULT_WIDTH);
  });
});

describe("windowMinWidth", () => {
  it("不显示文件管理器时取窗口的硬性最小宽度", () => {
    expect(windowMinWidth({ showFiles: false, fileLayout: "bottom", treeShown: false })).toBe(WINDOW_MIN.width);
    expect(windowMinWidth({ showFiles: false, fileLayout: "right", treeShown: true })).toBe(WINDOW_MIN.width);
  });

  it("文件管理器在下方时不需要额外容纳终端，仍不小于窗口最小值", () => {
    expect(windowMinWidth({ showFiles: true, fileLayout: "bottom", treeShown: false })).toBe(WINDOW_MIN.width);
    expect(windowMinWidth({ showFiles: true, fileLayout: "bottom", treeShown: true })).toBe(WINDOW_MIN.width);
  });

  it("文件管理器在右侧时终端、分隔条、管理器三段相加", () => {
    // TERMINAL_MIN_WIDTH(650) + SPLITTER_WIDTH(5) + 面板
    expect(windowMinWidth({ showFiles: true, fileLayout: "right", treeShown: false })).toBe(650 + 5 + FILE_LIST_MIN_WIDTH);
    expect(windowMinWidth({ showFiles: true, fileLayout: "right", treeShown: true })).toBe(
      650 + 5 + FILE_LIST_MIN_WIDTH + FILE_TREE_DEFAULT_WIDTH,
    );
  });

  it("小屏幕上以屏幕宽度封顶，宁可让列表横向滚动", () => {
    expect(windowMinWidth({ showFiles: true, fileLayout: "right", treeShown: true }, 1000)).toBe(1000);
  });

  it("屏幕比窗口最小值还窄时仍取窗口最小值", () => {
    expect(windowMinWidth({ showFiles: true, fileLayout: "right", treeShown: true }, 800)).toBe(WINDOW_MIN.width);
  });
});
