<script setup lang="ts">
// 快捷键设置：点击键位录制新组合；冲突的另一方恢复默认并提示。
import { ALargeSmall, Info, Keyboard, PanelsTopLeft, RotateCcw, SquareTerminal } from "@lucide/vue";
import { computed, onBeforeUnmount, ref, watch, type Component } from "vue";
import {
  SHORTCUT_ACTIONS,
  acceleratorOf,
  acceleratorParts,
  bindings,
  formatAccelerator,
  startRecording,
  type ShortcutAction,
  type ShortcutGroup,
} from "@/app/shortcuts";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import IconButton from "@/shared/ui-kit/IconButton.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import { t, type MessageKey } from "@/shared/i18n";
import { useApp } from "@/shared/store";
import { shortcutsDialog } from "@/features/shortcuts/shortcutsDialog";
import { dim, line, well } from "@/shared/ui-kit/styles";

const app = useApp();

const GROUPS: { id: ShortcutGroup; labelKey: MessageKey; icon: Component }[] = [
  { id: "term", labelKey: "shortcut.group.term", icon: SquareTerminal },
  { id: "font", labelKey: "shortcut.group.font", icon: ALargeSmall },
  { id: "tab", labelKey: "shortcut.group.tab", icon: PanelsTopLeft },
];
const groups = GROUPS.map((group) => ({ ...group, actions: SHORTCUT_ACTIONS.filter((action) => action.group === group.id) }));

/** 是否为自定义键位。 */
const isCustom = (action: ShortcutAction) => typeof app.settings.shortcuts[action.id] === "string";
const anyCustom = computed(() => SHORTCUT_ACTIONS.some(isCustom));

const recordingId = ref<string | null>(null);
/** 因冲突恢复默认的动作，用于提示。 */
const notice = ref("");
let stopRecording: (() => void) | undefined;

function cancel() {
  stopRecording?.();
  stopRecording = undefined;
  recordingId.value = null;
}

function record(action: ShortcutAction) {
  if (recordingId.value === action.id) return cancel();
  cancel();
  notice.value = "";
  recordingId.value = action.id;
  stopRecording = startRecording((event) => {
    if (event.key === "Escape") return cancel();
    // 只按了修饰键，等待下一个键。
    const accelerator = acceleratorOf(event);
    if (!accelerator) return;
    set(action, accelerator);
    cancel();
  });
}

function update(change: (stored: Record<string, string>) => void) {
  const stored = { ...app.settings.shortcuts };
  change(stored);
  app.settings.shortcuts = stored;
}

/** 写入新键位，冲突方恢复默认。 */
function set(action: ShortcutAction, accelerator: string) {
  // 按展示形式比较：同一组合可能有 CmdOrCtrl/Cmd 两种写法。
  const shown = formatAccelerator(accelerator);
  const losers = SHORTCUT_ACTIONS.filter((other) => other.id !== action.id && formatAccelerator(bindings.value[other.id]) === shown);
  update((stored) => {
    for (const other of losers) delete stored[other.id];
    stored[action.id] = accelerator;
  });
  notice.value = losers.map((other) => t("shortcut.moved", { name: t(other.labelKey), key: formatAccelerator(other.defaultKey) })).join(" ");
}

function reset(action: ShortcutAction) {
  notice.value = "";
  update((stored) => delete stored[action.id]);
}

function resetAll() {
  cancel();
  notice.value = "";
  app.settings.shortcuts = {};
}

onBeforeUnmount(cancel);
// 对话框关闭时结束录制。
watch(
  () => shortcutsDialog.open,
  (open) => {
    if (!open) cancel();
    notice.value = "";
  },
);
</script>

<template>
  <UiModal v-model:show="shortcutsDialog.open" class="max-h-[calc(100svh-3rem)] w-xl">
    <template #header>
      <DialogHeader :icon="Keyboard" :title="t('shortcut.title')" :detail="t('shortcut.hint')" />
    </template>

    <section v-for="group in groups" :key="group.id" class="mb-3 last:mb-0">
      <h3 :class="[dim, 'm-0 flex items-center gap-1.5 px-1 pb-1.5 text-xs font-semibold']">
        <component :is="group.icon" :size="13" :stroke-width="2" />
        {{ t(group.labelKey) }}
      </h3>
      <div :class="[well, line, 'overflow-hidden rounded-xl border']">
        <div
          v-for="(action, index) in group.actions"
          :key="action.id"
          :class="[line, 'group flex h-10 items-center gap-2 pr-2 pl-3.5', index > 0 && 'border-t']"
        >
          <span class="min-w-0 flex-1 truncate text-sm">{{ t(action.labelKey) }}</span>
          <span v-if="isCustom(action)" class="size-1.5 shrink-0 rounded-full bg-blue-500" :title="t('shortcut.custom')" />

          <IconButton
            v-if="isCustom(action) && recordingId !== action.id"
            size="tiny"
            class="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
            :title="t('shortcut.reset')"
            @click="reset(action)"
          >
            <RotateCcw :size="12" />
          </IconButton>

          <!-- 点击录制，再点或按 Esc 取消。 -->
          <button
            type="button"
            :title="recordingId === action.id ? t('shortcut.esc') : t('shortcut.edit')"
            class="flex h-7.5 min-w-32 cursor-pointer items-center justify-end gap-1 rounded-lg px-2 outline-none transition duration-150 focus-visible:ring-2 focus-visible:ring-blue-500/60"
            :class="
              recordingId === action.id
                ? 'bg-blue-500/10 ring-2 ring-blue-500/50 dark:bg-blue-400/15'
                : 'hover:bg-zinc-900/5 dark:hover:bg-white/10'
            "
            @click="record(action)"
          >
            <span v-if="recordingId === action.id" class="flex items-center gap-1.5 text-xs text-blue-600 dark:text-blue-300">
              <span class="size-1.5 animate-pulse rounded-full bg-blue-500" />
              {{ t("shortcut.record") }}
            </span>
            <kbd
              v-for="(key, i) in acceleratorParts(bindings[action.id])"
              v-else
              :key="i"
              class="inline-grid h-6 min-w-6 place-items-center rounded-md border border-b-2 border-zinc-900/10 bg-white px-1.5 font-sans text-xs font-medium dark:border-white/10 dark:bg-white/10"
              :class="isCustom(action) ? 'text-blue-600 dark:text-blue-300' : 'text-zinc-600 dark:text-zinc-300'"
            >{{ key }}</kbd>
          </button>
        </div>
      </div>
    </section>

    <template #footer>
      <p v-if="notice" :class="[dim, 'm-0 mr-auto flex min-w-0 items-center gap-1.5 self-center text-xs']">
        <Info :size="13" class="shrink-0 text-blue-500" />
        <span class="min-w-0">{{ notice }}</span>
      </p>
      <UiButton v-else-if="anyCustom" class="mr-auto" ghost @click="resetAll">
        <template #icon><RotateCcw :size="14" /></template>
        {{ t("shortcut.resetAll") }}
      </UiButton>
      <UiButton type="primary" @click="shortcutsDialog.open = false">{{ t("common.close") }}</UiButton>
    </template>
  </UiModal>
</template>
