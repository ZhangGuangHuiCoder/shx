<script setup lang="ts">
// Windows 上的标题栏：系统的标题栏和边框关掉了（见 titlebar.ts），这一行由页面自己画。
// 左边是品牌标识和菜单，中间是标题，右边是最小化、最大化、关闭；空白处用来拖动窗口。
import { Check } from "@lucide/vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { NDropdown, type DropdownOption } from "naive-ui";
import { computed, h, onBeforeUnmount, onMounted, ref } from "vue";
import { errorMessage } from "../shared/api";
import BrandSlogan from "../shared/components/BrandSlogan.vue";
import { t } from "../shared/i18n";
import { message } from "../shared/ui";
import { useApp } from "../stores/app";
import { appMenu, type MenuNode } from "./menu";

const app = useApp();
const win = getCurrentWindow();

/** 当前标签页的名字放在标题里，和系统标题栏的习惯一样。 */
const title = computed(() => {
  const tab = app.tabs.find((item) => item.key === app.activeKey);
  return tab ? `${app.tabLabel(tab)} - shx` : "shx";
});

function run(action: () => unknown) {
  void Promise.resolve()
    .then(action)
    .catch((e) => message.error(errorMessage(e)));
}

// ---- 菜单 ------------------------------------------------------------------

const menus = computed(() => appMenu());
/** 展开着的菜单在 `menus` 中的位置；没有展开的时为 null。 */
const open = ref<number | null>(null);
/** 按下鼠标时展开着的菜单：点的是它自己的标题，这一下就是用来收起它的。 */
let openOnPress: number | null = null;

/** 快捷键在菜单里的写法。 */
const hint = (accelerator: string) => accelerator.replace("CmdOrCtrl", "Ctrl");

/** 把菜单项转成下拉菜单的选项；选项的键是它在菜单里的路径，选中时凭它找回菜单项。 */
function options(items: MenuNode[], path: string): DropdownOption[] {
  return items.flatMap((node, index): DropdownOption[] => {
    const key = `${path}.${index}`;
    if (node.type === "native") return [];
    if (node.type === "separator") return [{ type: "divider", key }];
    if (node.type === "submenu") return [{ key, label: node.label, disabled: node.enabled?.() === false, children: options(node.items, key) }];
    const accelerator = node.type === "item" ? node.accelerator : undefined;
    return [
      {
        key,
        label: () => h("span", { class: "ctx-label" }, [h("span", node.label), accelerator ? h("span", { class: "ctx-hint" }, hint(accelerator)) : null]),
        icon: node.type === "check" ? () => (node.checked() ? h(Check, { size: 15, strokeWidth: 1.8 }) : null) : undefined,
      },
    ];
  });
}

function select(key: string) {
  open.value = null;
  let items = menus.value.map<MenuNode>((menu) => ({ type: "submenu", ...menu }));
  let node: MenuNode | undefined;
  for (const index of key.split(".")) {
    node = items[Number(index)];
    items = node?.type === "submenu" ? node.items : [];
  }
  if (node?.type === "item" || node?.type === "check") run(node.action);
}

function press() {
  openOnPress = open.value;
}

function toggle(index: number) {
  open.value = openOnPress === index ? null : index;
}

/** 已经有菜单展开时，指针移到另一个标题上就换成那一个。 */
function hover(index: number) {
  if (open.value !== null) open.value = index;
}

// ---- 快捷键 ----------------------------------------------------------------

/** 菜单里所有带快捷键的项。原生菜单栏由系统识别快捷键，这里要自己来。 */
const shortcuts = computed(() => {
  const found: Extract<MenuNode, { type: "item" }>[] = [];
  const walk = (items: MenuNode[]) => {
    for (const node of items) {
      if (node.type === "submenu") walk(node.items);
      else if (node.type === "item" && node.accelerator) found.push(node);
    }
  };
  menus.value.forEach((menu) => walk(menu.items));
  return found;
});

function pressed(e: KeyboardEvent, accelerator: string) {
  const keys = accelerator.split("+");
  const key = keys.pop()!;
  return e.ctrlKey === keys.includes("CmdOrCtrl") && !e.altKey && !e.metaKey && !e.shiftKey && e.key.toLowerCase() === key.toLowerCase();
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value !== null) return void (open.value = null);
  const item = shortcuts.value.find((node) => pressed(e, node.accelerator!));
  if (!item) return;
  // 抢在终端和编辑器之前：这些键归应用，不该再落到它们手里。
  e.preventDefault();
  e.stopPropagation();
  run(item.action);
}

// ---- 窗口按钮 --------------------------------------------------------------

const maximized = ref(false);
let unlisten: (() => void) | undefined;
let unmounted = false;

async function syncMaximized() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey, true);
  void syncMaximized().catch(() => {});
  // 最大化和还原都表现为窗口尺寸变化。
  const off = await win.onResized(() => void syncMaximized().catch(() => {}));
  if (unmounted) off();
  else unlisten = off;
});

onBeforeUnmount(() => {
  unmounted = true;
  window.removeEventListener("keydown", onKey, true);
  unlisten?.();
});
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <BrandSlogan class="slogan" />
    <nav class="menus">
      <NDropdown
        v-for="(menu, index) in menus"
        :key="index"
        trigger="manual"
        placement="bottom-start"
        size="small"
        :show="open === index"
        :options="options(menu.items, String(index))"
        :menu-props="() => ({ class: 'ctx-menu' })"
        @select="select"
        @clickoutside="open = null"
      >
        <button
          class="menu"
          :class="{ on: open === index }"
          type="button"
          tabindex="-1"
          @mousedown="press"
          @click="toggle(index)"
          @mouseenter="hover(index)"
        >
          {{ menu.label }}
        </button>
      </NDropdown>
    </nav>
    <div class="title ellipsis">{{ title }}</div>
    <div class="controls">
      <button class="control" type="button" tabindex="-1" :title="t('win.minimize')" @click="run(() => win.minimize())">
        <svg viewBox="0 0 10 10" shape-rendering="crispEdges"><path d="M0 5.5h10" /></svg>
      </button>
      <button class="control" type="button" tabindex="-1" :title="t(maximized ? 'win.restore' : 'win.maximize')" @click="run(() => win.toggleMaximize())">
        <svg v-if="maximized" viewBox="0 0 10 10" shape-rendering="crispEdges"><path d="M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z" /></svg>
        <svg v-else viewBox="0 0 10 10" shape-rendering="crispEdges"><path d="M0.5 0.5h9v9h-9z" /></svg>
      </button>
      <!-- 走窗口的关闭流程：有会话开着时会先询问（closeGuard.ts）。 -->
      <button class="control close" type="button" tabindex="-1" :title="t('common.close')" @click="run(() => win.close())">
        <svg viewBox="0 0 10 10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" /></svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  position: relative;
  display: flex;
  align-items: center;
  height: 34px;
  flex-shrink: 0;
  background: var(--bg-raised);
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
}

/* 不接收鼠标：在它上面按下也是拖动窗口。 */
.slogan {
  height: 22px;
  margin: 0 10px 0 9px;
  pointer-events: none;
}

.menus {
  display: flex;
  gap: 1px;
}

.menu {
  height: 24px;
  padding: 0 9px;
  border: 0;
  border-radius: 6px;
  background: none;
  color: var(--text);
  font: inherit;
  cursor: default;
  transition: background-color 0.1s;
}

.menu:hover,
.menu.on {
  background: var(--bg-hover);
}

/* 标题对着窗口居中，不随左右两边的宽窄偏移；它不接收鼠标，拖动落在标题栏上。 */
.title {
  position: absolute;
  left: 50%;
  max-width: 40%;
  transform: translateX(-50%);
  color: var(--text-dim);
  pointer-events: none;
}

.controls {
  display: flex;
  height: 100%;
  margin-left: auto;
}

.control {
  display: grid;
  place-items: center;
  width: 46px;
  height: 100%;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text);
  cursor: default;
  transition: background-color 0.1s;
}

.control svg {
  width: 10px;
  height: 10px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
}

.control:hover {
  background: var(--bg-hover);
}

.control:active {
  background: var(--bg-active);
}

/* 关闭按钮沿用 Windows 的红色。 */
.control.close:hover {
  background: #e81123;
  color: #fff;
}

.control.close:active {
  background: #f1707a;
  color: #fff;
}
</style>
