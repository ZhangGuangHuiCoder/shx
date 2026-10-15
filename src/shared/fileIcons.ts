import {
  File, FileArchive, FileAudio, FileCode, FileCog, FileImage, FileJson, FileKey,
  FileSpreadsheet, FileSymlink, FileTerminal, FileText, FileType, FileVideo,
  Folder, FolderSymlink,
} from "@lucide/vue";
import type { Component } from "vue";
import { isArchive } from "@/shared/util";

export type FileKind = "file" | "dir" | "link" | "other";

export interface FileIconTarget {
  name: string;
  isDir?: boolean;
  kind?: FileKind;
}

export interface FileIconOptions {
  /** 未知类型按文本文件显示。 */
  text?: boolean;
}

export interface FileIconInfo {
  component: Component;
  color: string;
}

// 按大类上色，同类内靠图标形状区分。
const COLOR = {
  folder: "text-blue-500",
  code: "text-emerald-500",
  media: "text-violet-500",
  archive: "text-pink-500",
  plain: "text-zinc-500 dark:text-zinc-400",
} as const;

const FOLDER: FileIconInfo = { component: Folder, color: COLOR.folder };
const LINK_DIR: FileIconInfo = { component: FolderSymlink, color: COLOR.folder };
const LINK_FILE: FileIconInfo = { component: FileSymlink, color: COLOR.plain };
const ARCHIVE: FileIconInfo = { component: FileArchive, color: COLOR.archive };

/** 按完整文件名识别的特殊文件。 */
const SPECIAL: Record<string, FileIconInfo> = {
  dockerfile: { component: FileCode, color: COLOR.code },
  containerfile: { component: FileCode, color: COLOR.code },
  makefile: { component: FileTerminal, color: COLOR.code },
  gnumakefile: { component: FileTerminal, color: COLOR.code },
  "cmakelists.txt": { component: FileCode, color: COLOR.code },
  rakefile: { component: FileCode, color: COLOR.code },
  gemfile: { component: FileCode, color: COLOR.code },
  ".gitignore": { component: FileCog, color: COLOR.plain },
  ".gitattributes": { component: FileCog, color: COLOR.plain },
  ".gitmodules": { component: FileCog, color: COLOR.plain },
  ".editorconfig": { component: FileCog, color: COLOR.plain },
  ".npmignore": { component: FileCog, color: COLOR.plain },
  license: { component: FileText, color: COLOR.plain },
  licence: { component: FileText, color: COLOR.plain },
};

/** 扩展名（小写，不带点）→ 图标。 */
const BY_EXT: Record<string, FileIconInfo> = {};
const group = (info: FileIconInfo, exts: string[]) => {
  for (const ext of exts) BY_EXT[ext] = info;
};

group({ component: FileCode, color: COLOR.code }, [
  "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "vue", "svelte",
  "py", "pyw", "rb", "rs", "go", "java", "kt", "kts", "groovy",
  "c", "h", "cpp", "cc", "cxx", "hpp", "hh", "cs", "php", "phtml",
  "swift", "m", "mm", "scala", "lua", "pl", "pm", "r", "jl",
  "dart", "ex", "exs", "clj", "cljs", "hs", "erl", "lisp", "lsp",
  "asm", "s", "f", "f90", "sql", "graphql", "gql", "prisma", "proto",
  "html", "htm", "xhtml", "css", "scss", "sass", "less", "styl",
]);
group({ component: FileJson, color: COLOR.plain }, ["json", "jsonc", "json5", "yaml", "yml", "toml"]);
group({ component: FileCog, color: COLOR.plain }, [
  "ini", "conf", "cfg", "config", "properties", "env", "plist", "rc",
]);
group({ component: FileText, color: COLOR.plain }, [
  "txt", "md", "markdown", "rst", "log", "nfo", "tex", "adoc", "doc", "docx", "odt", "rtf", "pdf",
]);
group({ component: FileImage, color: COLOR.media }, [
  "png", "jpg", "jpeg", "gif", "bmp", "ico", "icns", "webp", "svg",
  "avif", "tiff", "tif", "heic", "heif", "raw", "psd", "ai", "sketch",
]);
group({ component: FileAudio, color: COLOR.media }, [
  "mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "wma", "opus", "aiff", "aif", "mid", "midi",
]);
group({ component: FileVideo, color: COLOR.media }, [
  "mp4", "m4v", "mkv", "mov", "avi", "webm", "flv", "wmv", "mpg", "mpeg", "3gp",
]);
group({ component: FileArchive, color: COLOR.archive }, [
  "7z", "rar", "zst", "lzma", "jar", "war", "deb", "rpm", "apk", "pkg", "dmg", "iso",
]);
group({ component: FileSpreadsheet, color: COLOR.plain }, ["csv", "tsv", "xls", "xlsx", "ods", "numbers"]);
group({ component: FileType, color: COLOR.media }, ["ttf", "otf", "woff", "woff2", "eot", "fon"]);
group({ component: FileKey, color: COLOR.plain }, [
  "key", "pem", "crt", "cer", "der", "pub", "p12", "pfx", "gpg", "asc", "keystore", "jks", "sig",
]);
group({ component: FileTerminal, color: COLOR.code }, [
  "sh", "bash", "zsh", "fish", "ps1", "bat", "cmd", "command",
]);

const DEFAULT_ICON: FileIconInfo = { component: File, color: COLOR.plain };

/** 判定顺序：链接 → 目录 → 特殊文件名 → 压缩包 → 扩展名 → 默认。 */
export function fileIcon(target: FileIconTarget, opts: FileIconOptions = {}): FileIconInfo {
  const { name, isDir = false, kind = "file" } = target;

  if (kind === "link") return isDir ? LINK_DIR : LINK_FILE;
  if (isDir || kind === "dir") return FOLDER;
  if (kind === "other") return { component: FileCog, color: COLOR.plain };

  const lower = name.toLowerCase();
  if (SPECIAL[lower]) return SPECIAL[lower];
  if (isArchive(name)) return ARCHIVE;

  const dot = lower.lastIndexOf(".");
  if (dot >= 0) {
    const hit = BY_EXT[lower.slice(dot + 1)];
    if (hit) return hit;
  }

  return opts.text ? { component: FileText, color: COLOR.plain } : DEFAULT_ICON;
}
