<script setup lang="ts">
// Windows 自绘标题栏（系统标题栏已关闭，见 titlebar.ts），空白处可拖动窗口。
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  DropdownMenuContent,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { errorMessage } from "@/shared/api";
import BrandSlogan from "@/shared/ui-kit/BrandSlogan.vue";
import { t } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { dim, fg, glassBar, line, menuPanel, popLayer } from "@/shared/ui-kit/styles";
import PopPanel from "@/shared/ui-kit/PopPanel.vue";
import AppMenuContent from "@/app/AppMenuContent.vue";
import { appMenu, type MenuNode } from "@/app/menu";
import { matchAccelerator } from "@/app/shortcuts";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

const app = useApp();
const win = getCurrentWindow();

const title = computed(() => {
  const tab = app.tabs.find((item) => item.key === app.activeKey);
  return tab ? `${app.tabLabel(tab)} - Shell Xplore` : "Shell Xplore";
});

function run(action: () => unknown) {
  void Promise.resolve()
    .then(action)
    .catch((e) => message.error(errorMessage(e)));
}

const menus = computed(() => appMenu());
/** 展开中的菜单下标，无则为 null。 */
const open = ref<number | null>(null);

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

/** v 为 false 时只收起当前这个。 */
function onOpenChange(index: number, v: boolean) {
  open.value = v ? index : open.value === index ? null : open.value;
}

/** 已有菜单展开时，悬停即切换。 */
function hover(index: number) {
  if (open.value !== null) open.value = index;
}

/** 带快捷键的菜单项；自绘菜单需自行响应快捷键。 */
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

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value !== null) return void (open.value = null);
  const item = shortcuts.value.find((node) => matchAccelerator(e, node.accelerator!));
  if (!item) return;
  // 抢在终端和编辑器之前处理。
  e.preventDefault();
  e.stopPropagation();
  run(item.action);
}

const maximized = ref(false);
let unlisten: (() => void) | undefined;
let unmounted = false;

async function syncMaximized() {
  maximized.value = await win.isMaximized();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey, true);
  void syncMaximized().catch(() => {});
  // 最大化/还原都会触发尺寸变化。
  const off = await win.onResized(() => void syncMaximized().catch(() => {}));
  if (unmounted) off();
  else unlisten = off;
});

const windowButtons = computed(() => [
  { title: t("win.minimize"), path: "M0 5.5h10", action: () => win.minimize(), danger: false },
  maximized.value
    ? { title: t("win.restore"), path: "M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z", action: () => win.toggleMaximize(), danger: false }
    : { title: t("win.maximize"), path: "M0.5 0.5h9v9h-9z", action: () => win.toggleMaximize(), danger: false },
  { title: t("common.close"), path: "M0.5 0.5l9 9M9.5 0.5l-9 9", action: () => win.close(), danger: true },
]);

onBeforeUnmount(() => {
  unmounted = true;
  window.removeEventListener("keydown", onKey, true);
  unlisten?.();
});
</script>

<template>
  <header :class="[glassBar, line, 'relative flex h-9 shrink-0 items-center border-b text-xs']" data-tauri-drag-region>
    <!-- 不接收鼠标，以便拖动窗口。 -->
    <BrandSlogan class="pointer-events-none my-0 mr-3 ml-2 h-6" />
    <nav class="flex gap-px">
      <DropdownMenuRoot v-for="(menu, index) in menus" :key="index" :open="open === index" @update:open="(v: boolean) => onOpenChange(index, v)">
        <DropdownMenuTrigger as-child>
          <button
            :class="[fg, 'h-6 cursor-default rounded-md px-2 transition-colors duration-100 hover:bg-zinc-900/5 dark:hover:bg-white/10', open === index ? 'bg-zinc-900/5 dark:bg-white/10' : 'bg-transparent']"
            type="button"
            tabindex="-1"
            @mouseenter="hover(index)"
          >
            {{ menu.label }}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuPortal>
          <PopTransition>
          <DropdownMenuContent :class="popLayer" side="bottom" align="start" :side-offset="4">
            <PopPanel :class="menuPanel">
              <AppMenuContent :items="menu.items" :path="String(index)" @select="select" />
            </PopPanel>
          </DropdownMenuContent>
          </PopTransition>
        </DropdownMenuPortal>
      </DropdownMenuRoot>
    </nav>
    <!-- 相对窗口居中，不接收鼠标。 -->
    <div :class="[dim, 'pointer-events-none absolute left-1/2 max-w-2/5 -translate-x-1/2 truncate']">{{ title }}</div>
    <div class="ml-auto flex h-full">
      <!-- 关闭走 closeGuard 的询问流程。 -->
      <button
        v-for="button in windowButtons"
        :key="button.title"
        :class="[fg, 'grid h-full w-12 cursor-default place-items-center bg-transparent p-0 transition-colors duration-100', button.danger ? 'hover:bg-red-600 hover:text-white active:bg-red-500 active:text-white' : 'hover:bg-zinc-900/5 active:bg-zinc-900/10 dark:hover:bg-white/10 dark:active:bg-white/15']"
        type="button"
        tabindex="-1"
        :title="button.title"
        @click="run(button.action)"
      >
        <svg class="size-2.5 fill-none stroke-current" stroke-width="1" viewBox="0 0 10 10" :shape-rendering="button.danger ? undefined : 'crispEdges'"><path :d="button.path" /></svg>
      </button>
    </div>
  </header>
</template>
