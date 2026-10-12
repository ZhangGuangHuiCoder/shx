// 会话保持：主机的终端运行在服务器的 tmux 里（见 keepalive.rs），每个终端一个 tmux
// 会话。会话名把标签页和“它的” shell 对应起来：用同一个名字再次连接，shell 还在。
// 一台主机可以有好几个会话，按槽位编号（1 号、2 号……），界面上显示成 #1、#2。

/** 会话配置 id 的定长短替身，可以安全地用在 tmux 会话名里。 */
function tag(profileId: string): string {
  // FNV-1a：每次运行结果相同，对名字来说这就够了。
  let hash = 0x811c9dc5;
  for (let i = 0; i < profileId.length; i++) hash = Math.imul(hash ^ profileId.charCodeAt(i), 0x01000193);
  return (hash >>> 0).toString(16).padStart(8, "0");
}

/** 主机第 `slot` 个 tmux 会话的名字。任何名字都不是另一个名字的前缀。 */
export function sessionName(profileId: string, slot: number): string {
  return `shx-${slot}-${tag(profileId)}`;
}

/** `name` 是这台主机的会话时，它的槽位；不是则为 null。 */
export function slotOf(profileId: string, name: string | null): number | null {
  const slot = name?.match(/^shx-([1-9]\d*)-([0-9a-f]{8})$/);
  return slot && slot[2] === tag(profileId) ? Number(slot[1]) : null;
}

/** 新标签页该进哪个会话：直接进 `enter`，或者让用户在 `choose` 里挑一个、要不就新开 `fresh`。 */
export type SessionPlan = { enter: string } | { choose: string[]; fresh: string };

/**
 * 为主机的新标签页安排会话。`inUse` 是已打开的标签页占着的会话，`kept` 是服务器上现有的会话
 * （别的主机配置的也可以混在里面）。
 *
 * - 服务器上没有留着的会话：新开一个，用第一个空着的槽位。因此头一个标签页总是 1 号。
 * - 留着一个：直接进去，重新打开应用后可以接着之前的继续。
 * - 留着不止一个：不替用户猜，按槽位列出来让他选，也可以新开。
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
