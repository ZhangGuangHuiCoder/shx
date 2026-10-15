// 会话保活：每个终端对应服务器上一个 tmux 会话（见 keepalive.rs），按槽位编号，同名重连即可恢复 shell。

/** 配置 id 的定长短哈希，可安全用于 tmux 会话名。 */
function tag(profileId: string): string {
  // FNV-1a，结果稳定即可。
  let hash = 0x811c9dc5;
  for (let i = 0; i < profileId.length; i++) hash = Math.imul(hash ^ profileId.charCodeAt(i), 0x01000193);
  return (hash >>> 0).toString(16).padStart(8, "0");
}

/** 会话名，保证互不为前缀。 */
export function sessionName(profileId: string, slot: number): string {
  return `shx-${slot}-${tag(profileId)}`;
}

/** 不属于该主机时返回 null。 */
export function slotOf(profileId: string, name: string | null): number | null {
  const slot = name?.match(/^shx-([1-9]\d*)-([0-9a-f]{8})$/);
  return slot && slot[2] === tag(profileId) ? Number(slot[1]) : null;
}

export type SessionPlan = { enter: string } | { choose: string[]; fresh: string };

/**
 * 为新标签页安排会话（inUse：已占用的会话；kept：服务器现有会话）。
 * 无空闲会话时新开（取最小空槽位）；恰有一个时直接进入；多个时让用户选择。
 */
export function planSession(profileId: string, inUse: Iterable<string | null>, kept: Iterable<string>): SessionPlan {
  const taken = new Set(inUse);
  const existing = new Set([...kept].filter((name) => slotOf(profileId, name) !== null));
  const free = [...existing].filter((name) => !taken.has(name)).sort((a, b) => slotOf(profileId, a)! - slotOf(profileId, b)!);
  let slot = 1;
  while (taken.has(sessionName(profileId, slot)) || existing.has(sessionName(profileId, slot))) slot++;
  const fresh = sessionName(profileId, slot);
  if (!free.length) return { enter: fresh };
  return free.length === 1 ? { enter: free[0] } : { choose: free, fresh };
}
