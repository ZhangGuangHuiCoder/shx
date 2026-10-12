import assert from "node:assert/strict";
import { test } from "node:test";
import { planSession, sessionName, slotOf } from "./keepAlive.ts";

test("session names are stable and safe to hand to a shell", () => {
  const id = "3f2c1a9e-7b1d-4c55-9a43-0d1f7e2a6b10";
  assert.equal(sessionName(id, 1), sessionName(id, 1));
  for (const profile of [id, "p1", "weird id; rm -rf '/'", "主机"]) {
    assert.match(sessionName(profile, 12), /^shx-12-[0-9a-f]{8}$/);
  }
  assert.notEqual(sessionName("p1", 1), sessionName("p2", 1));
});

test("no session name is the beginning of another", () => {
  // 没有完全匹配时，tmux 会按前缀匹配目标。
  const names = ["p1", "p10", "p100"].flatMap((id) => [1, 10, 11].map((slot) => sessionName(id, slot)));
  for (const a of names) for (const b of names) assert.ok(a === b || !b.startsWith(a), `${a} / ${b}`);
});

test("a session name gives back its slot, for its own host only", () => {
  assert.equal(slotOf("p1", sessionName("p1", 1)), 1);
  assert.equal(slotOf("p1", sessionName("p1", 12)), 12);
  assert.equal(slotOf("p1", sessionName("p2", 1)), null);
  for (const other of [null, "", "main", "shx-0-00000000", "shx-1-xyz", `${sessionName("p1", 1)}x`]) assert.equal(slotOf("p1", other), null);
});

test("with nothing kept on the server, a new tab starts the first free session", () => {
  const [one, two, three] = [1, 2, 3].map((slot) => sessionName("p1", slot));
  assert.deepEqual(planSession("p1", [], []), { enter: one });
  assert.deepEqual(planSession("p1", [one, null], [one]), { enter: two });
  assert.deepEqual(planSession("p1", [two], [two]), { enter: one });
  assert.deepEqual(planSession("p1", [one, two], [one, two]), { enter: three });
  // 其他主机的会话不会造成妨碍，也不会被拿来用。
  assert.deepEqual(planSession("p1", [], [sessionName("p2", 1), "someone-elses"]), { enter: one });
});

test("a single kept session is entered straight away, wherever it sits", () => {
  const [one, two, three] = [1, 2, 3].map((slot) => sessionName("p1", slot));
  assert.deepEqual(planSession("p1", [], [one]), { enter: one });
  assert.deepEqual(planSession("p1", [], [three]), { enter: three });
  // 已经有标签页开着的会话不算“留着的”。
  assert.deepEqual(planSession("p1", [one], [one, two]), { enter: two });
});

test("several kept sessions are offered in slot order, along with a fresh one", () => {
  const name = (slot: number) => sessionName("p1", slot);
  assert.deepEqual(planSession("p1", [], [name(3), name(1)]), { choose: [name(1), name(3)], fresh: name(2) });
  assert.deepEqual(planSession("p1", [name(1)], [name(1), name(2), name(10), name(4)]), { choose: [name(2), name(4), name(10)], fresh: name(3) });
});
