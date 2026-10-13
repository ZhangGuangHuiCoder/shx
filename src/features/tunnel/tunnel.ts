// 隧道（TUN 模式，见后端的 tunnel 模块）设置的校验和读取，以及出口归属的查询。隧道只管 IPv4。
// 都是纯函数，不依赖框架，可以直接用 `node --test` 做单元测试。
import type { TunnelConfig } from "../../shared/api.ts";

/** 一次最多填这么多个网段；后端的上限相同。 */
export const MAX_NETWORKS = 256;

export function defaultTunnelConfig(): TunnelConfig {
  return { route: "all", cidrs: [], bypass: [] };
}

const DECIMAL = /^(0|[1-9]\d{0,2})$/;

export function isIPv4(text: string): boolean {
  const parts = text.split(".");
  return parts.length === 4 && parts.every((part) => DECIMAL.test(part) && Number(part) <= 255);
}

/** 网段的规范写法（总是带前缀长度）；不是 IPv4 网段时返回 null。主机位留给后端去清零。 */
export function normalizeNetwork(text: string): string | null {
  const [address, prefix, ...rest] = text.trim().split("/");
  if (rest.length || !isIPv4(address)) return null;
  if (prefix === undefined) return `${address}/32`;
  return DECIMAL.test(prefix) && Number(prefix) <= 32 ? `${address}/${Number(prefix)}` : null;
}

/** 把用户输入的一段文字拆成网段：每行一个，也可以用逗号或空格隔开。 */
export function parseNetworks(text: string): { networks: string[]; invalid: string[] } {
  const networks = new Set<string>();
  const invalid: string[] = [];
  for (const item of text.split(/[\s,;，；]+/).filter(Boolean)) {
    const network = normalizeNetwork(item);
    if (network) networks.add(network);
    else invalid.push(item);
  }
  return { networks: [...networks], invalid };
}

/**
 * 读取保存的（或备份里的）隧道设置；不成样子时返回 null，个别不对的项丢掉。
 * 旧版本存下来的、现在已经没有的项（DNS、IPv6）和 IPv6 的网段就此丢掉。
 */
export function parseTunnelConfig(value: unknown): TunnelConfig | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const v = value as Record<string, unknown>;
  const networks = (list: unknown) =>
    Array.isArray(list)
      ? [...new Set(list.flatMap((item) => (typeof item === "string" ? normalizeNetwork(item) ?? [] : [])))].slice(0, MAX_NETWORKS)
      : [];
  return { route: v.route === "include" ? "include" : "all", cidrs: networks(v.cidrs), bypass: networks(v.bypass) };
}

/** 设置里妨碍开启隧道的问题；没有问题时返回 null。 */
export function tunnelProblem(config: TunnelConfig): "noNetworks" | "tooManyNetworks" | null {
  if (config.route === "include" && !config.cidrs.length) return "noNetworks";
  if (config.cidrs.length > MAX_NETWORKS || config.bypass.length > MAX_NETWORKS) return "tooManyNetworks";
  return null;
}

/** 交给后端的设置。排除网段只在全局模式下填写和生效；分流时不带上，免得把想进隧道的网段挡在外面。 */
export function tunnelRequest(config: TunnelConfig): TunnelConfig {
  return { route: config.route, cidrs: config.cidrs, bypass: config.route === "all" ? config.bypass : [] };
}

/** 每秒的流量：两次状态之间的差除以间隔。计数变小了（隧道重开过）就从头算。 */
export function rate(before: { bytes: number; at: number } | null, bytes: number, at: number): number {
  if (!before || at <= before.at || bytes < before.bytes) return 0;
  return ((bytes - before.bytes) * 1000) / (at - before.at);
}
