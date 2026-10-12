// “关于 shx”对话框（AboutDialog.vue）。两个平台用同一个，不用系统自带的关于面板。
import { getVersion } from "@tauri-apps/api/app";
import { h } from "vue";
import { showDialog } from "../shared/ui";
import AboutDialog from "./AboutDialog.vue";
import { checkForUpdates } from "./update";

/** 对话框是否开着：从菜单再点一次不能叠出两个。 */
let shown = false;

export async function openAbout() {
  if (shown) return;
  shown = true;
  try {
    const version = await getVersion();
    const next = await showDialog<"update">((done) => h(AboutDialog, { version, onDone: done }), 360);
    if (next === "update") void checkForUpdates();
  } finally {
    shown = false;
  }
}
