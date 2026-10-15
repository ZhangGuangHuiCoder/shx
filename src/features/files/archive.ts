// 压缩格式及其支持的级别（服务器端支持的格式见 sftp.rs）。

/** 压缩级别，各格式支持的不同。 */
export type Level = "store" | "fast" | "normal" | "max";

export interface ArchiveFormat {
  /** 主扩展名，也用作显示名。 */
  ext: string;
  aliases: string[];
  levels: Level[];
}

/** 从不压缩到最高压缩。 */
export const LEVELS: Level[] = ["store", "fast", "normal", "max"];

const COMPRESSED: Level[] = ["fast", "normal", "max"];
const TAR_GZ: ArchiveFormat = { ext: ".tar.gz", aliases: [".tgz"], levels: COMPRESSED };

export const FORMATS: ArchiveFormat[] = [
  { ext: ".zip", aliases: [], levels: LEVELS },
  { ext: ".tar", aliases: [], levels: ["store"] },
  TAR_GZ,
];

export const DEFAULT_FORMAT = TAR_GZ;

function extensionOf(name: string): { format: ArchiveFormat; ext: string } | null {
  const lower = name.toLowerCase();
  for (const format of FORMATS) {
    const ext = [format.ext, ...format.aliases].find((e) => lower.endsWith(e) && lower.length > e.length);
    if (ext) return { format, ext };
  }
  return null;
}

/** 拆分出主名和格式；扩展名不识别时整体保留，格式为 null。 */
export function splitName(name: string): { stem: string; format: ArchiveFormat | null } {
  const known = extensionOf(name);
  return known ? { stem: name.slice(0, -known.ext.length), format: known.format } : { stem: name, format: null };
}

/** 扩展名能否继续延长成另一格式（如 .tar → .tar.gz）。 */
export function mayGrow(format: ArchiveFormat): boolean {
  return FORMATS.some((other) => other.ext.startsWith(`${format.ext}.`));
}

/** 格式不支持所需级别时的回退。 */
export function levelFor(format: ArchiveFormat, wanted: Level): Level {
  return format.levels.includes(wanted) ? wanted : format.levels.includes("normal") ? "normal" : format.levels[0];
}

/** 传给压缩工具的数值；null 表示工具默认值。 */
export function levelNumber(level: Level): number | null {
  return { store: 0, fast: 1, normal: null, max: 9 }[level];
}
