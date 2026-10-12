import assert from "node:assert/strict";
import { test } from "node:test";
import { isLocale, locale, MESSAGES, t, type MessageKey } from "./i18n.ts";

const placeholders = (message: string) => [...message.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
const keys = Object.keys(MESSAGES.zh) as MessageKey[];

test("both languages take the same placeholders for every message", () => {
  for (const key of keys) {
    assert.deepEqual(placeholders(MESSAGES.en[key]), placeholders(MESSAGES.zh[key]), key);
  }
});

test("no message is empty or left untranslated", () => {
  // 少数在两种语言中本来就相同的条目。
  const same = new Set<MessageKey>([]);
  for (const key of keys) {
    assert.ok(MESSAGES.zh[key].trim() && MESSAGES.en[key].trim(), `${key} is empty`);
    assert.ok(!/[一-鿿]/.test(MESSAGES.en[key]), `${key} has Chinese in the English text`);
    if (!same.has(key)) assert.notEqual(MESSAGES.en[key], MESSAGES.zh[key], `${key} is the same in both languages`);
  }
});

test("t follows the current language and fills in placeholders", () => {
  locale.value = "zh";
  assert.equal(t("common.cancel"), "取消");
  assert.equal(t("fm.count", { count: 3 }), "3 项");
  locale.value = "en";
  assert.equal(t("common.cancel"), "Cancel");
  assert.equal(t("fm.count", { count: 3 }), "3 items");
  assert.equal(t("fm.selected", { count: 2, size: "1.5 MB" }), "2 selected, 1.5 MB");
  // 没有传值的占位符原样保留，而不是悄悄丢掉。
  assert.equal(t("fm.count"), "{count} items");
  assert.equal(t("session.connecting", { name: "a{b}c" }), "Connecting to a{b}c…");
});

test("only the supported languages are accepted", () => {
  assert.ok(isLocale("zh") && isLocale("en"));
  assert.ok(!isLocale("fr") && !isLocale(undefined) && !isLocale(""));
});
