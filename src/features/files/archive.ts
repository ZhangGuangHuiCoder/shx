// “添加到压缩包”对话框提供的压缩格式（服务器能打包的格式见 sftp.rs）及各自支持的级别。
// 不依赖框架，直接用 `node --test` 做单元测试。

/** 压缩强度；各格式支持其中的哪些并不相同。 */
export type Level = "store" | "fast" | "normal" | "max";

export interface ArchiveFormat {
  /** 选定该格式的扩展名，也是列表中显示的名称。 */
  ext: string;
  /** 表示同一格式的其他扩展名。 */
  aliases: string[];
  levels: Level[];
}

/** 全部级别，从不压缩到压缩得最狠。 */
export const LEVELS: Level[] = ["store", "fast", "normal", "max"];

const COMPRESSED: Level[] = ["fast", "normal", "max"];
const TAR_GZ: ArchiveFormat = { ext: ".tar.gz", aliases: [".tgz"], levels: COMPRESSED };

export const FORMATS: ArchiveFormat[] = [
  { ext: ".zip", aliases: [], levels: LEVELS },
  { ext: ".tar", aliases: [], levels: ["store"] },
  TAR_GZ,
];

/** 用户没有选过格式时使用的格式。 */
export const DEFAULT_FORMAT = TAR_GZ;

/** `name` 结尾的扩展名（如果它属于某种格式）。 */
function extensionOf(name: string): { format: ArchiveFormat; ext: string } | null {
  const lower = name.toLowerCase();
  for (const format of FORMATS) {
    const ext = [format.ext, ...format.aliases].find((e) => lower.endsWith(e) && lower.length > e.length);
    if (ext) return { format, ext };
  }
  return null;
}

/**
 * 把压缩包名称拆成扩展名之前的部分和扩展名所代表的格式。扩展名不认识时名称整体保留，
 * 格式为 null："v1.2" 不是 "v1" 加 ".2"。
 */
export function splitName(name: string): { stem: string; format: ArchiveFormat | null } {
  const known = extensionOf(name);
  return known ? { stem: name.slice(0, -known.ext.length), format: known.format } : { stem: name, format: null };
}

/** 该格式的扩展名后面是否还能接着写成另一种格式：".tar" 之后也许是 ".gz"。 */
export function mayGrow(format: ArchiveFormat): boolean {
  return FORMATS.some((other) => other.ext.startsWith(`${format.ext}.`));
}

/** 格式不支持想要的级别时，退而使用的级别。 */
export function levelFor(format: ArchiveFormat, wanted: Level): Level {
  return format.levels.includes(wanted) ? wanted : format.levels.includes("normal") ? "normal" : format.levels[0];
}

/** 级别对应的、传给工具的数字；null 表示让工具使用自己的默认值。 */
export function levelNumber(level: Level): number | null {
  return { store: 0, fast: 1, normal: null, max: 9 }[level];
}
