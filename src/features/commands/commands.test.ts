import assert from "node:assert/strict";
import { test } from "node:test";
import { BUILTIN, FORMER_TITLES, GROUPS } from "./builtinCommands.ts";
import {
  commandGroups, commandParts, defaultsChanged, favoriteGroup, FAVORITES, filterGroups, hasPlaceholder, markHits, parseCustomCommand, removeCommand,
  restoreDefaults, toggleFavorite, UNGROUPED, type CommandGroup, type CustomCommand,
} from "./commands.ts";

const mine = (id: string, group = "", command = `echo ${id}`): CustomCommand => ({ id, name: `name ${id}`, command, group });
const defaults = (lang: "zh" | "en") => commandGroups([], lang, "Ungrouped");
const ids = (groups: CommandGroup[]) => groups.flatMap((g) => g.items.map((i) => i.id));
const find = (groups: CommandGroup[], id: string) => groups.flatMap((g) => g.items).find((i) => i.id === id);

test("the default commands are complete in both languages", () => {
  const [zh, en] = [defaults("zh"), defaults("en")];
  assert.ok(ids(zh).length >= 250);
  assert.equal(new Set(ids(zh)).size, ids(zh).length, "a command is listed twice");
  // 每个默认分组都有命令，并且按定义的顺序出现。
  assert.deepEqual(zh.map((g) => g.key), Object.keys(GROUPS));
  assert.deepEqual(ids(en), ids(zh));
  assert.deepEqual(en.map((g) => g.key), zh.map((g) => g.key));
  for (const group of [...zh, ...en]) {
    assert.ok(group.title && group.items.length, "an empty group");
    for (const item of group.items) assert.ok(item.name.trim() && item.command.trim() && item.group === group.title);
  }
  for (const item of en.flatMap((g) => g.items)) assert.ok(!/[一-鿿]/.test(item.name), item.name);
  for (const item of zh.flatMap((g) => g.items)) assert.ok(/[一-鿿]|Compose|Docker|CPU|DNS/.test(item.name), item.name);
});

test("the default groups and commands are well-formed", () => {
  // 分组按标题归类，不论用哪种语言写：两个分组不能叫同一个名字。
  const titles = Object.values(GROUPS).flatMap((text) => [...new Set([text.zh.toLowerCase(), text.en.toLowerCase()])]);
  assert.equal(new Set(titles).size, titles.length, "two groups share a title");
  for (const [, command, zh, en] of BUILTIN) {
    assert.equal(command, command.trim(), command);
    assert.ok(!command.includes("\n"), `${command} spans lines`);
    assert.ok(zh === zh.trim() && en === en.trim(), command);
  }
  // 每个分组里，名称各不相同：否则列表里分不清谁是谁。
  for (const lang of ["zh", "en"] as const) {
    for (const group of defaults(lang)) {
      const names = group.items.map((item) => item.name);
      assert.deepEqual(names.filter((name, i) => names.indexOf(name) !== i), [], `${group.title} repeats a name`);
    }
  }
});

test("no default command is one that destroys things", () => {
  for (const { command } of defaults("en").flatMap((g) => g.items)) {
    // 待填写的 `<PART>` 后面跟着 "/" 不是往系统路径重定向，先把它们换掉再检查。
    const plain = command.replace(/<[A-Z][A-Z0-9_]*>/g, "X");
    assert.ok(!/\brm\b|\bmkfs|\bdd\b|\bshutdown\b|\breboot\b|>\s*\//.test(plain), command);
  }
});

test("a part left to fill in is recognised", () => {
  assert.ok(hasPlaceholder("kill -9 <PID>"));
  assert.ok(hasPlaceholder("tar -czf <NAME>.tar.gz <DIR>"));
  assert.ok(!hasPlaceholder("df -h"));
  // 普通的 shell 语法不算占位符。
  assert.ok(!hasPlaceholder("sort < names.txt > sorted.txt"));
  assert.ok(!hasPlaceholder("cat <<EOF"));
});

test("new commands join the list: ungrouped first, then own groups, then the default ones", () => {
  const groups = commandGroups([mine("a", "db"), mine("b"), mine("c", "site"), mine("d", "db"), mine("e")], "en", "Ungrouped");
  assert.deepEqual(groups.slice(0, 3).map((g) => [g.key, g.title, g.items.map((i) => i.id)]), [
    [UNGROUPED, "Ungrouped", ["b", "e"]],
    ["custom:db", "db", ["a", "d"]],
    ["custom:site", "site", ["c"]],
  ]);
  assert.deepEqual(groups.slice(3).map((g) => g.key), defaults("en").map((g) => g.key));
  assert.equal(find(groups, "b")!.group, "");
  assert.equal(find(groups, "a")!.group, "db");
});

test("a command put in a default group lands there whatever language the title was typed in", () => {
  for (const title of ["网络", "network", " Network "]) {
    for (const lang of ["zh", "en"] as const) {
      const network = commandGroups([mine("a", title)], lang, "Ungrouped").find((g) => g.key === "network")!;
      assert.equal(network.title, lang === "zh" ? "网络" : "Network");
      assert.equal(network.items[network.items.length - 1].id, "a");
    }
  }
});

test("a command saved under a group's former title stays in that group", () => {
  // 以前用过的标题不能和现在某个分组的标题相同，并且都指向现有的分组。
  const titles = Object.values(GROUPS).flatMap((text) => [text.zh.toLowerCase(), text.en.toLowerCase()]);
  for (const [title, key] of Object.entries(FORMER_TITLES)) {
    assert.ok(!titles.includes(title) && title === title.toLowerCase() && key in GROUPS, title);
  }
  const groups = commandGroups([mine("a", "硬件"), mine("b", "Performance"), mine("c", " Archives "), mine("d", "防火墙"), mine("e", "ops")], "zh", "未分组");
  const groupOf = (id: string) => groups.find((g) => g.items.some((i) => i.id === id))!;
  assert.deepEqual(["a", "b", "c", "d", "e"].map((id) => groupOf(id).key), ["system", "process", "files", "security", "custom:ops"]);
  // 显示的是分组现在的标题。
  assert.equal(find(groups, "a")!.group, "系统");
});

test("a default command can be changed, moved and deleted like any other", () => {
  const id = "builtin:uptime";
  const before = defaults("en");
  const at = ids(before).indexOf(id);

  // 修改后留在原来的位置，其余命令不受影响。
  const edited = commandGroups([{ id, name: "Load", command: "uptime -p", group: "System" }], "zh", "未分组");
  assert.equal(ids(edited).indexOf(id), at);
  assert.deepEqual(find(edited, id), { id, name: "Load", command: "uptime -p", group: "系统" });
  assert.equal(ids(edited).length, ids(before).length);

  const moved = commandGroups([{ id, name: "Load", command: "uptime", group: "ops" }], "en", "Ungrouped");
  assert.deepEqual(moved[0].items.map((i) => i.id), [id]);
  assert.equal(moved[0].key, "custom:ops");

  const saved = removeCommand([mine("a"), { id, name: "Load", command: "uptime", group: "" }], id);
  assert.deepEqual(saved, [mine("a"), { id, name: "", command: "", group: "", deleted: true }]);
  assert.ok(!ids(commandGroups(saved, "en", "Ungrouped")).includes(id));
  assert.equal(ids(commandGroups(saved, "en", "Ungrouped")).length, ids(before).length);

  // 删除自己新建的命令不留痕迹。
  assert.deepEqual(removeCommand([mine("a"), mine("b")], "a"), [mine("b")]);
});

test("the default commands can be brought back without touching one's own", () => {
  const saved = [mine("a"), { id: "builtin:uptime", name: "Load", command: "uptime", group: "" }, ...removeCommand([], "builtin:df -h")];
  assert.ok(defaultsChanged(saved));
  assert.ok(!defaultsChanged([mine("a")]));
  assert.deepEqual(restoreDefaults(saved), [mine("a")]);
  assert.deepEqual(ids(commandGroups(restoreDefaults(saved), "en", "U")).filter((id) => id !== "a"), ids(defaults("en")));
});

test("searching matches every word against name, command and group", () => {
  const all = commandGroups([mine("a", "db", "shopctl dump shop")], "en", "Ungrouped");
  const found = (query: string) => filterGroups(all, query).flatMap((g) => g.items.map((i) => i.command));
  assert.deepEqual(found("  "), all.flatMap((g) => g.items.map((i) => i.command)));
  assert.deepEqual(found("shopctl"), ["shopctl dump shop"]);
  assert.deepEqual(found("DB shop"), ["shopctl dump shop"]);
  assert.ok(found("docker").length >= 8 && found("docker").every((c) => c.startsWith("docker")));
  assert.deepEqual(found("docker follow compose"), ["docker compose logs -f --tail 100"]);
  assert.deepEqual(found("no-such-thing"), []);
  // Kubernetes 的命令和 Docker 的在同一个分组里，靠别名才搜得到，而且不带出 Docker 的命令。
  const k8s = found("k8s");
  assert.ok(k8s.length >= 20 && k8s.every((c) => /^(kubectl|helm|crictl) /.test(c)), k8s.join("\n"));
  assert.deepEqual(found("Kubernetes"), k8s);
  assert.deepEqual(found("k8s rollout undo"), ["kubectl rollout undo deployment/<NAME> -n <NAMESPACE>"]);
  // 分组标题里省掉的词也靠别名：搜“防火墙”只出防火墙的命令，不带出同组的其他命令。
  const firewall = found("防火墙");
  assert.ok(firewall.length >= 10 && firewall.every((c) => /^(ufw|firewall-cmd|iptables|nft) /.test(c)), firewall.join("\n"));
  assert.deepEqual(found("firewall"), firewall);
  assert.ok(found("性能").includes("vmstat 1 5") && found("压缩").includes("unzip <FILE> -d <DIR>") && found("硬件").includes("lscpu"));
  // 自己保存的命令也一样。
  const own = commandGroups([mine("a", "", "kubectl get cronjobs")], "en", "Ungrouped");
  assert.ok(filterGroups(own, "k8s").flatMap((g) => g.items.map((i) => i.id)).includes("a"));
});

test("favourite commands form a group of their own, in the order they were added", () => {
  const all = commandGroups([mine("a", "db")], "en", "Ungrouped");
  assert.equal(favoriteGroup(all, [], "Favorites"), null);

  let favorites = toggleFavorite([], "builtin:df -h");
  favorites = toggleFavorite(favorites, "a");
  favorites = toggleFavorite(favorites, "builtin:uptime");
  const group = favoriteGroup(all, favorites, "Favorites")!;
  assert.deepEqual([group.key, group.title, group.items.map((i) => i.id)], [FAVORITES, "Favorites", ["builtin:df -h", "a", "builtin:uptime"]]);
  // 各条命令和它在自己分组里的那一条是同一条。
  assert.deepEqual(group.items[1], find(all, "a"));

  // 再点一次就取消。
  assert.deepEqual(toggleFavorite(favorites, "a"), ["builtin:df -h", "builtin:uptime"]);
  // 已经不存在的命令、被搜索滤掉的命令不列出。
  assert.deepEqual(favoriteGroup(all, ["gone", "a", "a"], "F")!.items.map((i) => i.id), ["a"]);
  assert.deepEqual(favoriteGroup(filterGroups(all, "uptime"), favorites, "F")!.items.map((i) => i.id), ["builtin:uptime"]);
  assert.equal(favoriteGroup(filterGroups(all, "docker"), favorites, "F"), null);
});

test("what a search found and what is left to fill in are marked for display", () => {
  assert.deepEqual(markHits("Docker disk", []), [{ text: "Docker disk" }]);
  assert.deepEqual(markHits("Docker disk", ["do", "disk"]), [{ text: "Do", hit: true }, { text: "cker " }, { text: "disk", hit: true }]);
  // 互相重叠的命中合成一段。
  assert.deepEqual(markHits("abcd", ["abc", "cd"]), [{ text: "abcd", hit: true }]);
  assert.deepEqual(commandParts("kill -9 <PID>"), [{ text: "kill -9 " }, { text: "<PID>", placeholder: true }]);
  assert.deepEqual(commandParts("ping <HOST> -c 4", ["ping", "host"]), [
    { text: "ping", hit: true }, { text: " " }, { text: "<HOST>", placeholder: true }, { text: " -c 4" },
  ]);
  assert.deepEqual(commandParts("sort < a > b"), [{ text: "sort < a > b" }]);
});

test("a saved command from a backup is taken only if it is one", () => {
  assert.deepEqual(parseCustomCommand({ id: "1", name: "n", command: "ls", group: "g", extra: 1 }), { id: "1", name: "n", command: "ls", group: "g" });
  // 没有名称时退回到命令的第一行。
  assert.deepEqual(parseCustomCommand({ id: "1", command: " ls -l\nwc " }), { id: "1", name: "ls -l", command: " ls -l\nwc ", group: "" });
  // 被删除的默认命令只留下 id。
  assert.deepEqual(parseCustomCommand({ id: "builtin:uptime", deleted: true, name: "x" }), { id: "builtin:uptime", name: "", command: "", group: "", deleted: true });
  for (const bad of [null, "ls", {}, { id: "", command: "ls" }, { id: "1", command: "  " }, { id: 1, command: "ls" }, { id: "", deleted: true }]) {
    assert.equal(parseCustomCommand(bad), null);
  }
});
