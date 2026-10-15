import { describe, expect, it } from "vitest";
import { BUILTIN, FORMER_TITLES, GROUPS, SEARCH_ALIASES } from "@/features/commands/builtinCommands.ts";
import {
  commandGroups,
  commandParts,
  defaultsChanged,
  favoriteGroup,
  filterGroups,
  groupKey,
  hasPlaceholder,
  markHits,
  parseCustomCommand,
  removeCommand,
  restoreDefaults,
  searchWords,
  toggleFavorite,
  UNGROUPED,
  type CustomCommand,
} from "@/features/commands/commands.ts";

const firstBuiltinId = `builtin:${BUILTIN[0][1]}`;
const builtinGroups = (saved: CustomCommand[] = []) => commandGroups(saved, "en", "Ungrouped");
const allItems = (groups: ReturnType<typeof builtinGroups>) => groups.flatMap((g) => g.items);

describe("groupKey", () => {
  it("空标题归入未分组", () => {
    expect(groupKey("")).toBe(UNGROUPED);
    expect(groupKey("   ")).toBe(UNGROUPED);
  });

  it("默认分组的中英文标题（忽略大小写）都归到固定键", () => {
    expect(groupKey("系统")).toBe("system");
    expect(groupKey(" system ")).toBe("system");
    expect(groupKey("Network")).toBe("network");
  });

  it("以前用过的旧标题归到合并后的分组", () => {
    expect(groupKey("硬件")).toBe("system");
    expect(groupKey("Archives")).toBe("files");
  });

  it("其他标题是自建分组，保留原文但去掉首尾空白", () => {
    expect(groupKey("我的分组")).toBe("custom:我的分组");
    expect(groupKey(" My Team ")).toBe("custom:My Team");
  });
});

describe("commandGroups", () => {
  it("空存档下列出全部默认命令，名称取指定语言", () => {
    const en = builtinGroups();
    const zh = commandGroups([], "zh", "未分组");
    const count = (list: typeof en) => list.reduce((n, g) => n + g.items.length, 0);
    expect(count(en)).toBe(BUILTIN.length);
    expect(count(zh)).toBe(BUILTIN.length);

    const uptimeEn = allItems(en).find((i) => i.command === "uptime");
    const uptimeZh = allItems(zh).find((i) => i.command === "uptime");
    expect(uptimeEn?.name).toBe("Uptime and load");
    expect(uptimeZh?.name).toBe("运行时间与负载");
  });

  it("默认分组排在未分组之后、自建分组之后；分组内 item.group 回填标题", () => {
    const groups = builtinGroups([
      { id: "x1", name: "Mine", command: "echo hi", group: "" },
      { id: "x2", name: "Theirs", command: "echo bye", group: "Team" },
    ]);
    expect(groups[0].key).toBe(UNGROUPED);
    expect(groups[0].title).toBe("Ungrouped");
    expect(groups[0].items[0].group).toBe("");
    expect(groups[1].key).toBe("custom:Team");
    expect(groups[groups.length - 1].key in GROUPS).toBe(true);
  });

  it("删除的默认命令不列出", () => {
    const groups = builtinGroups([{ id: "builtin:uptime", name: "", command: "", group: "", deleted: true }]);
    expect(allItems(groups).some((i) => i.command === "uptime")).toBe(false);
  });

  it("修改过的默认命令以存档为准（名称、命令、分组都被覆盖）", () => {
    const groups = builtinGroups([{ id: "builtin:uptime", name: "Load only", command: "uptime -p", group: "System" }]);
    const item = allItems(groups).find((i) => i.id === "builtin:uptime");
    expect(item?.name).toBe("Load only");
    expect(item?.command).toBe("uptime -p");
    expect(item?.group).toBe(GROUPS.system.en);
  });
});

describe("removeCommand", () => {
  it("默认命令留下删除墓碑，防止它重新出现", () => {
    const saved: CustomCommand[] = [];
    const result = removeCommand(saved, firstBuiltinId);
    expect(result).toContainEqual({ id: firstBuiltinId, name: "", command: "", group: "", deleted: true });
  });

  it("用户自建命令直接移除", () => {
    const saved: CustomCommand[] = [{ id: "c1", name: "A", command: "a", group: "" }];
    expect(removeCommand(saved, "c1")).toEqual([]);
  });
});

describe("defaultsChanged / restoreDefaults", () => {
  it("检测是否动过默认命令", () => {
    expect(defaultsChanged([])).toBe(false);
    expect(defaultsChanged([{ id: firstBuiltinId, name: "", command: "", group: "", deleted: true }])).toBe(true);
    expect(defaultsChanged([{ id: "c1", name: "A", command: "a", group: "" }])).toBe(false);
  });

  it("恢复默认只移除默认命令的存档，用户命令保留", () => {
    const saved: CustomCommand[] = [
      { id: firstBuiltinId, name: "", command: "", group: "", deleted: true },
      { id: "c1", name: "A", command: "a", group: "" },
    ];
    expect(restoreDefaults(saved)).toEqual([{ id: "c1", name: "A", command: "a", group: "" }]);
  });
});

describe("toggleFavorite / favoriteGroup", () => {
  it("未收藏则追加，已收藏则取消", () => {
    expect(toggleFavorite([], "a")).toEqual(["a"]);
    expect(toggleFavorite(["a", "b"], "a")).toEqual(["b"]);
  });

  it("收藏分组按顺序列出，去重并跳过已不存在的 id", () => {
    const groups = builtinGroups();
    const items = allItems(groups);
    const fav = favoriteGroup(groups, [items[0].id, "missing", items[0].id, items[1].id], "Favorites");
    expect(fav?.key).toBe("favorites");
    expect(fav?.title).toBe("Favorites");
    expect(fav?.items.map((i) => i.id)).toEqual([items[0].id, items[1].id]);
    expect(favoriteGroup(groups, ["missing"], "Favorites")).toBeNull();
  });
});

describe("搜索", () => {
  it("searchWords 拆成小写词并忽略空白", () => {
    expect(searchWords("  Kill   PID ")).toEqual(["kill", "pid"]);
  });

  it("空查询原样返回全部分组", () => {
    const groups = builtinGroups();
    expect(filterGroups(groups, "   ")).toBe(groups);
  });

  it("多个词要同时命中（名称、命令、分组标题）", () => {
    const groups = builtinGroups();
    const hits = filterGroups(groups, "system uptime");
    const commands = allItems(hits).map((i) => i.command);
    expect(commands).toContain("uptime");
    expect(commands.every((c) => c.includes("uptime"))).toBe(true);
  });

  it("别名里的词也能命中（archive 能搜到 tar）", () => {
    // 取第一条指向现存内置命令的别名。
    const entry = [...SEARCH_ALIASES].find(([cmd]) =>
      BUILTIN.some(([, builtin]) => builtin === cmd || builtin.startsWith(`${cmd} `)),
    )!;
    expect(entry).toBeTruthy();
    const [matchedCommand, words] = entry;
    const aliasWord = words.split(" ").pop()!; // 英文关键词
    const hits = filterGroups(builtinGroups(), aliasWord);
    expect(allItems(hits).some((i) => i.command === matchedCommand || i.command.startsWith(`${matchedCommand} `))).toBe(true);
  });

  it("没有命中的分组被整组去掉", () => {
    const groups = builtinGroups();
    const hits = filterGroups(groups, "definitely-no-such-command-xyz");
    expect(hits).toHaveLength(0);
  });
});

describe("hasPlaceholder", () => {
  it("只有大写尖括号占位才算", () => {
    expect(hasPlaceholder("kill -9 <PID>")).toBe(true);
    expect(hasPlaceholder("tail <FILE> -n <N>")).toBe(true);
    expect(hasPlaceholder("ls -l")).toBe(false);
    expect(hasPlaceholder("grep <pid>")).toBe(false);
  });
});

describe("markHits / commandParts", () => {
  it("按命中分段且不区分大小写", () => {
    const parts = markHits("Kill the KILL", ["kill"]);
    expect(parts.filter((p) => p.hit).map((p) => p.text)).toEqual(["Kill", "KILL"]);
    expect(parts[0]).toEqual({ text: "Kill", hit: true });
  });

  it("没有词时整段是普通文本", () => {
    expect(markHits("abc", [])).toEqual([{ text: "abc" }]);
  });

  it("commandParts 把占位符单独成段，普通文字照常标命中", () => {
    const parts = commandParts("kill <PID> now", ["kill"]);
    expect(parts.some((p) => p.text === "<PID>" && p.placeholder)).toBe(true);
    expect(parts[0]).toEqual({ text: "kill", hit: true });
  });
});

describe("parseCustomCommand", () => {
  it("拒绝非对象、缺 id、空命令", () => {
    expect(parseCustomCommand(null)).toBeNull();
    expect(parseCustomCommand("x")).toBeNull();
    expect(parseCustomCommand({ id: "", command: "ls" })).toBeNull();
    expect(parseCustomCommand({ id: "x", command: "   " })).toBeNull();
  });

  it("删除墓碑只保留 id", () => {
    expect(parseCustomCommand({ id: "x", name: "A", command: "ls", deleted: true })).toEqual({
      id: "x",
      name: "",
      command: "",
      group: "",
      deleted: true,
    });
  });

  it("名称缺失时取命令第一行，group 非法时为空串", () => {
    expect(parseCustomCommand({ id: "x", command: "  ls -l\npwd" })).toEqual({
      id: "x",
      command: "  ls -l\npwd",
      name: "ls -l",
      group: "",
    });
    expect(parseCustomCommand({ id: "x", command: "ls", group: 7 })).toMatchObject({ group: "" });
  });
});

describe("内置命令数据完整性", () => {
  it("命令本身是 id：不允许重复", () => {
    expect(new Set(BUILTIN.map(([, command]) => command)).size).toBe(BUILTIN.length);
  });

  it("每行的分组键都存在，中英文标题都非空", () => {
    for (const [key, , zh, en] of BUILTIN) {
      expect(GROUPS[key]).toBeTruthy();
      expect(zh.trim().length).toBeGreaterThan(0);
      expect(en.trim().length).toBeGreaterThan(0);
    }
  });

  it("占位符只允许 <UPPER_CASE> 形式；其余 < 只能是 shell 重定向", () => {
    for (const [, command] of BUILTIN) {
      // 去掉占位符后，剩余的 < 必须是重定向（后跟空格）。
      const leftovers = command.replace(/<[A-Z][A-Z0-9_]*>/g, "");
      expect(leftovers).not.toMatch(/<[^ ]/);
    }
  });

  it("旧标题映射的目标都是现存分组", () => {
    for (const target of Object.values(FORMER_TITLES)) {
      expect(GROUPS[target]).toBeTruthy();
    }
  });

  it("搜索别名结构完整：命令名非空，关键词至少含一个英文词", () => {
    for (const [cmd, words] of SEARCH_ALIASES) {
      expect(cmd.trim().length).toBeGreaterThan(0);
      expect(words).toMatch(/[a-z]/i);
    }
    // 别名表要对至少一条实际收录的命令生效。
    const usable = [...SEARCH_ALIASES.keys()].some((cmd) =>
      BUILTIN.some(([, builtin]) => builtin === cmd || builtin.startsWith(`${cmd} `)),
    );
    expect(usable).toBe(true);
  });
});
