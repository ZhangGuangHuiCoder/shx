// 本地 shell 会话约定；会话 id 以 `local:` 开头，与后端 ID_PREFIX 对应。

/** 本地 shell 标签页使用的固定配置 id，不出现在主机列表中。 */
export const LOCAL_PROFILE_ID = "local";

/** 会话 id 是否为本地 shell。 */
export function isLocalSession(id: string | null | undefined): boolean {
  return !!id && id.startsWith("local:");
}

/** 标签页是否为本地 shell。 */
export function isLocalTab(tab: { profileId: string }): boolean {
  return tab.profileId === LOCAL_PROFILE_ID;
}
