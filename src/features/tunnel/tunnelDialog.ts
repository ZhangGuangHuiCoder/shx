// 隧道对话框的显示状态。对话框本身只在 App.vue 中挂载一次，从主机卡片的菜单和状态栏打开。
import { shallowReactive } from "vue";

export const tunnelDialog = shallowReactive({ open: false, profileId: null as string | null });

/** 打开某台主机的隧道设置。 */
export function openTunnelDialog(profileId: string) {
  Object.assign(tunnelDialog, { open: true, profileId });
}
