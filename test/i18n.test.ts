import { afterEach, describe, expect, it, vi } from "vitest";
import { MESSAGES, isLocale, systemLocale, t } from "@/shared/i18n.ts";

afterEach(() => vi.unstubAllGlobals());

describe("中英文案", () => {
  it("两种语言的 key 完全一致", () => {
    expect(Object.keys(MESSAGES.zh).sort()).toEqual(Object.keys(MESSAGES.en).sort());
  });

  it("每条文案都是非空字符串", () => {
    for (const key of Object.keys(MESSAGES.zh) as (keyof typeof MESSAGES.en)[]) {
      expect(MESSAGES.zh[key].trim().length).toBeGreaterThan(0);
      expect(MESSAGES.en[key].trim().length).toBeGreaterThan(0);
    }
  });
});

describe("t", () => {
  it("填入 {占位符}，未提供的占位符原样保留", () => {
    expect(t("common.saveFailed", { error: "boom" })).toContain("boom");
    expect(t("update.upToDate", { version: "1.2.3" })).toContain("1.2.3");
    // 无参数时占位符原样返回
    expect(t("common.saveFailed")).toContain("{error}");
  });
});

describe("isLocale", () => {
  it("只认 zh 和 en", () => {
    expect(isLocale("zh")).toBe(true);
    expect(isLocale("en")).toBe(true);
    expect(isLocale("ja")).toBe(false);
    expect(isLocale(null)).toBe(false);
  });
});

describe("systemLocale", () => {
  it("中文系统语言返回 zh（含中文地区变体）", () => {
    vi.stubGlobal("navigator", { language: "zh-CN" });
    expect(systemLocale()).toBe("zh");
    vi.stubGlobal("navigator", { language: "zh-TW" });
    expect(systemLocale()).toBe("zh");
  });

  it("其他语言以及没有 navigator 时返回 en", () => {
    vi.stubGlobal("navigator", { language: "en-US" });
    expect(systemLocale()).toBe("en");
    vi.stubGlobal("navigator", undefined);
    expect(systemLocale()).toBe("en");
  });
});
