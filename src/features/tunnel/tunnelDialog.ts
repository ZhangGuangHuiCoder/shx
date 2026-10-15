import { shallowReactive } from "vue";

export const tunnelDialog = shallowReactive({ open: false, profileId: null as string | null });

export function openTunnelDialog(profileId: string) {
  Object.assign(tunnelDialog, { open: true, profileId });
}
