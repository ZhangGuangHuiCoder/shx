import type { Transfer } from "@/shared/api";

/** 未结束的传输状态。 */
const ACTIVE = ["queued", "packing", "running", "paused"];

export const isActive = (item: Transfer) => ACTIVE.includes(item.state);
