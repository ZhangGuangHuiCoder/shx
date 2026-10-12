// “可编辑文件类型”对话框的显示状态。对话框本身只在 App.vue 中挂载一次，从应用菜单打开。
import { shallowReactive } from "vue";

export const rulesDialog = shallowReactive({ open: false });

export function openRulesDialog() {
  rulesDialog.open = true;
}
