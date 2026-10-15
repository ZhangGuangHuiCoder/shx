import { describe, expect, it } from "vitest";
import { SHORTCUT_ACTIONS, acceleratorOf, formatAccelerator, matchAccelerator, parseAccelerator } from "@/app/shortcuts";
import { IS_MAC } from "@/shared/platform";

/** node 环境无 KeyboardEvent，构造替身。 */
const key = (key: string, mods: Partial<{ ctrl: boolean; alt: boolean; shift: boolean; meta: boolean }> = {}) => ({
  key,
  ctrlKey: mods.ctrl ?? false,
  altKey: mods.alt ?? false,
  shiftKey: mods.shift ?? false,
  metaKey: mods.meta ?? false,
});

describe("parseAccelerator", () => {
  it("解析修饰键与键名", () => {
    const parsed = parseAccelerator("Ctrl+Shift+K");
    expect(parsed).toMatchObject({ ctrl: true, shift: true, alt: false, meta: false, key: "K" });
  });

  it("CmdOrCtrl 按平台落到 Ctrl 或 Cmd 上", () => {
    const parsed = parseAccelerator("CmdOrCtrl+=")!;
    expect(parsed?.key).toBe("=");
    expect(IS_MAC ? parsed?.meta : parsed?.ctrl).toBe(true);
    expect(IS_MAC ? parsed?.ctrl : parsed?.meta).toBe(false);
  });

  it("拒绝写法不对的键位", () => {
    expect(parseAccelerator("Ctrl+")).toBeNull();
    expect(parseAccelerator("+")).toBeNull();
    expect(parseAccelerator("Ctrl+Bogus+K")).toBeNull();
    expect(parseAccelerator("Ctrl+Ctrl+K")).toBeNull();
  });
});

describe("matchAccelerator", () => {
  it("修饰键要全都按到才匹配", () => {
    expect(matchAccelerator(key("k", { ctrl: true, shift: true }), "Ctrl+Shift+K")).toBe(true);
    expect(matchAccelerator(key("k", { ctrl: true }), "Ctrl+Shift+K")).toBe(false);
    expect(matchAccelerator(key("j", { ctrl: true, shift: true }), "Ctrl+Shift+K")).toBe(false);
  });

  it("键名不区分大小写", () => {
    expect(matchAccelerator(key("K", { ctrl: true, shift: true }), "Ctrl+Shift+k")).toBe(true);
  });

  it("单独的 Ctrl 不吃掉 Ctrl+Shift 的键位，反过来也一样", () => {
    expect(matchAccelerator(key("k", { ctrl: true }), "Ctrl+K")).toBe(true);
    expect(matchAccelerator(key("k", { ctrl: true, shift: true }), "Ctrl+K")).toBe(false);
  });
});

describe("acceleratorOf", () => {
  it("把事件写成规范写法", () => {
    expect(acceleratorOf(key("k", { ctrl: true, shift: true }))).toBe(IS_MAC ? "Ctrl+Shift+K" : "CmdOrCtrl+Shift+K");
    expect(acceleratorOf(key("c", { meta: true }))).toBe(IS_MAC ? "Cmd+C" : "CmdOrCtrl+C");
    expect(acceleratorOf(key("Tab", { ctrl: true }))).toBe(IS_MAC ? "Ctrl+Tab" : "CmdOrCtrl+Tab");
  });

  it("单独按修饰键不算组合", () => {
    expect(acceleratorOf(key("Control"))).toBeNull();
    expect(acceleratorOf(key("Shift"))).toBeNull();
  });

  it("写出来的键位自己能匹配上（存进设置后要能原样生效）", () => {
    // 非 mac 平台 meta 归入 CmdOrCtrl，无法往返，跳过。
    const metas = IS_MAC ? [false, true] : [false];
    for (const ctrl of [false, true]) {
      for (const alt of [false, true]) {
        for (const shift of [false, true]) {
          for (const meta of metas) {
            const written = acceleratorOf(key("j", { ctrl, alt, shift, meta }));
            if (!written) continue;
            expect(matchAccelerator(key("j", { ctrl, alt, shift, meta }), written)).toBe(true);
          }
        }
      }
    }
  });
});

describe("formatAccelerator", () => {
  it("按平台展示", () => {
    if (IS_MAC) expect(formatAccelerator("CmdOrCtrl+Shift+K")).toBe("⌘⇧K");
    else expect(formatAccelerator("CmdOrCtrl+Shift+K")).toBe("Ctrl+Shift+K");
  });

  it("写法不对时原样返回", () => {
    expect(formatAccelerator("Ctrl+")).toBe("Ctrl+");
  });
});

describe("SHORTCUT_ACTIONS", () => {
  it("默认键位互不冲突，且都是合法写法", () => {
    const defaults = SHORTCUT_ACTIONS.map((action) => action.defaultKey);
    for (const accelerator of defaults) expect(parseAccelerator(accelerator), accelerator).not.toBeNull();
    expect(new Set(defaults).size).toBe(defaults.length);
  });

  it("动作 id 各不相同", () => {
    expect(new Set(SHORTCUT_ACTIONS.map((action) => action.id)).size).toBe(SHORTCUT_ACTIONS.length);
  });
});
