/**
 * 分段选择滑块的内联样式。格子等宽，内边距和间隙均为 2px：
 * 宽度为一格，位移为 index ×（一格 + 间隙）。无选中项时返回 null。
 */
export function segmentSlider(count: number, index: number): Record<string, string> | null {
  if (count <= 0 || index < 0) return null;
  return {
    width: `calc((100% - 4px - ${count - 1} * 2px) / ${count})`,
    transform: `translateX(calc(${index} * (100% + 2px)))`,
  };
}
