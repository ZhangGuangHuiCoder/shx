<script setup lang="ts">
// Hosts 方案管理：左侧方案列表（勾选启用），右侧编辑所选方案正文，或预览系统 hosts。
// 改动需点“应用”才写回系统 hosts（弹管理员授权）；“还原”仅移除托管块。
import { Eye, FilePen, Network, Plus, Trash2 } from "@lucide/vue";
import { computed, ref, watch } from "vue";
import { hostsDialog } from "@/features/hosts/hostsDialog";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import { api, errorMessage, type AppError, type HostScheme } from "@/shared/api";
import { t } from "@/shared/i18n";
import { message } from "@/shared/ui";
import { dim, hoverFill, line, selectedFill, well } from "@/shared/ui-kit/styles";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";

const schemes = ref<HostScheme[]>([]);
const selectedId = ref<string | null>(null);
const viewing = ref(false);
const systemText = ref("");
const path = ref("");
const busy = ref(false);

const selected = computed(() => schemes.value.find((s) => s.id === selectedId.value) ?? schemes.value[0] ?? null);

async function load() {
  schemes.value = await api.hostsList().catch(() => []);
  path.value = await api.hostsPath().catch(() => "");
}

watch(
  () => hostsDialog.open,
  (open) => {
    if (open) {
      viewing.value = true;
      void load();
      void showSystem();
    }
  },
);

function select(id: string) {
  selectedId.value = id;
  viewing.value = false;
}

function add() {
  const base = t("hosts.newName");
  let name = base;
  for (let i = 2; schemes.value.some((s) => s.name === name); i++) name = `${base} ${i}`;
  const scheme: HostScheme = { id: crypto.randomUUID(), name, enabled: false, lines: "" };
  schemes.value.push(scheme);
  select(scheme.id);
}

function remove(id: string) {
  schemes.value = schemes.value.filter((s) => s.id !== id);
  // 删除立即持久化并写回系统 hosts。
  void save();
}

async function showSystem() {
  systemText.value = await api.hostsRead().catch((e) => errorMessage(e));
  viewing.value = true;
}

/** 执行需管理员授权的操作；忽略用户取消授权，返回是否成功。 */
async function run(task: () => Promise<void>): Promise<boolean> {
  busy.value = true;
  try {
    await task();
    return true;
  } catch (e) {
    if ((e as AppError).kind !== "cancelled") message.error(errorMessage(e));
    return false;
  } finally {
    busy.value = false;
  }
}

/** 保存整套方案并写回系统 hosts。 */
function save() {
  return run(() => api.hostsSave(schemes.value.map((s) => ({ ...s }))));
}

async function apply() {
  if (await save()) message.success(t("hosts.applied"));
}

async function revert() {
  if (await run(api.hostsRevert)) message.success(t("hosts.reverted"));
}
</script>

<template>
  <UiModal v-model:show="hostsDialog.open" class="w-2xl">
    <template #header>
      <DialogHeader :icon="Network" :title="t('hosts.title')" :detail="path" />
    </template>

    <div class="flex gap-3">
      <div class="flex w-44 shrink-0 flex-col gap-1.5">
        <div :class="[well, line, 'flex max-h-[30rem] min-h-64 flex-col gap-0.5 overflow-y-auto rounded-xl border p-1']">
          <div
            v-for="s in schemes"
            :key="s.id"
            class="group flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-sm"
            :class="s.id === selected?.id && !viewing ? selectedFill : hoverFill"
            @click="select(s.id)"
          >
            <UiCheckbox :checked="s.enabled" @update:checked="s.enabled = $event" />
            <span class="min-w-0 flex-1 truncate">{{ s.name }}</span>
            <button
              type="button"
              :title="t('hosts.delete')"
              class="grid size-6 shrink-0 cursor-pointer place-items-center rounded-md text-zinc-400 opacity-0 transition hover:bg-red-500/15 hover:text-red-500 focus-visible:opacity-100 group-hover:opacity-100 dark:text-zinc-500"
              @click.stop="remove(s.id)"
            >
              <Trash2 :size="14" />
            </button>
          </div>
          <p v-if="!schemes.length" :class="[dim, 'px-2 py-4 text-center text-xs']">{{ t("hosts.empty") }}</p>
        </div>
        <UiButton size="small" @click="add">
          <template #icon><Plus :size="13" /></template>
          {{ t("hosts.new") }}
        </UiButton>
      </div>

      <div class="flex min-w-0 flex-1 flex-col gap-2">
        <div class="flex items-center gap-2">
          <UiInput
            :value="selected?.name"
            class="min-w-0 flex-1"
            :placeholder="t('hosts.namePlaceholder')"
            :disabled="viewing || !selected"
            @update:value="selected!.name = $event"
          />
          <UiButton size="medium" :selected="!viewing" :title="t('hosts.edit')" @click="viewing = false"><FilePen :size="15" /></UiButton>
          <UiButton size="medium" :selected="viewing" :title="t('hosts.view')" @click="showSystem"><Eye :size="15" /></UiButton>
        </div>
        <UiInput
          v-if="!viewing"
          :value="selected?.lines"
          type="textarea"
          :rows="16"
          :placeholder="t('hosts.linesPlaceholder')"
          :disabled="!selected"
          @update:value="selected!.lines = $event"
        />
        <UiInput v-else :value="systemText" type="textarea" :rows="16" disabled />
        <p :class="[dim, 'm-0 text-xs/relaxed']">{{ t("hosts.hint") }}</p>
      </div>
    </div>

    <template #footer>
      <div class="flex w-full items-center justify-between gap-2">
        <UiButton type="error" ghost :loading="busy" @click="revert">{{ t("hosts.revert") }}</UiButton>
        <div class="flex items-center gap-2">
          <UiButton @click="hostsDialog.open = false">{{ t("common.close") }}</UiButton>
          <UiButton type="primary" :loading="busy" :disabled="!schemes.length" @click="apply">{{ t("hosts.apply") }}</UiButton>
        </div>
      </div>
    </template>
  </UiModal>
</template>
