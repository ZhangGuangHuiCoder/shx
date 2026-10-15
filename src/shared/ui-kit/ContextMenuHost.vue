<script setup lang="ts">
// 全局右键菜单，在 App.vue 挂载一次，处理定位、键盘导航和关闭。
import { FocusScope } from "reka-ui";
import { computed, nextTick, onBeforeUnmount, ref, watch, type VNode } from "vue";
import { errorMessage } from "@/shared/api";
import { contextMenu } from "@/shared/contextMenu";
import { message } from "@/shared/ui";
import { dim, line, menuDivider, menuItem, menuPanel } from "@/shared/ui-kit/styles";
import PopTransition from "@/shared/ui-kit/PopTransition.vue";

const MENU_W = 224;
const EDGE = 6;

const menuEl = ref<HTMLElement>();
const hover = ref(-1);
/** 贴边修正后的实际位置。 */
const pos = ref({ left: 0, top: 0 });

/** reka FocusScope 卸载前派发的可取消事件（不冒泡），取消后不还原焦点。 */
const AUTOFOCUS_ON_UNMOUNT = "focusScope.autoFocusOnUnmount";
/** 点击外部关闭时不还原焦点，避免触发元素保持聚焦态。 */
let suppressFocusRestore = false;

function onUnmountAutoFocus(event: Event) {
  if (suppressFocusRestore) {
    event.preventDefault();
    suppressFocusRestore = false;
  }
}

const selectable = computed(() =>
  contextMenu.entries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => entry !== "divider" && !("heading" in entry) && !("group" in entry))
    .map(({ index }) => index),
);

function place() {
  const x = Math.min(contextMenu.x, window.innerWidth - MENU_W - EDGE);
  const y = Math.min(contextMenu.y, window.innerHeight - (menuEl.value?.offsetHeight ?? 320) - EDGE);
  pos.value = { left: Math.max(EDGE, x), top: Math.max(EDGE, y) };
}

watch(
  () => contextMenu.show,
  async (show) => {
    if (!show) return;
    hover.value = -1;
    suppressFocusRestore = false;
    place();
    await nextTick();
    place();
    menuEl.value?.focus();
    // 事件不冒泡，须挂在容器上；元素销毁时监听随之回收。
    menuEl.value?.addEventListener(AUTOFOCUS_ON_UNMOUNT, onUnmountAutoFocus);
  },
  { flush: "post" },
);

function close() {
  contextMenu.show = false;
}

function run(action: () => unknown) {
  close();
  void Promise.resolve()
    .then(action)
    .catch((e) => message.error(errorMessage(e)));
}

function select(index: number) {
  const entry = contextMenu.entries[index];
  if (!entry || entry === "divider" || "heading" in entry || "group" in entry || entry.disabled) return;
  run(entry.action);
}

function onKey(event: KeyboardEvent) {
  if (!contextMenu.show) return;
  const rows = selectable.value;
  if (event.key === "Escape") {
    event.preventDefault();
    close();
  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    if (!rows.length) return;
    const cur = rows.indexOf(hover.value);
    const next = event.key === "ArrowDown" ? (cur + 1) % rows.length : (cur <= 0 ? rows.length - 1 : cur - 1);
    hover.value = rows[next];
    menuEl.value?.querySelectorAll<HTMLElement>("[data-ctx-item]")[next]?.scrollIntoView({ block: "nearest" });
  } else if (event.key === "Home" || event.key === "End") {
    if (!rows.length) return;
    hover.value = event.key === "Home" ? rows[0] : rows[rows.length - 1];
  } else if (event.key === "Enter") {
    if (hover.value >= 0) {
      event.preventDefault();
      select(hover.value);
    }
  }
}

/** 外部按下时关闭菜单；若随后触发 contextmenu 会重新打开。 */
function onPointerDown(event: PointerEvent) {
  if (contextMenu.show && menuEl.value && !menuEl.value.contains(event.target as Node)) {
    suppressFocusRestore = true;
    close();
  }
}

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("pointerdown", onPointerDown, true);
  window.removeEventListener("blur", close);
});
window.addEventListener("keydown", onKey, true);
window.addEventListener("pointerdown", onPointerDown, true);
window.addEventListener("blur", close);

/** 渲染 heading 的 icon 函数返回的 VNode。 */
const VNodeView = (props: { vnode: VNode }): VNode => props.vnode;
</script>

<template>
  <Teleport to="body">
    <PopTransition kind="menu">
      <FocusScope v-if="contextMenu.show" as-child :loop="false">
        <div ref="menuEl" :class="[menuPanel, 'fixed z-50']" role="menu" tabindex="-1" :style="{ left: `${pos.left}px`, top: `${pos.top}px` }">
          <template v-for="(entry, index) in contextMenu.entries" :key="index">
            <div v-if="entry === 'divider'" :class="menuDivider" />
            <div v-else-if="'heading' in entry" :class="[line, 'mb-1 flex items-center gap-2 border-b px-3 py-2 *:first:shrink-0']">
              <VNodeView v-if="entry.icon" :vnode="entry.icon()" />
              <div class="min-w-0">
                <div class="truncate text-sm font-semibold">{{ entry.heading }}</div>
                <div v-if="entry.detail" :class="[dim, 'mt-px truncate text-xs']">{{ entry.detail }}</div>
              </div>
            </div>
            <div v-else-if="'group' in entry" :class="[dim, 'px-3 py-1 text-xs']">{{ entry.group }}</div>
            <div
              v-else
              data-ctx-item
              role="menuitem"
              :class="menuItem"
              :data-highlighted="(hover === index && !entry.disabled) || undefined"
              :data-danger="entry.danger || undefined"
              :aria-disabled="entry.disabled || undefined"
              @mouseenter="entry.disabled || (hover = index)"
              @click="entry.disabled || select(index)"
            >
              <!-- 固定尺寸的图标格，保证文字对齐。 -->
              <span class="grid size-3.75 shrink-0 place-items-center">
                <component
                  :is="entry.icon"
                  :size="15"
                  :stroke-width="1.8"
                  class="opacity-70 group-data-highlighted/item:opacity-100"
                />
              </span>
              <span class="flex min-w-0 flex-1 items-center justify-between gap-7">
                <span class="max-w-60 shrink-0 truncate">{{ entry.label }}</span>
                <span class="flex min-w-0 items-center gap-2">
                  <span v-if="entry.hint" class="min-w-0 truncate text-xs tracking-wide opacity-60">{{ entry.hint }}</span>
                  <button
                    v-if="entry.aside"
                    type="button"
                    class="grid size-5 shrink-0 cursor-pointer place-items-center rounded-md bg-transparent p-0 opacity-70 hover:bg-white/25 hover:opacity-100"
                    :title="entry.aside.title"
                    @click.stop="run(entry.aside!.action)"
                  >
                    <component :is="entry.aside.icon" :size="13" />
                  </button>
                </span>
              </span>
            </div>
          </template>
        </div>
      </FocusScope>
    </PopTransition>
  </Teleport>
</template>
