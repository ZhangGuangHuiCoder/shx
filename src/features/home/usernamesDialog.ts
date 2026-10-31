import { shallowReactive } from "vue";

export const usernamesDialog = shallowReactive({ open: false });

export function openUsernamesDialog() {
  usernamesDialog.open = true;
}
