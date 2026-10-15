// 主机图标（lucide），逐个导入以便摇树；配置中保存 kebab 名称。
import {
  Activity, Anchor, Archive, Banknote, Beer, Bird, Blocks, Bookmark, Bot, Box, Boxes, Braces,
  BrickWall, Briefcase, Bug, Building, Building2, Cable, Camera, Car, Cat, Cloud,
  CloudCog, Code, Code2, Cog, Coffee, Compass, Container, Coins, Cpu, CreditCard,
  Crown, Database, Dog, Factory, File, Fingerprint, Fish, Flag, FlaskConical, Flower,
  Flower2, Gamepad2, Gem, Ghost, Gift, GitBranch, Globe, GraduationCap, Hammer, HardDrive, Heart,
  Home, Image, KeyRound, Landmark, Layers, Laptop, Leaf, Lock, Mail, MapPin, MessageSquare,
  Monitor, MonitorCheck, Moon, Mountain, Music, Network, Package, PackageOpen, Palette,
  Phone, Pizza, Plane, Plug, PlugZap, Rocket, Router, ScanFace, Send, Server, Settings,
  Shield, ShieldCheck, Ship, SquareTerminal, Star, Store, Sun, Terminal, Ticket, Train,
  TreePine, User, Users, Video, Waypoints, Wifi, Wrench, Zap,
} from "@lucide/vue";
import type { Component } from "vue";
import type { Locale, MessageKey } from "@/shared/i18n";

export interface HostIconGroup {
  label: MessageKey;
  icons: string[];
}

/** 按用途分组的可选图标及中文名。每组 7 或 14 个，正好占满整行。 */
const CATALOG: { label: MessageKey; icons: [name: string, zh: string][] }[] = [
  {
    label: "icon.group.server",
    icons: [
      ["server", "服务器"], ["cloud", "云主机"], ["cloud-cog", "云服务"], ["container", "容器"], ["boxes", "集群"],
      ["box", "节点"], ["blocks", "微服务"], ["layers", "中间层"], ["cpu", "计算"], ["monitor", "桌面主机"],
      ["laptop", "笔记本"], ["database", "数据库"], ["hard-drive", "存储"], ["archive", "备份归档"],
    ],
  },
  {
    label: "icon.group.network",
    icons: [
      ["network", "内网"], ["globe", "公网站点"], ["router", "路由器"], ["wifi", "无线"], ["waypoints", "网关代理"],
      ["cable", "专线"], ["plug", "接入"], ["plug-zap", "电源"], ["brick-wall", "防火墙"], ["shield", "防护"],
      ["shield-check", "安全加固"], ["lock", "加密"], ["key-round", "密钥"], ["fingerprint", "认证"],
    ],
  },
  {
    label: "icon.group.dev",
    icons: [
      ["terminal", "终端"], ["square-terminal", "跳板机"], ["code", "代码"], ["braces", "接口"], ["git-branch", "代码仓库"],
      ["package", "制品"], ["package-open", "发布"], ["bug", "调试"], ["flask-conical", "测试"], ["rocket", "生产"],
      ["wrench", "运维"], ["cog", "任务调度"], ["activity", "监控"], ["monitor-check", "巡检"],
    ],
  },
  {
    label: "icon.group.apps",
    icons: [
      ["mail", "邮件"], ["send", "消息推送"], ["message-square", "聊天"], ["phone", "语音"], ["video", "视频"],
      ["music", "音乐"], ["image", "图片"], ["camera", "监控摄像"], ["gamepad-2", "游戏"], ["credit-card", "支付"],
      ["bot", "机器人"], ["users", "社区"], ["briefcase", "办公"], ["file", "文件服务"],
    ],
  },
  {
    label: "icon.group.places",
    icons: [
      ["home", "家里"], ["building", "办公室"], ["building-2", "机房"], ["factory", "工厂"], ["store", "门店"],
      ["landmark", "机构"], ["map-pin", "地域"],
    ],
  },
  {
    label: "icon.group.marks",
    icons: [["star", "星标"], ["heart", "喜欢"], ["bookmark", "书签"], ["flag", "旗标"], ["crown", "主力"], ["gem", "珍贵"], ["zap", "高性能"]],
  },
];

export const HOST_ICON_GROUPS: HostIconGroup[] = CATALOG.map(({ label, icons }) => ({ label, icons: icons.map(([name]) => name) }));

const ZH = new Map(CATALOG.flatMap(({ icons }) => icons));

/** 名称 → 组件，含已下架的旧图标以兼容已有配置。 */
const COMPONENTS = new Map<string, Component>(
  Object.entries({
    Activity, Anchor, Archive, Banknote, Beer, Bird, Blocks, Bookmark, Bot, Box, Boxes, Braces,
    BrickWall, Briefcase, Bug, Building, Building2, Cable, Camera, Car, Cat, Cloud,
    CloudCog, Code, Code2, Cog, Coffee, Compass, Container, Coins, Cpu, CreditCard,
    Crown, Database, Dog, Factory, File, Fingerprint, Fish, Flag, FlaskConical, Flower,
    Flower2, Gamepad2, Gem, Ghost, Gift, GitBranch, Globe, GraduationCap, Hammer, HardDrive, Heart,
    Home, Image, KeyRound, Landmark, Layers, Laptop, Leaf, Lock, Mail, MapPin, MessageSquare,
    Monitor, MonitorCheck, Moon, Mountain, Music, Network, Package, PackageOpen, Palette,
    Phone, Pizza, Plane, Plug, PlugZap, Rocket, Router, ScanFace, Send, Server, Settings,
    Shield, ShieldCheck, Ship, SquareTerminal, Star, Store, Sun, Terminal, Ticket, Train,
    TreePine, User, Users, Video, Waypoints, Wifi, Wrench, Zap,
  }).map(([pascal, component]) => [
    // Building2 → building-2
    pascal.replace(/[A-Z]|\d+/g, (m, i) => (i ? "-" : "") + m.toLowerCase()),
    component,
  ]),
);

/** 返回 null 时显示首字符。 */
export function resolveHostIcon(name?: string | null): Component | null {
  return name ? COMPONENTS.get(name) ?? null : null;
}

/** 当前语言下的图标名。 */
export function hostIconLabel(name: string, locale: Locale): string {
  return (locale === "zh" && ZH.get(name)) || name.replace(/-/g, " ");
}

/** 按中英文名匹配小写搜索词。 */
export function hostIconMatches(name: string, query: string): boolean {
  return name.includes(query) || name.replace(/-/g, " ").includes(query) || !!ZH.get(name)?.includes(query);
}

/** 由 id 推算的默认色相。 */
export function hostHue(id: string): number {
  let h = 0;
  for (const ch of id) h = (h * 31 + ch.charCodeAt(0)) % 360;
  return h;
}

/** 主机配色：保存的色相映射到最接近的 Tailwind 颜色。类名须为完整字面量。 */
export interface HostTint {
  /** 代表色相，选中时保存此值。 */
  hue: number;
  swatch: string;
  /** 头像渐变与投影。 */
  avatar: string;
  text: string;
  chip: string;
  /** 卡片左上角的光晕。 */
  glow: string;
  /** 连接中的轮廓。 */
  line: string;
  /** 悬停、聚焦或菜单打开时的彩色投影。 */
  hover: string;
}

export const HOST_TINTS: HostTint[] = [
  {
    hue: 0,
    swatch: "bg-red-500",
    avatar: "from-red-400 to-red-600 shadow-red-500/40",
    text: "text-red-600 dark:text-red-400",
    chip: "bg-red-500/15 text-red-700 dark:text-red-300",
    glow: "from-red-500/10",
    line: "border-red-400/60",
    hover: "hover:shadow-red-500/20 focus-within:shadow-red-500/20 [&.card-menu-open]:shadow-red-500/20",
  },
  {
    hue: 25,
    swatch: "bg-orange-500",
    avatar: "from-orange-400 to-orange-600 shadow-orange-500/40",
    text: "text-orange-600 dark:text-orange-400",
    chip: "bg-orange-500/15 text-orange-700 dark:text-orange-300",
    glow: "from-orange-500/10",
    line: "border-orange-400/60",
    hover: "hover:shadow-orange-500/20 focus-within:shadow-orange-500/20 [&.card-menu-open]:shadow-orange-500/20",
  },
  {
    hue: 40,
    swatch: "bg-amber-500",
    avatar: "from-amber-400 to-amber-600 shadow-amber-500/40",
    text: "text-amber-600 dark:text-amber-400",
    chip: "bg-amber-500/15 text-amber-700 dark:text-amber-300",
    glow: "from-amber-500/10",
    line: "border-amber-400/60",
    hover: "hover:shadow-amber-500/20 focus-within:shadow-amber-500/20 [&.card-menu-open]:shadow-amber-500/20",
  },
  {
    hue: 52,
    swatch: "bg-yellow-500",
    avatar: "from-yellow-400 to-yellow-600 shadow-yellow-500/40",
    text: "text-yellow-600 dark:text-yellow-400",
    chip: "bg-yellow-500/15 text-yellow-700 dark:text-yellow-300",
    glow: "from-yellow-500/10",
    line: "border-yellow-400/60",
    hover: "hover:shadow-yellow-500/20 focus-within:shadow-yellow-500/20 [&.card-menu-open]:shadow-yellow-500/20",
  },
  {
    hue: 80,
    swatch: "bg-lime-500",
    avatar: "from-lime-400 to-lime-600 shadow-lime-500/40",
    text: "text-lime-600 dark:text-lime-400",
    chip: "bg-lime-500/15 text-lime-700 dark:text-lime-300",
    glow: "from-lime-500/10",
    line: "border-lime-400/60",
    hover: "hover:shadow-lime-500/20 focus-within:shadow-lime-500/20 [&.card-menu-open]:shadow-lime-500/20",
  },
  {
    hue: 140,
    swatch: "bg-green-500",
    avatar: "from-green-400 to-green-600 shadow-green-500/40",
    text: "text-green-600 dark:text-green-400",
    chip: "bg-green-500/15 text-green-700 dark:text-green-300",
    glow: "from-green-500/10",
    line: "border-green-400/60",
    hover: "hover:shadow-green-500/20 focus-within:shadow-green-500/20 [&.card-menu-open]:shadow-green-500/20",
  },
  {
    hue: 155,
    swatch: "bg-emerald-500",
    avatar: "from-emerald-400 to-emerald-600 shadow-emerald-500/40",
    text: "text-emerald-600 dark:text-emerald-400",
    chip: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300",
    glow: "from-emerald-500/10",
    line: "border-emerald-400/60",
    hover: "hover:shadow-emerald-500/20 focus-within:shadow-emerald-500/20 [&.card-menu-open]:shadow-emerald-500/20",
  },
  {
    hue: 172,
    swatch: "bg-teal-500",
    avatar: "from-teal-400 to-teal-600 shadow-teal-500/40",
    text: "text-teal-600 dark:text-teal-400",
    chip: "bg-teal-500/15 text-teal-700 dark:text-teal-300",
    glow: "from-teal-500/10",
    line: "border-teal-400/60",
    hover: "hover:shadow-teal-500/20 focus-within:shadow-teal-500/20 [&.card-menu-open]:shadow-teal-500/20",
  },
  {
    hue: 188,
    swatch: "bg-cyan-500",
    avatar: "from-cyan-400 to-cyan-600 shadow-cyan-500/40",
    text: "text-cyan-600 dark:text-cyan-400",
    chip: "bg-cyan-500/15 text-cyan-700 dark:text-cyan-300",
    glow: "from-cyan-500/10",
    line: "border-cyan-400/60",
    hover: "hover:shadow-cyan-500/20 focus-within:shadow-cyan-500/20 [&.card-menu-open]:shadow-cyan-500/20",
  },
  {
    hue: 200,
    swatch: "bg-sky-500",
    avatar: "from-sky-400 to-sky-600 shadow-sky-500/40",
    text: "text-sky-600 dark:text-sky-400",
    chip: "bg-sky-500/15 text-sky-700 dark:text-sky-300",
    glow: "from-sky-500/10",
    line: "border-sky-400/60",
    hover: "hover:shadow-sky-500/20 focus-within:shadow-sky-500/20 [&.card-menu-open]:shadow-sky-500/20",
  },
  {
    hue: 217,
    swatch: "bg-blue-500",
    avatar: "from-blue-400 to-blue-600 shadow-blue-500/40",
    text: "text-blue-600 dark:text-blue-400",
    chip: "bg-blue-500/15 text-blue-700 dark:text-blue-300",
    glow: "from-blue-500/10",
    line: "border-blue-400/60",
    hover: "hover:shadow-blue-500/20 focus-within:shadow-blue-500/20 [&.card-menu-open]:shadow-blue-500/20",
  },
  {
    hue: 238,
    swatch: "bg-indigo-500",
    avatar: "from-indigo-400 to-indigo-600 shadow-indigo-500/40",
    text: "text-indigo-600 dark:text-indigo-400",
    chip: "bg-indigo-500/15 text-indigo-700 dark:text-indigo-300",
    glow: "from-indigo-500/10",
    line: "border-indigo-400/60",
    hover: "hover:shadow-indigo-500/20 focus-within:shadow-indigo-500/20 [&.card-menu-open]:shadow-indigo-500/20",
  },
  {
    hue: 258,
    swatch: "bg-violet-500",
    avatar: "from-violet-400 to-violet-600 shadow-violet-500/40",
    text: "text-violet-600 dark:text-violet-400",
    chip: "bg-violet-500/15 text-violet-700 dark:text-violet-300",
    glow: "from-violet-500/10",
    line: "border-violet-400/60",
    hover: "hover:shadow-violet-500/20 focus-within:shadow-violet-500/20 [&.card-menu-open]:shadow-violet-500/20",
  },
  {
    hue: 272,
    swatch: "bg-purple-500",
    avatar: "from-purple-400 to-purple-600 shadow-purple-500/40",
    text: "text-purple-600 dark:text-purple-400",
    chip: "bg-purple-500/15 text-purple-700 dark:text-purple-300",
    glow: "from-purple-500/10",
    line: "border-purple-400/60",
    hover: "hover:shadow-purple-500/20 focus-within:shadow-purple-500/20 [&.card-menu-open]:shadow-purple-500/20",
  },
  {
    hue: 292,
    swatch: "bg-fuchsia-500",
    avatar: "from-fuchsia-400 to-fuchsia-600 shadow-fuchsia-500/40",
    text: "text-fuchsia-600 dark:text-fuchsia-400",
    chip: "bg-fuchsia-500/15 text-fuchsia-700 dark:text-fuchsia-300",
    glow: "from-fuchsia-500/10",
    line: "border-fuchsia-400/60",
    hover: "hover:shadow-fuchsia-500/20 focus-within:shadow-fuchsia-500/20 [&.card-menu-open]:shadow-fuchsia-500/20",
  },
  {
    hue: 328,
    swatch: "bg-pink-500",
    avatar: "from-pink-400 to-pink-600 shadow-pink-500/40",
    text: "text-pink-600 dark:text-pink-400",
    chip: "bg-pink-500/15 text-pink-700 dark:text-pink-300",
    glow: "from-pink-500/10",
    line: "border-pink-400/60",
    hover: "hover:shadow-pink-500/20 focus-within:shadow-pink-500/20 [&.card-menu-open]:shadow-pink-500/20",
  },
  {
    hue: 348,
    swatch: "bg-rose-500",
    avatar: "from-rose-400 to-rose-600 shadow-rose-500/40",
    text: "text-rose-600 dark:text-rose-400",
    chip: "bg-rose-500/15 text-rose-700 dark:text-rose-300",
    glow: "from-rose-500/10",
    line: "border-rose-400/60",
    hover: "hover:shadow-rose-500/20 focus-within:shadow-rose-500/20 [&.card-menu-open]:shadow-rose-500/20",
  },
];

/** 预计算每个整数色相最近的颜色。 */
const TINT_BY_HUE = Array.from({ length: 360 }, (_, hue) => {
  const distance = (a: number) => Math.min(Math.abs(a - hue), 360 - Math.abs(a - hue));
  return HOST_TINTS.reduce((best, tint) => (distance(tint.hue) < distance(best.hue) ? tint : best));
});

export function hostTint(hue: number): HostTint {
  return TINT_BY_HUE[((Math.round(hue) % 360) + 360) % 360];
}
