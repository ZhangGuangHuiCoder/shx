import { shallowReactive } from "vue";

export const shortcutsDialog = shallowReactive({ open: false });

export function openShortcutsDialog() {
  shortcutsDialog.open = true;
}
