// 字体：默认使用内置的 JetBrains Mono（SIL OFL 1.1），可切换为系统字体。
import "@fontsource-variable/jetbrains-mono/index.css";
import "@fontsource-variable/jetbrains-mono/wght-italic.css";
import { computed, ref, watchEffect } from "vue";
import { readStored, STORAGE, type Settings } from "@/shared/settings";

type FontSetting = Settings["font"];

export function isFontSetting(value: unknown): value is FontSetting {
  return value === "builtin" || value === "system";
}

/** 内置字体的 CSS 名称。 */
const BUILTIN = "JetBrains Mono Variable";
const SYSTEM_UI = '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif';
const SYSTEM_MONO = 'Menlo, Monaco, "SF Mono", "Cascadia Mono", Consolas, "PingFang SC", "Microsoft YaHei", monospace';

/** 启动时直接读取保存的设置，保证首次渲染字体正确。 */
const saved = readStored<Partial<Settings>>(STORAGE.settings, {}).font;
const setting = ref<FontSetting>(isFontSetting(saved) ? saved : "builtin");

/** 内置字体缺少的字形（如中文）回落到系统字体。 */
const monoFamily = computed(() => (setting.value === "builtin" ? `"${BUILTIN}", ${SYSTEM_MONO}` : SYSTEM_MONO));
export const uiFamily = computed(() => (setting.value === "builtin" ? `"${BUILTIN}", ${SYSTEM_UI}` : SYSTEM_UI));

// 字体 CSS 变量只在这里定义。
watchEffect(() => {
  document.documentElement.style.setProperty("--font-sans", uiFamily.value);
  document.documentElement.style.setProperty("--font-mono", monoFamily.value);
});

const usable = ref(false);

/** 终端和编辑器自行测量字宽，须等内置字体加载完成后再切换，否则会量错。 */
export const loadedMonoFamily = computed(() => (setting.value === "builtin" && usable.value ? monoFamily.value : SYSTEM_MONO));

/** 进行中或已成功的加载；失败则清空以便重试。 */
let loading: Promise<boolean> | undefined;

/** 加载全部字形子集：canvas 绘制（终端）不会按需触发加载。返回是否可用。 */
function loadBuiltin(): Promise<boolean> {
  loading ??= (async () => {
    const faces = [...document.fonts].filter((face) => face.family.replace(/["']/g, "") === BUILTIN);
    await Promise.allSettled(faces.map((face) => face.load()));
    // 没有任何字形时 check 也返回 true，需先判断数量。
    const ok = faces.length > 0 && document.fonts.check(`1em "${BUILTIN}"`);
    if (!ok) loading = undefined;
    return ok;
  })();
  return loading;
}

// 模块加载即开始，尽量赶在首次渲染前。
if (setting.value === "builtin") void loadBuiltin().then((ok) => (usable.value = ok));

/** 字体就绪或超时后兑现，用于避免启动时字体闪变。 */
export function fontReady(timeout: number): Promise<void> {
  if (setting.value !== "builtin") return Promise.resolve();
  return Promise.race([loadBuiltin().then(() => {}), new Promise<void>((done) => setTimeout(done, timeout))]);
}

export function applyFont(next: FontSetting) {
  setting.value = next;
  if (next === "builtin") void loadBuiltin().then((ok) => (usable.value = ok));
}
