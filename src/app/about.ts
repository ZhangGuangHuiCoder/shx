// “关于”对话框，两个平台共用，不用系统自带的关于面板。
import { getVersion } from "@tauri-apps/api/app";
import { h } from "vue";
import { showDialog } from "@/shared/ui";
import AboutDialog from "@/app/AboutDialog.vue";
import { checkForUpdates } from "@/app/update";

/** 防止重复打开。 */
let shown = false;

export async function openAbout() {
  if (shown) return;
  shown = true;
  try {
    const version = await getVersion();
    const next = await showDialog<"update">((done) => h(AboutDialog, { version, onDone: done }), "w-sm");
    if (next === "update") void checkForUpdates();
  } finally {
    shown = false;
  }
}
