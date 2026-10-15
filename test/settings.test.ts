import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_EDIT_RULES, FONT_DEFAULT, STORAGE, defaultSettings, readStored } from "@/shared/settings.ts";
import { systemLocale } from "@/shared/i18n.ts";

describe("defaultSettings", () => {
  it("给出应用首发时的默认值", () => {
    const s = defaultSettings();
    expect(s.otherDoubleClick).toBe("download");
    expect(s.showHidden).toBe(false);
    expect(s.showFiles).toBe(true);
    expect(s.fileLayout).toBe("bottom");
    expect(s.showStatusBar).toBe(true);
    expect(s.terminalFontSize).toBe(FONT_DEFAULT);
    expect(s.editorFontSize).toBe(FONT_DEFAULT);
    expect(s.favoriteCommands).toEqual([]);
    expect(s.theme).toBe("dark");
    // 语言取决于运行环境，只验证与探测结果一致。
    expect(s.language).toBe(systemLocale());
  });

  it("editRules 从默认规则拷贝，两次调用互不影响", () => {
    const a = defaultSettings();
    expect(a.editRules).toEqual(DEFAULT_EDIT_RULES);
    a.editRules.push(".custom");
    const b = defaultSettings();
    expect(b.editRules).not.toContain(".custom");
    expect(b.editRules).toHaveLength(DEFAULT_EDIT_RULES.length);
  });
});

describe("readStored", () => {
  let store: Map<string, string>;

  beforeEach(() => {
    store = new Map();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => (store.has(key) ? store.get(key)! : null),
      setItem: (key: string, value: string) => void store.set(key, String(value)),
      removeItem: (key: string) => void store.delete(key),
      clear: () => store.clear(),
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("没有存过返回默认值", () => {
    expect(readStored(STORAGE.settings, { a: 1 })).toEqual({ a: 1 });
  });

  it("存过的值 JSON 解析后返回", () => {
    localStorage.setItem(STORAGE.settings, JSON.stringify({ showHidden: true }));
    expect(readStored(STORAGE.settings, defaultSettings())).toMatchObject({ showHidden: true });
  });

  it("内容损坏时回退到默认值而不是抛错", () => {
    localStorage.setItem(STORAGE.settings, "{not json");
    expect(readStored(STORAGE.settings, "fallback")).toBe("fallback");
  });
});
