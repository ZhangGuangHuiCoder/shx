// 配置备份的导出与导入。
import { open, save } from "@tauri-apps/plugin-dialog";
import { api, errorMessage } from "@/shared/api";
import { t } from "@/shared/i18n";
import { readStored, STORAGE } from "@/shared/settings";
import { message } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { buildExport, ImportError, parseImport } from "@/features/backup/config";

const backupFiles = () => [{ name: t("backup.fileType"), extensions: ["shxbak"] }];

/** 导出全部数据为加密备份。 */
export async function exportBackup() {
  const stamp = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  const path = await save({ title: t("backup.exportTitle"), defaultPath: `shx-${stamp}.shxbak`, filters: backupFiles() });
  if (!path) return;
  try {
    // 数据变化即时写入存储，存储内容即完整数据。
    const content = buildExport({
      profiles: readStored(STORAGE.profiles, []),
      secrets: readStored(STORAGE.secrets, {}),
      keys: readStored(STORAGE.keys, []),
      knownHosts: readStored(STORAGE.knownHosts, {}),
      settings: readStored(STORAGE.settings, {}),
      commands: readStored(STORAGE.commands, []),
    });
    await api.configExport(path, JSON.stringify(content));
    message.success(t("backup.exported", { path }));
  } catch (e) {
    message.error(t("backup.exportFailed", { error: errorMessage(e) }));
  }
}

/** 选择备份并合并到当前配置。 */
export async function importBackup() {
  const path = await open({ title: t("backup.importTitle"), filters: backupFiles() });
  if (typeof path !== "string") return;
  try {
    const { data, skipped } = parseImport(await api.configImport(path));
    useApp().importConfig(data);
    const imported = t("backup.imported", { profiles: data.profiles.length, keys: data.keys.length });
    message.success(imported + (skipped ? t("backup.skipped", { count: skipped }) : ""));
  } catch (e) {
    const reason = e instanceof ImportError ? t(`backup.${e.code}`) : errorMessage(e);
    message.error(t("backup.importFailed", { error: reason }));
  }
}
