import assert from "node:assert/strict";
import { test } from "node:test";
import type { Profile, StoredKey } from "../../shared/api.ts";
import { applyImport, buildExport, ImportError, parseImport, pickSettings, type ConfigData } from "./config.ts";

const profile = (id: string, over: Partial<Profile> = {}): Profile => ({
  id, name: id, group: "", host: `${id}.example.com`, port: 22, username: "root", auth: "password", keyId: null, saveSecret: true, note: "", pinned: false,
  ...over,
});
const key = (id: string, name = id): StoredKey => ({ id, name, content: `KEY-${id}`, encrypted: false });

const local: ConfigData = {
  profiles: [profile("a"), profile("b", { auth: "key", keyId: "k1" })],
  secrets: { a: "pw-a", b: "phrase-b" },
  keys: [key("k1")],
  knownHosts: { "a.example.com:22": { "ssh-ed25519": "SHA256:a", "ssh-rsa": "SHA256:a-rsa" } },
  settings: { terminalFontSize: 13, showHidden: false },
  commands: [{ id: "c1", name: "disk", command: "df -h", group: "" }],
};

test("a backup imports back unchanged", () => {
  const file = buildExport(local, new Date("2026-01-02T03:04:05Z"));
  assert.equal(file.exportedAt, "2026-01-02T03:04:05.000Z");
  assert.deepEqual(parseImport(JSON.stringify(file)), { data: local, skipped: 0 });
});

test("content that is not an shx backup is rejected with a reason", () => {
  const rejected = (json: string, code: ImportError["code"]) =>
    assert.throws(() => parseImport(json), (e) => e instanceof ImportError && e.code === code, json);
  rejected("{ not json", "invalidJson");
  rejected("[]", "notBackup");
  rejected('{"profiles": []}', "notBackup");
  rejected('{"app":"shx","version":2}', "newerVersion");
  rejected('{"app":"shx"}', "newerVersion");
});

test("malformed entries are skipped and counted, valid ones kept and normalised", () => {
  const parsed = parseImport(
    JSON.stringify({
      app: "shx",
      version: 1,
      profiles: [
        { id: "ok", host: "h", port: 22, username: "u", auth: "agent" },
        { id: "", host: "h", port: 22, username: "u", auth: "agent" },
        { id: "x", host: "h", port: 70000, username: "u", auth: "agent" },
        { id: "x", host: "h", port: "22", username: "u", auth: "agent" },
        { id: "x", host: "h", port: 22, username: "u", auth: "telnet" },
        "nonsense",
      ],
      keys: [{ id: "k", content: "KEY" }, { id: "k2", content: "  " }, null],
      secrets: { ok: "pw", bad: 42 },
      knownHosts: { "h:22": { "ssh-ed25519": "SHA256:x", junk: 1 }, bad: "nope" },
      settings: "nope",
      commands: [{ id: "c", command: "df -h" }, { id: "c2", command: "" }, "ls"],
    }),
  );
  assert.deepEqual(parsed.data.profiles, [
    { id: "ok", host: "h", port: 22, username: "u", auth: "agent", name: "h", group: "", keyId: null, saveSecret: false, note: "", pinned: false },
  ]);
  assert.deepEqual(parsed.data.keys, [{ id: "k", content: "KEY", name: "key", encrypted: false }]);
  assert.deepEqual(parsed.data.secrets, { ok: "pw" });
  assert.deepEqual(parsed.data.knownHosts, { "h:22": { "ssh-ed25519": "SHA256:x" } });
  assert.deepEqual(parsed.data.settings, {});
  assert.deepEqual(parsed.data.commands, [{ id: "c", name: "df -h", command: "df -h", group: "" }]);
  assert.equal(parsed.skipped, 5 + 2 + 1 + 2 + 2);
});

const incoming: ConfigData = {
  profiles: [profile("b", { name: "b-renamed", auth: "key", keyId: "k2" }), profile("c")],
  secrets: { b: "new-phrase", c: "pw-c", gone: "orphan" },
  keys: [key("k2")],
  knownHosts: { "a.example.com:22": { "ssh-ed25519": "SHA256:a-new" }, "c.example.com:22": { "ssh-ed25519": "SHA256:c" } },
  settings: { terminalFontSize: 16 },
  commands: [{ id: "c1", name: "disk space", command: "df -h /", group: "ops" }, { id: "c2", name: "mem", command: "free -h", group: "" }],
};

test("keep-alive comes back from a backup, and only as a real true", () => {
  const data: ConfigData = {
    profiles: [profile("on", { keepAlive: true }), profile("off"), { ...profile("odd"), keepAlive: "yes" as unknown as boolean }],
    secrets: {}, keys: [], knownHosts: {}, settings: {}, commands: [],
  };
  const back = parseImport(JSON.stringify(buildExport(data))).data.profiles;
  assert.deepEqual(back.map((p) => p.keepAlive), [true, undefined, undefined]);
});

test("favorite folders come back from a backup, without duplicates or anything that is not a path", () => {
  const data: ConfigData = {
    profiles: [
      profile("some", { favoriteDirs: ["/var/log", "/etc/nginx"] }),
      profile("none"),
      { ...profile("odd"), favoriteDirs: ["/srv", 7, "relative", "/srv", ""] as unknown as string[] },
      { ...profile("junk"), favoriteDirs: "/srv" as unknown as string[] },
    ],
    secrets: {}, keys: [], knownHosts: {}, settings: {}, commands: [],
  };
  const back = parseImport(JSON.stringify(buildExport(data))).data.profiles;
  assert.deepEqual(back.map((p) => p.favoriteDirs), [["/var/log", "/etc/nginx"], undefined, ["/srv"], undefined]);
});

test("importing keeps local entries and lets the backup win on the same id", () => {
  const merged = applyImport(local, incoming);
  assert.deepEqual(merged.profiles.map((p) => [p.id, p.name]), [["a", "a"], ["b", "b-renamed"], ["c", "c"]]);
  assert.deepEqual(merged.keys.map((k) => k.id), ["k1", "k2"]);
  // "gone" 在两边都没有对应的会话配置，所以它的密码不会被带上。
  assert.deepEqual(merged.secrets, { a: "pw-a", b: "new-phrase", c: "pw-c" });
  assert.deepEqual(merged.knownHosts, {
    "a.example.com:22": { "ssh-ed25519": "SHA256:a-new", "ssh-rsa": "SHA256:a-rsa" },
    "c.example.com:22": { "ssh-ed25519": "SHA256:c" },
  });
  assert.deepEqual(merged.settings, { terminalFontSize: 16, showHidden: false });
  assert.deepEqual(merged.commands.map((c) => [c.id, c.command]), [["c1", "df -h /"], ["c2", "free -h"]]);
});

test("a backup from before the command assistant imports with no commands", () => {
  const old = JSON.stringify({ app: "shx", version: 1, profiles: [], secrets: {}, keys: [], knownHosts: {}, settings: {} });
  assert.deepEqual(parseImport(old), { data: { profiles: [], secrets: {}, keys: [], knownHosts: {}, settings: {}, commands: [] }, skipped: 0 });
});

test("restoring a backup onto an empty installation reproduces it", () => {
  const empty: ConfigData = { profiles: [], secrets: {}, keys: [], knownHosts: {}, settings: {}, commands: [] };
  assert.deepEqual(applyImport(empty, local), local);
});

test("importing never mutates what is currently stored", () => {
  const before = structuredClone(local);
  applyImport(local, incoming);
  assert.deepEqual(local, before);
});

test("only known settings of the right shape are taken", () => {
  const defaults = { editRules: [".txt"], otherDoubleClick: "download", showHidden: false, terminalFontSize: 13 };
  assert.deepEqual(
    pickSettings(defaults, { editRules: [".log", "*.conf"], showHidden: true, terminalFontSize: "big", unknown: 1 }),
    { editRules: [".log", "*.conf"], otherDoubleClick: "download", showHidden: true, terminalFontSize: 13 },
  );
  assert.deepEqual(pickSettings(defaults, { editRules: [1, 2], otherDoubleClick: ["x"] }), defaults);
});
