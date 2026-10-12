// 命令助手的命令目录：随应用提供的默认命令和用户保存的命令合在一起，不作区分，
// 每一条都可以修改和删除。不依赖框架，可以直接用 `node --test` 做单元测试。
import { BUILTIN, FORMER_TITLES, GROUPS, SEARCH_ALIASES, type GroupKey, type Text } from "./builtinCommands.ts";

/**
 * 保存下来的一条命令：用户新建的，或者对某条默认命令的修改（`id` 与默认命令相同）。
 */
export interface CustomCommand {
  id: string;
  name: string;
  command: string;
  /** 所属分组的标题；为空表示未分组。 */
  group: string;
  /** 仅用于默认命令：用户把它删掉了。 */
  deleted?: boolean;
}

/** 命令助手中列出的一条命令。 */
export interface CommandItem {
  id: string;
  name: string;
  command: string;
  /** 所属分组的标题；未分组时为空。 */
  group: string;
}

export interface CommandGroup {
  /** 默认分组是固定的键（如 `system`），用户的分组是 `custom:标题`，未分组是 {@link UNGROUPED}。 */
  key: string;
  title: string;
  items: CommandItem[];
}

/** 未分组命令所在分组的键。 */
export const UNGROUPED = "ungrouped";
/** 收藏的命令组成的伪分组的键。 */
export const FAVORITES = "favorites";

type Lang = keyof Text;

const GROUP_KEYS = Object.keys(GROUPS) as GroupKey[];
const builtinId = (command: string) => `builtin:${command}`;
const BUILTIN_IDS = new Set(BUILTIN.map(([, command]) => builtinId(command)));

/** 标题对应的分组键。默认分组的标题不论用哪种语言写、是现在的还是以前用过的，都归到同一个分组。 */
export function groupKey(title: string): string {
  const name = title.trim().toLowerCase();
  if (!name) return UNGROUPED;
  const known = GROUP_KEYS.find((key) => Object.values(GROUPS[key]).some((text) => text.toLowerCase() === name));
  if (known) return known;
  return Object.keys(FORMER_TITLES).includes(name) ? FORMER_TITLES[name] : `custom:${title.trim()}`;
}

/**
 * 命令助手里的全部命令，按分组归类：默认命令（被修改过的以修改后的为准，被删除的不列出），
 * 加上用户新建的命令。未分组的排在最前，然后是用户自己的分组（按首次出现的顺序），
 * 最后是默认分组。
 */
export function commandGroups(saved: CustomCommand[], lang: Lang, ungrouped: string): CommandGroup[] {
  const byId = new Map(saved.map((c) => [c.id, c]));
  const titleOf = (key: string, typed: string) => (key === UNGROUPED ? ungrouped : key in GROUPS ? GROUPS[key as GroupKey][lang] : typed.trim());
  const groups = new Map<string, CommandGroup>();
  const add = (id: string, name: string, command: string, typed: string) => {
    const key = groupKey(typed);
    const group = groups.get(key) ?? { key, title: titleOf(key, typed), items: [] };
    group.items.push({ id, name, command, group: key === UNGROUPED ? "" : group.title });
    groups.set(key, group);
  };

  for (const [key, command, zh, en] of BUILTIN) {
    const mine = byId.get(builtinId(command));
    if (mine?.deleted) continue;
    if (mine) add(mine.id, mine.name, mine.command, mine.group);
    else add(builtinId(command), lang === "zh" ? zh : en, command, GROUPS[key][lang]);
  }
  for (const c of saved) if (!BUILTIN_IDS.has(c.id) && !c.deleted) add(c.id, c.name, c.command, c.group);

  const rank = (key: string) => (key === UNGROUPED ? 0 : key in GROUPS ? 2 : 1);
  return [...groups.values()].sort((a, b) => rank(a.key) - rank(b.key) || GROUP_KEYS.indexOf(a.key as GroupKey) - GROUP_KEYS.indexOf(b.key as GroupKey));
}

/** 删除一条命令。默认命令留下一条“已删除”的记录，否则它会重新出现。 */
export function removeCommand(saved: CustomCommand[], id: string): CustomCommand[] {
  const rest = saved.filter((c) => c.id !== id);
  return BUILTIN_IDS.has(id) ? [...rest, { id, name: "", command: "", group: "", deleted: true }] : rest;
}

/** 是否有默认命令被修改或删除过。 */
export function defaultsChanged(saved: CustomCommand[]): boolean {
  return saved.some((c) => BUILTIN_IDS.has(c.id));
}

/** 把默认命令恢复原样：撤销对它们的修改和删除，用户新建的命令不受影响。 */
export function restoreDefaults(saved: CustomCommand[]): CustomCommand[] {
  return saved.filter((c) => !BUILTIN_IDS.has(c.id));
}

/** 收藏一条命令，已收藏的则取消；新收藏的排在最后。 */
export function toggleFavorite(favorites: string[], id: string): string[] {
  return favorites.includes(id) ? favorites.filter((item) => item !== id) : [...favorites, id];
}

/**
 * `groups` 里被收藏的命令组成的分组，按收藏的先后排列；一条也没有时返回 null。
 * 收藏里记的命令已经不在了（被删除，或者被搜索滤掉）的，不列出。
 */
export function favoriteGroup(groups: CommandGroup[], favorites: string[], title: string): CommandGroup | null {
  const byId = new Map(groups.flatMap((group) => group.items).map((item) => [item.id, item]));
  const items = [...new Set(favorites)].flatMap((id) => byId.get(id) ?? []);
  return items.length ? { key: FAVORITES, title, items } : null;
}

/** 把搜索词拆成小写的词。 */
export function searchWords(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter(Boolean);
}

/** 保留名称、内容、分组或别名中包含 `query` 每个词的命令。 */
export function filterGroups(groups: CommandGroup[], query: string): CommandGroup[] {
  const words = searchWords(query);
  if (!words.length) return groups;
  return groups
    .map((group) => ({
      ...group,
      items: group.items.filter((item) => {
        const alias = SEARCH_ALIASES.get(item.command.trim().split(/\s+/)[0]) ?? "";
        const text = `${item.name} ${item.command} ${group.title} ${alias}`.toLowerCase();
        return words.every((word) => text.includes(word));
      }),
    }))
    .filter((group) => group.items.length);
}

const PLACEHOLDER = /<[A-Z][A-Z0-9_]*>/;

/**
 * 命令中是否还有待填写的 `<PART>`。这样的命令只会填到命令行上，绝不原样执行：
 * 在 shell 看来，`<PID>` 的意思是“从文件 PID 读取输入”。
 */
export function hasPlaceholder(command: string): boolean {
  return PLACEHOLDER.test(command);
}

/** 显示用的一段文字：`placeholder` 是待填写的部分，`hit` 是搜索命中的部分。 */
export interface TextPart {
  text: string;
  placeholder?: boolean;
  hit?: boolean;
}

/** 把文字按搜索词切开，标出命中的部分（不区分大小写）。 */
export function markHits(text: string, words: string[]): TextPart[] {
  const lower = text.toLowerCase();
  const hit = new Array<boolean>(text.length).fill(false);
  for (const word of words) {
    for (let at = lower.indexOf(word); word && at >= 0; at = lower.indexOf(word, at + word.length)) hit.fill(true, at, at + word.length);
  }
  const parts: TextPart[] = [];
  for (let i = 0; i < text.length; i++) {
    const last = parts[parts.length - 1];
    if (last && !!last.hit === hit[i]) last.text += text[i];
    else parts.push(hit[i] ? { text: text[i], hit: true } : { text: text[i] });
  }
  return parts;
}

/** 把命令切成普通文字和待填写的 `<PART>`，普通文字里再标出搜索命中的部分。 */
export function commandParts(command: string, words: string[] = []): TextPart[] {
  return command
    .split(new RegExp(`(${PLACEHOLDER.source})`))
    .flatMap((piece) => (PLACEHOLDER.test(piece) ? [{ text: piece, placeholder: true }] : markHits(piece, words)));
}

/** 从不可信的数据（本地存储、备份）中读取保存的命令；不是命令时返回 null。 */
export function parseCustomCommand(v: unknown): CustomCommand | null {
  if (typeof v !== "object" || v === null) return null;
  const { id, name, command, group, deleted } = v as Record<string, unknown>;
  if (typeof id !== "string" || !id) return null;
  if (deleted === true) return { id, name: "", command: "", group: "", deleted: true };
  if (typeof command !== "string" || !command.trim()) return null;
  return {
    id,
    command,
    name: typeof name === "string" && name.trim() ? name : command.trim().split("\n")[0],
    group: typeof group === "string" ? group : "",
  };
}
