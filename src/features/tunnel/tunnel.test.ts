import assert from "node:assert/strict";
import { test } from "node:test";
import {
  defaultTunnelConfig,
  isIPv4,
  MAX_NETWORKS,
  normalizeNetwork,
  parseNetworks,
  parseTunnelConfig,
  rate,
  tunnelProblem,
  tunnelRequest,
} from "./tunnel.ts";

test("addresses are recognised strictly", () => {
  for (const ok of ["0.0.0.0", "10.1.2.3", "255.255.255.255"]) assert.ok(isIPv4(ok), ok);
  for (const bad of ["", "10.1.2", "10.1.2.3.4", "256.1.1.1", "01.1.1.1", "1.1.1.-1", "1.1.1.1 ", "a.b.c.d", "1..1.1", "::1"]) assert.ok(!isIPv4(bad), bad);
});

test("networks are written with their prefix length", () => {
  assert.equal(normalizeNetwork("10.0.0.0/8"), "10.0.0.0/8");
  assert.equal(normalizeNetwork(" 192.168.1.7 "), "192.168.1.7/32");
  assert.equal(normalizeNetwork("0.0.0.0/0"), "0.0.0.0/0");
  for (const bad of ["", "10.0.0.0/33", "10.0.0.0/", "10.0.0.0/08", "10.0.0.0/8/8", "10.0.0.0/-1", "example.com", "10.0.0/8"]) {
    assert.equal(normalizeNetwork(bad), null, bad);
  }
  // 隧道只管 IPv4。
  for (const v6 of ["::1", "fd00::/8", "2001:db8::/32", "::/0"]) assert.equal(normalizeNetwork(v6), null, v6);
});

test("a pasted list is split into networks, and what is not one is reported", () => {
  const parsed = parseNetworks("10.0.0.0/8\n  172.16.0.0/12, 192.168.1.7；100.64.0.0/10\n\noffice 10.0.0.0/8 10.0.0.0/40 fd00::/8");
  assert.deepEqual(parsed.networks, ["10.0.0.0/8", "172.16.0.0/12", "192.168.1.7/32", "100.64.0.0/10"]);
  assert.deepEqual(parsed.invalid, ["office", "10.0.0.0/40", "fd00::/8"]);
  assert.deepEqual(parseNetworks(" \n "), { networks: [], invalid: [] });
});

test("saved settings are read back, and nonsense falls back to the defaults", () => {
  const saved = { route: "include", cidrs: ["10.0.0.0/8", "172.16.5.1"], bypass: [] };
  assert.deepEqual(parseTunnelConfig(saved), { route: "include", cidrs: ["10.0.0.0/8", "172.16.5.1/32"], bypass: [] });
  assert.deepEqual(parseTunnelConfig(JSON.parse(JSON.stringify(defaultTunnelConfig()))), defaultTunnelConfig());

  assert.deepEqual(parseTunnelConfig({}), defaultTunnelConfig());
  assert.deepEqual(parseTunnelConfig({ route: "everything", cidrs: ["10.0.0.0/8", 7, "nope", "10.0.0.0/8"], bypass: "none" }), { ...defaultTunnelConfig(), cidrs: ["10.0.0.0/8"] });
  assert.equal(parseTunnelConfig({ cidrs: Array.from({ length: 300 }, (_, i) => `10.${i % 256}.${Math.floor(i / 256)}.0/24`) })!.cidrs.length, MAX_NETWORKS);
  for (const junk of [null, undefined, "all", 7, []]) assert.equal(parseTunnelConfig(junk), null);
});

test("settings saved by an earlier version lose what the tunnel no longer does", () => {
  const old = { route: "all", cidrs: [], bypass: ["10.0.0.0/8", "fc00::/7", "fe80::/10"], dns: true, dnsServer: "1.1.1.1", ipv6: true };
  assert.deepEqual(parseTunnelConfig(old), { route: "all", cidrs: [], bypass: ["10.0.0.0/8"] });
});

test("what stands in the way of starting is named", () => {
  const config = defaultTunnelConfig();
  assert.equal(tunnelProblem(config), null);
  assert.equal(tunnelProblem({ ...config, route: "include" }), "noNetworks");
  assert.equal(tunnelProblem({ ...config, route: "include", cidrs: ["10.0.0.0/8"] }), null);
  assert.equal(tunnelProblem({ ...config, bypass: Array.from({ length: MAX_NETWORKS + 1 }, () => "10.0.0.0/8") }), "tooManyNetworks");
});

test("the bypass list only applies when everything goes through the tunnel", () => {
  const config = { ...defaultTunnelConfig(), bypass: ["10.0.0.0/8", "192.168.0.0/16"] };
  assert.deepEqual(tunnelRequest(config), { route: "all", cidrs: [], bypass: ["10.0.0.0/8", "192.168.0.0/16"] });
  assert.deepEqual(defaultTunnelConfig(), { route: "all", cidrs: [], bypass: [] });
  // 分流到内网网段时，全局模式下填的排除网段不能把它挡在隧道外面。
  assert.deepEqual(tunnelRequest({ ...config, route: "include", cidrs: ["10.0.0.0/8"] }), { route: "include", cidrs: ["10.0.0.0/8"], bypass: [] });
  // 只交出后端认识的项。
  const stale = { ...config, dns: true } as typeof config;
  assert.deepEqual(Object.keys(tunnelRequest(stale)).sort(), ["bypass", "cidrs", "route"]);
});

test("throughput is the difference between two reports", () => {
  assert.equal(rate(null, 5000, 1000), 0);
  assert.equal(rate({ bytes: 1000, at: 1000 }, 3000, 2000), 2000);
  assert.equal(rate({ bytes: 1000, at: 1000 }, 2000, 1500), 2000);
  // 隧道重开后计数从零开始；时钟没动也不能除以零。
  assert.equal(rate({ bytes: 9000, at: 1000 }, 100, 2000), 0);
  assert.equal(rate({ bytes: 1000, at: 1000 }, 3000, 1000), 0);
});
