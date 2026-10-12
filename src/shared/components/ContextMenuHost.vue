<script setup lang="ts">
// 渲染窗口的右键菜单。只挂载一次，放在主题 provider 内。
import { NDropdown, type DropdownOption } from "naive-ui";
import { computed, h, onBeforeUnmount, onMounted } from "vue";
import { errorMessage } from "../api";
import { contextMenu } from "../contextMenu";
import { message } from "../ui";

const options = computed<DropdownOption[]>(() =>
  contextMenu.entries.map((entry, index) => {
    if (entry === "divider") return { type: "divider", key: `divider-${index}` };
    if ("heading" in entry) {
      return {
        type: "render",
        key: `heading-${index}`,
        render: () =>
          h("div", { class: "ctx-heading" }, [
            entry.icon?.(),
            h("div", { class: "ctx-heading-text" }, [
              h("div", { class: "ctx-heading-title" }, entry.heading),
              entry.detail ? h("div", { class: "ctx-heading-detail" }, entry.detail) : null,
            ]),
          ]),
      };
    }
    if ("group" in entry) return { type: "render", key: `group-${index}`, render: () => h("div", { class: "ctx-group" }, entry.group) };
    return {
      key: index,
      label: () => h("span", { class: "ctx-label" }, [h("span", entry.label), entry.hint ? h("span", { class: "ctx-hint" }, entry.hint) : null]),
      disabled: entry.disabled,
      icon: () => h(entry.icon, { size: 15, strokeWidth: 1.8 }),
      props: entry.danger ? { class: "ctx-danger" } : undefined,
    };
  }),
);

function close() {
  contextMenu.show = false;
}

function select(index: number) {
  close();
  const entry = contextMenu.entries[index];
  if (!entry || entry === "divider" || "heading" in entry || "group" in entry) return;
  void Promise.resolve()
    .then(entry.action)
    .catch((e) => message.error(errorMessage(e)));
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") close();
}

onMounted(() => {
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("blur", close);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("blur", close);
});
</script>

<template>
  <NDropdown
    trigger="manual"
    placement="bottom-start"
    size="small"
    :show="contextMenu.show"
    :x="contextMenu.x"
    :y="contextMenu.y"
    :options="options"
    :menu-props="() => ({ class: 'ctx-menu' })"
    @select="select"
    @clickoutside="close"
  />
</template>
