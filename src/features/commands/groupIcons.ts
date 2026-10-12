// 命令助手里各分组的图标。
import {
  Activity, Container, Cpu, Database, Folder, FolderArchive, GitBranch, Globe, HardDrive, Inbox, KeyRound, LayoutGrid, Network, Package,
  ScrollText, ServerCog, ShieldCheck, Star, UserCog,
} from "@lucide/vue";
import type { Component } from "vue";
import { FAVORITES, UNGROUPED } from "./commands";

/** 显示全部命令的伪分组的键。 */
export const ALL = "all";

const ICONS: Record<string, Component> = {
  [ALL]: LayoutGrid,
  [FAVORITES]: Star,
  [UNGROUPED]: Inbox,
  system: Cpu,
  process: Activity,
  disk: HardDrive,
  files: FolderArchive,
  network: Network,
  ssh: KeyRound,
  service: ServerCog,
  text: ScrollText,
  users: UserCog,
  security: ShieldCheck,
  package: Package,
  container: Container,
  web: Globe,
  database: Database,
  git: GitBranch,
};

/** 分组键对应的图标；用户自己的分组统一用文件夹。 */
export const groupIcon = (key: string): Component => ICONS[key] ?? Folder;
