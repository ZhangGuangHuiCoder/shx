<script setup lang="ts">
import UiModal from "@/shared/ui-kit/UiModal.vue";
import { closeDialog, dialogs, type DialogEntry } from "@/shared/ui-kit/dialog-store";
import type { VNode } from "vue";

const Render = (props: { render: (entry: DialogEntry) => VNode | null; entry: DialogEntry }): VNode | null =>
  props.render(props.entry);
</script>

<template>
  <UiModal
    v-for="d in dialogs"
    :key="d.id"
    :show="d.open"
    bare
    :closable="false"
    :class="d.width"
    title="Dialog"
    @update:show="(open: boolean) => !open && closeDialog(d, null)"
  >
    <Render :render="d.render" :entry="d" />
  </UiModal>
</template>
