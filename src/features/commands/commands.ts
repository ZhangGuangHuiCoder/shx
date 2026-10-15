// 命令目录：默认命令与用户命令合并，均可修改和删除（纯函数）。
import { BUILTIN, FORMER_TITLES, GROUPS, SEARCH_ALIASES, type GroupKey, type Text } from "@/features/commands/builtinCommands.ts";

/** 用户新建的命令，或对默认命令的修改（id 与默认命令相同）。 */
export interface CustomCommand {
  id: string;
  name: string;
  command: string;
  /** 分组标题，空为未分组。 */
  group: string;
  /** 默认命令被删除的标记。 */
  deleted?: boolean;
}

export interface CommandItem {
  id: string;
  name: string;
  command: string;
  group: string;
}

export interface CommandGroup {
  /** 默认分组为固定键，用户分组为 `custom:标题`，未分组为 UNGROUPED。 */
  key: string;
  title: string;
  items: CommandItem[];
}

export const UNGROUPED = "ungrouped";
export const FAVORITES = "favorites";

type Lang = keyof Text;

const GROUP_KEYS = Object.keys(GROUPS) as GroupKey[];
const builtinId = (command: string) => `builtin:${command}`;
const BUILTIN_IDS = new Set(BUILTIN.map(([, command]) => builtinId(command)));

/** 标题 → 分组键；默认分组的各语言标题及旧标题都归到同一分组。 */
export function groupKey(title: string): string {
  const name = title.trim().toLowerCase();
  if (!name) return UNGROUPED;
  const known = GROUP_KEYS.find((key) => Object.values(GROUPS[key]).some((text) => text.toLowerCase() === name));
  if (known) return known;
  return Object.keys(FORMER_TITLES).includes(name) ? FORMER_TITLES[name] : `custom:${title.trim()}`;
}

/** 按分组归类全部命令。排序：未分组 → 用户分组（按出现顺序）→ 默认分组。 */
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

/** 删除默认命令时留下删除标记，否则会重新出现。 */
export function removeCommand(saved: CustomCommand[], id: string): CustomCommand[] {
  const rest = saved.filter((c) => c.id !== id);
  return BUILTIN_IDS.has(id) ? [...rest, { id, name: "", command: "", group: "", deleted: true }] : rest;
}

export function defaultsChanged(saved: CustomCommand[]): boolean {
  return saved.some((c) => BUILTIN_IDS.has(c.id));
}

/** 恢复默认命令，保留用户新建的命令。 */
export function restoreDefaults(saved: CustomCommand[]): CustomCommand[] {
  return saved.filter((c) => !BUILTIN_IDS.has(c.id));
}

/** 切换收藏状态。 */
export function toggleFavorite(favorites: string[], id: string): string[] {
  return favorites.includes(id) ? favorites.filter((item) => item !== id) : [...favorites, id];
}

/** 收藏分组，按收藏先后排列；无收藏时返回 null。 */
export function favoriteGroup(groups: CommandGroup[], favorites: string[], title: string): CommandGroup | null {
  const byId = new Map(groups.flatMap((group) => group.items).map((item) => [item.id, item]));
  const items = [...new Set(favorites)].flatMap((id) => byId.get(id) ?? []);
  return items.length ? { key: FAVORITES, title, items } : null;
}

export function searchWords(query: string): string[] {
  return query.toLowerCase().split(/\s+/).filter(Boolean);
}

/** 保留名称、命令、分组或别名包含全部搜索词的命令。 */
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

/** 含 `<PART>` 占位的命令只填入不执行：shell 会把 `<PID>` 当作输入重定向。 */
export function hasPlaceholder(command: string): boolean {
  return PLACEHOLDER.test(command);
}

export interface TextPart {
  text: string;
  placeholder?: boolean;
  hit?: boolean;
}

/** 按搜索词切分并标记命中（不区分大小写）。 */
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

/** 切出占位符，其余部分标记搜索命中。 */
export function commandParts(command: string, words: string[] = []): TextPart[] {
  return command
    .split(new RegExp(`(${PLACEHOLDER.source})`))
    .flatMap((piece) => (PLACEHOLDER.test(piece) ? [{ text: piece, placeholder: true }] : markHits(piece, words)));
}

/** 解析不可信数据中的命令，非法时返回 null。 */
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
