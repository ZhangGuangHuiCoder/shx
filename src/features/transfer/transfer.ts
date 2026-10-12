import type { Transfer } from "../../shared/api";

/** 传输尚未结束的各种状态。 */
const ACTIVE = ["queued", "packing", "running", "paused"];

export const isActive = (item: Transfer) => ACTIVE.includes(item.state);
