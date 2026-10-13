// 应用的字体。自带一款开源的等宽字体 JetBrains Mono（SIL OFL 1.1，许可全文在 public/licenses 里），
// 整个界面、终端和编辑器默认都用它；也可以在设置里改用系统自带的字体。
import "@fontsource-variable/jetbrains-mono/index.css";
import "@fontsource-variable/jetbrains-mono/wght-italic.css";
import { computed, ref, watchEffect } from "vue";
import { readStored, STORAGE, type Settings } from "./settings";

type FontSetting = Settings["font"];

export function isFontSetting(value: unknown): value is FontSetting {
  return value === "builtin" || value === "system";
}

/** 内置字体在 CSS 里的名字（由 @fontsource-variable/jetbrains-mono 的样式表定义）。 */
const BUILTIN = "JetBrains Mono Variable";
/** 系统的界面字体。 */
const SYSTEM_UI = '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif';
/** 系统自带的等宽字体。 */
const SYSTEM_MONO = 'Menlo, Monaco, "SF Mono", "Cascadia Mono", Consolas, "PingFang SC", "Microsoft YaHei", monospace';

/** 用户选的字体。启动时先读保存的设置，第一次渲染就是对的字体；之后由 `applyFont` 更新。 */
const setting = ref<FontSetting>(savedSetting());

function savedSetting(): FontSetting {
  const saved = readStored<Partial<Settings>>(STORAGE.settings, {}).font;
  return isFontSetting(saved) ? saved : "builtin";
}

/** 选内置字体时把它排在 `system` 前面：内置字体里没有的字（比如中文）仍然落到系统字体上。 */
const family = (system: string) => computed(() => (setting.value === "builtin" ? `"${BUILTIN}", ${system}` : system));

/** 界面文字的字体。交给 Naive UI 的主题（theme.ts），由 <NGlobalStyle> 写到 body 上。 */
export const uiFamily = family(SYSTEM_UI);
/** 等宽文字的字体：命令、路径、文件列表里的数字。选系统字体时，只有这些地方还是等宽的。 */
export const monoFamily = family(SYSTEM_MONO);

// 样式表里的等宽文字都通过这个变量取字体；它只在这里定义。
watchEffect(() => document.documentElement.style.setProperty("--font-mono", monoFamily.value));

/**
 * 终端和编辑器该用的字体。它们自己量字宽、自己画字，字体文件没加载完就用会量错，
 * 所以这里要等内置字体确实可用之后才换过去；在那之前是系统字体。
 */
export const loadedMonoFamily = ref(SYSTEM_MONO);

/** 进行中或已经成功的那次加载；失败的不留着，下次用到时重试。 */
let loading: Promise<boolean> | undefined;

/**
 * 加载内置字体的全部字形（各语言的子集、正体和斜体），返回它是否可用。页面上的文字用到哪个子集
 * 浏览器会自己去加载，画在 canvas 上的（终端）不会，所以一次全部加载好。
 */
function loadBuiltin(): Promise<boolean> {
  loading ??= (async () => {
    const faces = [...document.fonts].filter((face) => face.family.replace(/["']/g, "") === BUILTIN);
    // 个别语言的子集加载不了不要紧，所以不因为其中一个失败就整体作罢。
    await Promise.allSettled(faces.map((face) => face.load()));
    // 最基本的拉丁字形必须在。一个字形都没登记时 `check` 也返回 true，所以先看有没有。
    const usable = faces.length > 0 && document.fonts.check(`1em "${BUILTIN}"`);
    if (!usable) loading = undefined;
    return usable;
  })();
  return loading;
}

// 模块一加载就开始取字体，不等界面建好：尽量赶在第一次渲染之前。
if (setting.value === "builtin") void loadBuiltin();

/**
 * 界面要用的字体就绪时兑现，最多等 `timeout` 毫秒。启动时窗口等它之后再露面，
 * 就不会先看到系统字体、再跳成内置字体。
 */
export function fontReady(timeout: number): Promise<void> {
  if (setting.value !== "builtin") return Promise.resolve();
  return new Promise((done) => {
    setTimeout(done, timeout);
    void loadBuiltin().then(() => done());
  });
}

let latest = 0;

/** 让界面、终端和编辑器改用 `next`。 */
export async function applyFont(next: FontSetting) {
  const turn = ++latest;
  // 页面上的文字立刻换：字体还没加载完时浏览器先用后面的系统字体顶着，加载完自己换过去。
  setting.value = next;
  // 终端和编辑器等字体可用了再换。加载不了就留在系统字体上，总比字对不齐好。
  const usable = next === "system" || (await loadBuiltin());
  if (turn !== latest) return;
  loadedMonoFamily.value = usable ? monoFamily.value : SYSTEM_MONO;
}
