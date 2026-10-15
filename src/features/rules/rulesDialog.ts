import { shallowReactive } from "vue";

export const rulesDialog = shallowReactive({ open: false });

export function openRulesDialog() {
  rulesDialog.open = true;
}
