// 主机卡片可选的图标，全部来自 lucide。逐个显式导入组件，打包时才能摇树；
// 会话配置里保存的是图标的 kebab 名称。
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
import type { Locale, MessageKey } from "../../shared/i18n";

export interface HostIconGroup {
  label: MessageKey;
  icons: string[];
}

/**
 * 选择器里提供的图标，按“这台主机是做什么的”归类，后面是它的中文名（用于提示和搜索）。
 * 每组 7 个或 14 个：选择器一行放 7 个，这样每组都是整行。
 */
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

/**
 * 名称 → 组件。除了选择器里提供的，还包括以前提供过、现在不再列出的图标：
 * 已经选了它们的主机照常显示。
 */
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
    // 数字前同样断开：Building2 → building-2。
    pascal.replace(/[A-Z]|\d+/g, (m, i) => (i ? "-" : "") + m.toLowerCase()),
    component,
  ]),
);

/** 把保存的图标名解析为组件；返回 null 时退回显示首字符。 */
export function resolveHostIcon(name?: string | null): Component | null {
  return name ? COMPONENTS.get(name) ?? null : null;
}

/** 图标在当前语言下的名字：中文名，或 "hard-drive" → "hard drive"。 */
export function hostIconLabel(name: string, locale: Locale): string {
  return (locale === "zh" && ZH.get(name)) || name.replace(/-/g, " ");
}

/** 图标是否匹配搜索词（小写）：英文名和中文名都算。 */
export function hostIconMatches(name: string, query: string): boolean {
  return name.includes(query) || name.replace(/-/g, " ").includes(query) || !!ZH.get(name)?.includes(query);
}

/** 没有自定义颜色的主机所用的色相，由它的 id 算出，固定不变。 */
export function hostHue(id: string): number {
  let h = 0;
  for (const ch of id) h = (h * 31 + ch.charCodeAt(0)) % 360;
  return h;
}
