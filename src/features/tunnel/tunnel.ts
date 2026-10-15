// 隧道（TUN，仅 IPv4）设置的解析、校验与规则表逻辑（纯函数）。
import type { TunnelConfig, TunnelRule, TunnelVia } from "@/shared/api.ts";

/** 规则数上限，须与后端一致。 */
export const MAX_RULES = 256;

/** 全部地址，对应兜底规则。 */
export const EVERYTHING = "0.0.0.0/0";

export function defaultTunnelConfig(): TunnelConfig {
  return { default: "tunnel", rules: [], lan: null };
}

const DECIMAL = /^(0|[1-9]\d{0,2})$/;

export function isIPv4(text: string): boolean {
  const parts = text.split(".");
  return parts.length === 4 && parts.every((part) => DECIMAL.test(part) && Number(part) <= 255);
}

const ipv4Number = (address: string) => address.split(".").reduce((all, part) => all * 256 + Number(part), 0);

/** 规范化网段（带前缀、主机位清零）；非法时返回 null。 */
export function normalizeNetwork(text: string): string | null {
  const [address, prefix, ...rest] = text.trim().split("/");
  if (rest.length || !isIPv4(address)) return null;
  if (prefix === undefined) return `${address}/32`;
  if (!DECIMAL.test(prefix) || Number(prefix) > 32) return null;
  // 统一写法以便去重：10.1.2.3/8 → 10.0.0.0/8。
  const bits = ipv4Number(address);
  const size = 2 ** (32 - Number(prefix));
  const network = bits - (bits % size);
  return `${[24, 16, 8, 0].map((shift) => Math.floor(network / 2 ** shift) % 256).join(".")}/${Number(prefix)}`;
}

/** 补全网段：缺的段补 0，无前缀时按已填段数取 /8、/16、/24、/32。如 `10` → `10.0.0.0/8`。 */
export function completeNetwork(text: string): string {
  const [address = "", prefix = ""] = text.trim().split("/");
  const octets = address.split(".").slice(0, 4);
  while (octets.length && !octets[octets.length - 1]) octets.pop();
  if (!octets.length) return text.trim();
  return `${[0, 1, 2, 3].map((index) => octets[index] || "0").join(".")}/${prefix || 8 * octets.length}`;
}

const isVia = (value: unknown): value is TunnelVia => value === "tunnel" || value === "direct";

/** 解析保存或备份中的隧道设置，非法时返回 null。兼容旧版格式并转换为“兜底 + 规则”。 */
export function parseTunnelConfig(value: unknown): TunnelConfig | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const v = value as Record<string, unknown>;
  const rule = (net: unknown, via: TunnelVia, disabled = false): TunnelRule[] => {
    const network = typeof net === "string" ? normalizeNetwork(net) : null;
    return network ? [{ net: network, via, ...(disabled ? { disabled } : {}) }] : [];
  };
  const networks = (list: unknown, via: TunnelVia) => (Array.isArray(list) ? list.flatMap((net) => rule(net, via)) : []);
  let config: TunnelConfig;
  if (isVia(v.default) || Array.isArray(v.rules)) {
    const listed = Array.isArray(v.rules) ? v.rules : [];
    const rules = listed.flatMap((item) => {
      if (typeof item !== "object" || item === null) return [];
      const { net, via, disabled } = item as Record<string, unknown>;
      return isVia(via) ? rule(net, via, disabled === true) : [];
    });
    // 旧版的“本地网络直连”开关。
    config = { default: isVia(v.default) ? v.default : "tunnel", rules, lan: isVia(v.lan) ? v.lan : v.keepLan === true ? "direct" : null };
  } else if (v.route === "include") {
    config = { default: "direct", rules: networks(v.cidrs, "tunnel"), lan: null };
  } else {
    config = { default: "tunnel", rules: networks(v.bypass, "direct"), lan: null };
  }
  // 重复网段只保留第一条。
  const rules = config.rules.filter((rule, at) => config.rules.findIndex((other) => other.net === rule.net) === at);
  return { ...config, rules: rules.slice(0, MAX_RULES) };
}

const active = (config: TunnelConfig) => config.rules.filter((rule) => !rule.disabled);

/** 校验设置，合法时返回 null。 */
export function tunnelProblem(config: TunnelConfig): "noNetworks" | "tooManyRules" | null {
  if (config.default === "direct" && config.lan !== "tunnel" && !active(config).some((rule) => rule.via === "tunnel")) return "noNetworks";
  if (config.rules.length > MAX_RULES) return "tooManyRules";
  return null;
}

/** 发给后端的设置，去掉停用规则。 */
export function tunnelRequest(config: TunnelConfig): TunnelConfig {
  return { default: config.default, rules: active(config).map(({ net, via }) => ({ net, via })), lan: config.lan };
}

/** 规则表的行，网段为输入中的文本；表尾始终保留一个空行。 */
export interface RuleRow {
  /** 界面用的行标识。 */
  id: number;
  text: string;
  via: TunnelVia;
  enabled: boolean;
}

let rowId = 0;

export function ruleRow(via: TunnelVia, text = "", enabled = true): RuleRow {
  return { id: ++rowId, text, via, enabled };
}

export function rowsFromRules(rules: TunnelRule[]): RuleRow[] {
  const keyed = rules.map((rule) => ({ row: ruleRow(rule.via, rule.net, !rule.disabled), key: rowKey(rule.net) }));
  keyed.sort((a, b) => (a.key && b.key ? rowOrder(a.key, b.key) : 0));
  return keyed.map(({ row }) => row);
}

/** 排序键；非法网段返回 null。 */
function rowKey(text: string): { prefix: number; address: number } | null {
  const net = normalizeNetwork(text);
  if (!net) return null;
  const [address, prefix] = net.split("/");
  return { prefix: Number(prefix), address: ipv4Number(address) };
}

/** 前缀长的在前，相同则按地址。这样从上往下首个匹配即为最具体的规则。 */
const rowOrder = (a: NonNullable<ReturnType<typeof rowKey>>, b: NonNullable<ReturnType<typeof rowKey>>) => b.prefix - a.prefix || a.address - b.address;

/** 只移动这一行到正确位置，其余行不动（可能正在输入）；同网段排在已有行之后。 */
export function placedRow(rows: RuleRow[], row: RuleRow): RuleRow[] {
  const key = rowKey(row.text);
  if (!key || !rows.includes(row)) return rows;
  const rest = rows.filter((other) => other !== row);
  const later = (other: RuleRow) => {
    const otherKey = rowKey(other.text);
    return !!otherKey && rowOrder(key, otherKey) < 0;
  };
  let at = rest.findIndex(later);
  // 放在末尾空行之前。
  if (at < 0) at = rest.length && !rest[rest.length - 1].text.trim() ? rest.length - 1 : rest.length;
  return [...rest.slice(0, at), row, ...rest.slice(at)];
}

/** 行转规则，跳过空行；未完成的网段另行报告。 */
export function rulesFromRows(rows: RuleRow[]): { rules: TunnelRule[]; incomplete: boolean } {
  const rules: TunnelRule[] = [];
  let incomplete = false;
  for (const row of rows) {
    if (!row.text.trim()) continue;
    const net = normalizeNetwork(row.text);
    if (net) rules.push({ net, via: row.via, ...(row.enabled ? {} : { disabled: true }) });
    else incomplete = true;
  }
  return { rules, incomplete };
}

/** 按换行、逗号或空格拆分粘贴文本。 */
export function pastedItems(text: string): string[] {
  return text.split(/[\s,;，；]+/).filter(Boolean);
}

/** 计算速率；计数回退（隧道重开）时从头计。 */
export function rate(before: { bytes: number; at: number } | null, bytes: number, at: number): number {
  if (!before || at <= before.at || bytes < before.bytes) return 0;
  return ((bytes - before.bytes) * 1000) / (at - before.at);
}
