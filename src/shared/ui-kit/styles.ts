// 公共 Tailwind 类名组合。Tailwind 会扫描本文件，类名须写成完整字面量。
// 背景模糊开销大，只用于浮在变化内容之上的面（glassPop）；横栏和卡片只用半透明填充。

/** 正文文字。 */
export const fg = "text-zinc-800 dark:text-zinc-100";
/** 次要文字。 */
export const dim = "text-zinc-500 dark:text-zinc-400";
export const accentText = "text-blue-600 dark:text-blue-400";
/** 成功、警告、危险的文字颜色。 */
export const okText = "text-emerald-600 dark:text-emerald-400";
export const warnText = "text-amber-600 dark:text-amber-400";
export const dangerText = "text-red-600 dark:text-red-400";
/** 表单项、分组的小标题。 */
export const sectionLabel = `${dim} text-xs font-semibold`;
/** 分隔线颜色，配合 border 使用。 */
export const line = "border-zinc-900/10 dark:border-white/10";
/** 分隔线色的实心填充。 */
export const lineFill = "bg-zinc-900/10 dark:bg-white/10";
export const hoverFill = "hover:bg-zinc-900/5 dark:hover:bg-white/10";
export const selectedFill = "bg-blue-500/15 dark:bg-blue-400/20";
/** 内凹底色：列表框、侧栏等。 */
export const well = "bg-zinc-900/3 dark:bg-black/20";
export const focusRing = "outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60";

/** 标题栏、标签栏、状态栏等横栏。 */
export const glassBar = "bg-white/50 dark:bg-zinc-900/40";

/** 卡片、面板。 */
export const glassCard =
  "border border-white/70 bg-white/55 shadow-lg shadow-zinc-900/5 inset-shadow-xs inset-shadow-white dark:border-white/10 dark:bg-white/5 dark:shadow-black/25 dark:inset-shadow-white/10";

/** 浮层：对话框、菜单、下拉、消息条。 */
export const glassPop =
  "border border-white/70 bg-white/75 shadow-2xl shadow-zinc-900/20 inset-shadow-xs inset-shadow-white backdrop-blur-3xl backdrop-saturate-150 dark:border-white/10 dark:bg-zinc-800/75 dark:shadow-black/50 dark:inset-shadow-white/10";

/**
 * 液态玻璃骨架：模糊 + 亮边 + 内高光 + 投影。
 * 注意：不要放在带 transform 的父元素里，macOS WebKit 会把父图层画成白块。
 */
const liquidGlass =
  "border border-white/60 shadow-lg shadow-zinc-900/10 inset-shadow-sm inset-shadow-white/70 backdrop-blur-md backdrop-saturate-150 dark:border-white/20 dark:shadow-black/40 dark:inset-shadow-white/25";

/** 斜向高光。 */
const glassSheen = "bg-linear-to-br from-white/45 via-white/5 to-white/30 dark:from-white/20 dark:via-white/0 dark:to-white/10";

/** 中性玻璃胶囊（圆角由使用处指定）。 */
export const capsule = `${liquidGlass} ${glassSheen} bg-white/25 dark:bg-white/5`;

/** 图标按钮组。 */
export const toolGroup = `${capsule} inline-flex shrink-0 items-center gap-0.5 rounded-full p-0.5`;

/** 胶囊内的选中段。 */
export const raisedChip = "bg-zinc-900/8 inset-shadow-xs inset-shadow-zinc-900/5 dark:bg-white/15 dark:shadow-sm dark:shadow-black/30 dark:inset-shadow-white/15";

/** 输入框外壳。不含尺寸类：没有类名合并工具，冲突类的优先级不确定，由使用处单独指定。 */
export const field =
  "flex w-full gap-2 border border-zinc-900/10 bg-white/60 px-3 shadow-xs shadow-zinc-900/5 inset-shadow-xs inset-shadow-white transition focus-within:border-zinc-900/25 focus-within:bg-white/85 dark:border-white/10 dark:bg-white/8 dark:shadow-black/30 dark:inset-shadow-white/10 dark:focus-within:border-white/25 dark:focus-within:bg-white/12";

/** 外壳默认尺寸。 */
export const fieldMd = "min-h-9 items-center rounded-xl text-sm";

/** 外壳内的原生输入元素。 */
export const fieldInput =
  "m-0 min-w-0 flex-1 border-0 bg-transparent p-0 text-zinc-800 outline-none placeholder:text-zinc-400 dark:text-zinc-100 dark:placeholder:text-zinc-500";

/** 外壳内的小图标按钮。 */
export const fieldIcon =
  "grid shrink-0 cursor-pointer place-items-center rounded-md p-0.5 text-zinc-400 hover:text-zinc-700 dark:text-zinc-500 dark:hover:text-zinc-200";

/** reka 浮层定位层，外观在内部 PopPanel 上。 */
export const popLayer = "z-50 outline-none";

/** 下拉面板。 */
export const popPanel = `${glassPop} flex max-h-80 flex-col overflow-hidden rounded-2xl p-1.5`;

/** 下拉项，左侧为 popCheck 留位。 */
export const popItem =
  "relative flex min-h-8 cursor-pointer items-center rounded-lg py-1.5 pr-3 pl-8 text-sm text-zinc-800 outline-none select-none transition-colors duration-100 data-disabled:pointer-events-none data-disabled:opacity-40 data-highlighted:bg-zinc-900/6 data-[state=checked]:font-medium data-[state=checked]:text-blue-600 dark:text-zinc-100 dark:data-highlighted:bg-white/10 dark:data-[state=checked]:text-blue-400";

export const popCheck = "absolute left-2.5 grid place-items-center text-blue-500";

/** 下拉空状态。 */
export const popEmpty = `${dim} px-3 py-4 text-center text-sm`;

/** 小标记配色：中性、成功。 */
export const plainChip = `${dim} bg-zinc-900/5 dark:bg-white/10`;
export const okChip = "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300";

/** 对话框中的提示框。 */
export const notice = `${dim} flex items-center gap-2 rounded-xl bg-zinc-900/5 p-3 text-xs/normal dark:bg-white/5`;

/** 键帽。 */
export const keycap =
  "inline-grid h-5 min-w-5 place-items-center rounded-md border border-zinc-900/10 bg-white/70 px-1 text-xs leading-none text-zinc-500 shadow-xs dark:border-white/10 dark:bg-white/10 dark:text-zinc-400";

/** 状态圆点，颜色见 statusDotColor。 */
export const statusDot = "size-2 shrink-0 rounded-full";

/** 菜单外框。 */
export const menuPanel = `${glassPop} w-56 max-h-svh overflow-y-auto rounded-xl p-1 outline-none`;

/** 菜单项。高亮态用 data-highlighted，危险项用 data-danger；两套颜色互斥，不会同时生效。 */
export const menuItem =
  "group/item relative flex h-7 w-full cursor-pointer items-center gap-2 rounded-md border-0 px-2 text-left text-sm leading-none outline-none select-none aria-disabled:pointer-events-none aria-disabled:opacity-40 data-disabled:pointer-events-none data-disabled:opacity-40 not-data-highlighted:not-data-danger:text-zinc-800 dark:not-data-highlighted:not-data-danger:text-zinc-100 not-data-highlighted:data-danger:text-red-600 dark:not-data-highlighted:data-danger:text-red-400 data-highlighted:text-white data-highlighted:not-data-danger:bg-blue-500 data-highlighted:data-danger:bg-red-500";

export const menuDivider = `mx-2 my-1 h-px ${lineFill}`;
