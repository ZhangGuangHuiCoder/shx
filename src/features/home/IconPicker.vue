<script setup lang="ts">
import { Eraser, Palette, Search, Wand2 } from "@lucide/vue";
import { NButton, NInput, NModal, NPopover, NSpace } from "naive-ui";
import { computed, ref, watch } from "vue";
import type { Profile } from "../../shared/api";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import { locale, t } from "../../shared/i18n";
import { HOST_ICON_GROUPS, hostHue, hostIconLabel, hostIconMatches, resolveHostIcon, type HostIconGroup } from "./hostIcons";

const props = defineProps<{ profile: Profile | null }>();
const emit = defineEmits<{
  /** icon 为 null 表示显示首字符；color 为 null 表示自动选取色相。 */
  save: [value: { icon: string | null; color: number | null }];
  close: [];
}>();

/** 卡片图标可选的预设色相。 */
const HUES = [0, 24, 40, 55, 85, 145, 165, 190, 215, 245, 275, 310];

// 本地草稿：点击“保存”后才生效。
const iconDraft = ref<string | null>(null);
const colorDraft = ref<number | undefined>(undefined);
const query = ref("");
const colorOpen = ref(false);

function pickColor(hue: number | undefined) {
  colorDraft.value = hue;
  colorOpen.value = false;
}

watch(
  () => props.profile,
  (p) => {
    iconDraft.value = p?.icon ?? null;
    colorDraft.value = p?.color;
    query.value = "";
  },
  { immediate: true },
);

const dirty = computed(
  () => (iconDraft.value ?? null) !== (props.profile?.icon ?? null) ||
    (colorDraft.value ?? null) !== (props.profile?.color ?? null),
);

const groups = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return HOST_ICON_GROUPS;
  return HOST_ICON_GROUPS.map((g) => ({ ...g, icons: g.icons.filter((name) => hostIconMatches(name, q)) })).filter((g) => g.icons.length);
});

/** 一行放几个图标（见样式里的网格）。 */
const PER_ROW = 7;

/** 双列瀑布流：按估算的高度把分组均衡拆到两列，列内各自向下堆叠。 */
const columns = computed(() => {
  const cols: HostIconGroup[][] = [[], []];
  const heights = [0, 0];
  for (const g of groups.value) {
    const i = heights[0] <= heights[1] ? 0 : 1;
    cols[i].push(g);
    // 以图标的行高为单位：标题和内边距算一行多一点，再加上图标的行数。
    heights[i] += 1.2 + Math.ceil(g.icons.length / PER_ROW);
  }
  return cols;
});

// 标题旁边的头像就是卡片上会出现的样子：选中的图标（没选时是名称的首字符）和颜色。
const previewIcon = computed(() => resolveHostIcon(iconDraft.value));
const previewHue = computed(() => colorDraft.value ?? (props.profile ? hostHue(props.profile.id) : 210));
const initial = computed(() => (props.profile?.name.trim()[0] || "?").toUpperCase());

function save() {
  emit("save", { icon: iconDraft.value, color: colorDraft.value ?? null });
}
</script>

<template>
  <NModal
    :show="profile !== null"
    preset="card"
    class="icon-picker-modal"
    style="width: 760px; max-width: calc(100vw - 48px)"
    :bordered="false"
    @update:show="(v: boolean) => !v && emit('close')"
  >
    <template #header>
      <DialogHeader :title="t('profile.customize')" :detail="profile?.name">
        <template #icon>
          <div class="preview" :style="{ '--h': String(previewHue) }">
            <component :is="previewIcon" v-if="previewIcon" :size="19" :stroke-width="1.9" />
            <span v-else>{{ initial }}</span>
          </div>
        </template>
      </DialogHeader>
      <!-- 搜索框嵌入标题行中部，参考命令助手头部：加高、大圆角、撑满剩余空间。 -->
      <NInput
        v-model:value="query"
        class="header-search"
        size="large"
        clearable
        autofocus
        :placeholder="t('icon.search')"
      >
        <template #prefix><Search :size="16" /></template>
      </NInput>
    </template>
    <!-- 主题配色入口放在标题行、关闭按钮旁边。 -->
    <template #header-extra>
      <NPopover
        v-model:show="colorOpen"
        trigger="click"
        placement="bottom-end"
        content-class="color-popover"
      >
        <template #trigger>
          <button class="color-trigger" :class="{ open: colorOpen }" :title="t('icon.color')">
            <span
              class="color-trigger-bg"
              :class="{ auto: colorDraft === undefined }"
              :style="colorDraft === undefined ? null : { background: `hsl(${colorDraft} 58% 52%)` }"
            />
            <Palette :size="14" class="color-trigger-icon" />
          </button>
        </template>
        <div class="popover-swatches">
          <button
            class="swatch auto"
            :class="{ selected: colorDraft === undefined }"
            :title="t('profile.resetColor')"
            @click="pickColor(undefined)"
          >
            <Wand2 :size="13" />
          </button>
          <button
            v-for="hue in HUES"
            :key="hue"
            class="swatch"
            :class="{ selected: colorDraft === hue }"
            :style="{ background: `hsl(${hue} 58% 52%)` }"
            :title="`${hue}`"
            @click="pickColor(hue)"
          />
        </div>
      </NPopover>
    </template>
    <div class="picker">
      <!-- 双列瀑布流：两列独立堆叠，仅纵向滚动。 -->
      <div class="grid-wrap">
        <div class="masonry">
          <div v-for="(col, ci) in columns" :key="ci" class="masonry-col">
            <div v-for="group in col" :key="group.label" class="group">
              <div class="group-label">{{ t(group.label) }}</div>
              <div class="grid">
                <button
                  v-for="name in group.icons"
                  :key="name"
                  class="cell"
                  :class="{ selected: iconDraft === name }"
                  :title="hostIconLabel(name, locale)"
                  @click="iconDraft = name"
                >
                  <component :is="resolveHostIcon(name)" :size="20" />
                </button>
              </div>
            </div>
          </div>
        </div>
        <div v-if="!groups.length" class="none">{{ t("icon.noMatch") }}</div>
      </div>
    </div>

    <!-- 和其他对话框一样，按钮放在底栏里。 -->
    <template #footer>
      <NSpace justify="space-between" align="center">
        <NButton :disabled="iconDraft === null" @click="iconDraft = null">
          <template #icon><Eraser :size="14" /></template>
          {{ t("profile.resetIcon") }}
        </NButton>
        <NSpace>
          <NButton @click="emit('close')">{{ t("common.cancel") }}</NButton>
          <NButton type="primary" :disabled="!dirty" @click="save">{{ t("common.save") }}</NButton>
        </NSpace>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.picker {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* 盖住标题图标的底色块，画成主机卡片上头像的样子。 */
.preview {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  background: linear-gradient(145deg, hsl(var(--h) 66% 58%), hsl(calc(var(--h) + 22) 62% 42%));
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 28%);
  color: #fff;
  font-size: 16px;
  font-weight: 700;
  transition: background 0.15s;
}

.none {
  padding: 56px 0;
  color: var(--text-dim);
  text-align: center;
}

/* 标题行中的搜索框：撑满标题与主题按钮之间，与大输入框匹配的圆角。 */
.header-search {
  flex: 1;
  min-width: 0;
  border-radius: 10px;
}

/* 颜色主题按钮：底色反映当前选中的色相，自动配色时显示彩虹渐变。
   位于标题行、关闭按钮左侧。 */
.color-trigger {
  position: relative;
  width: 34px;
  height: 34px;
  margin-right: 12px;
  border-radius: 50%;
  border: 1px solid var(--border);
  padding: 0;
  cursor: pointer;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--bg-raised);
  transition: box-shadow 0.15s, border-color 0.15s;
}

.color-trigger:hover {
  border-color: var(--accent-border);
}

.color-trigger.open {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent-border);
}

.color-trigger-bg {
  position: absolute;
  inset: 0;
}

.color-trigger-bg.auto {
  background: conic-gradient(hsl(0 70% 55%), hsl(45 70% 52%), hsl(130 60% 45%), hsl(200 70% 52%), hsl(275 65% 58%), hsl(330 70% 58%), hsl(0 70% 55%));
}

.color-trigger-icon {
  position: relative;
  color: #fff;
  filter: drop-shadow(0 1px 2px rgb(0 0 0 / 55%));
}

.grid-wrap {
  /* 双列瀑布流：仅纵向滚动，杜绝横向滚动条。 */
  max-height: min(540px, calc(100vh - 280px));
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 6px;
}

.masonry {
  display: flex;
  align-items: flex-start;
  gap: 12px;
}

.masonry-col {
  flex: 1 1 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.group {
  box-sizing: border-box;
  padding: 10px 12px 12px;
  background: var(--bg-hover);
  border: 1px solid var(--border);
  border-radius: 12px;
}

.group-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-dim);
  margin-bottom: 8px;
  letter-spacing: 0.3px;
}

/* 一行 7 个，平分整行（PER_ROW 与此一致）。 */
.grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 6px;
}

.cell {
  aspect-ratio: 1;
  min-width: 0;
  padding: 0;
  display: grid;
  place-items: center;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.cell:hover {
  background: var(--fill);
  color: var(--text);
}

.cell.selected {
  color: var(--accent);
  background: var(--accent-soft);
  border-color: var(--accent-border);
}
</style>

<!-- NPopover 内容传送到 body，需要非 scoped 样式才能命中。 -->
<style>
/* 搜索框嵌入标题行中部：__main 变为 flex 行，输入框填满剩余宽度。 */
.icon-picker-modal.n-card > .n-card-header .n-card-header__main {
  display: flex;
  align-items: center;
  gap: 20px;
}

/* __main（搜索框）与 __extra（主题按钮）之间 n-card 默认间距为 0，手动补上。 */
.icon-picker-modal.n-card > .n-card-header .n-card-header__extra {
  margin-left: 20px;
}

/* 收紧标题行与图标区之间的间距，默认 20px。 */
.icon-picker-modal.n-card > .n-card-header {
  padding-bottom: 10px;
}

.color-popover {
  padding: 10px;
}

.color-popover .popover-swatches {
  display: grid;
  grid-template-columns: repeat(7, 24px);
  gap: 8px;
}

.color-popover .swatch {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid transparent;
  padding: 0;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: #fff;
  transition: transform 0.1s;
}

.color-popover .swatch:hover {
  transform: scale(1.16);
}

.color-popover .swatch.selected {
  border-color: var(--text);
  box-shadow: 0 0 0 2px var(--bg-raised);
}

.color-popover .swatch.auto {
  background: conic-gradient(hsl(0 70% 55%), hsl(45 70% 52%), hsl(130 60% 45%), hsl(200 70% 52%), hsl(275 65% 58%), hsl(330 70% 58%), hsl(0 70% 55%));
}
</style>
