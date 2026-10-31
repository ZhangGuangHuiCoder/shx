import { shallowReactive } from "vue";

export const hostsDialog = shallowReactive({ open: false });

export function openHostsDialog() {
  hostsDialog.open = true;
}
