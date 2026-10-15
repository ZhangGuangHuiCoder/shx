<script setup lang="ts">
import { Eraser, Palette, Search, Wand2 } from "@lucide/vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiPopover from "@/shared/ui-kit/UiPopover.vue";
import { computed, ref, watch } from "vue";
import type { Profile } from "@/shared/api";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import { locale, t } from "@/shared/i18n";
import { HOST_ICON_GROUPS, HOST_TINTS, hostHue, hostIconLabel, hostIconMatches, hostTint, resolveHostIcon } from "@/features/home/hostIcons";
import { accentText, dim } from "@/shared/ui-kit/styles";

const props = defineProps<{ profile: Profile | null }>();
const emit = defineEmits<{
  /** null 分别表示首字符、自动色相。 */
  save: [value: { icon: string | null; color: number | null }];
  close: [];
}>();

const RAINBOW = "bg-conic/decreasing from-violet-500 to-red-500";

// 本地草稿，保存后才生效。
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

// 标题旁头像即时预览选择效果。
const previewIcon = computed(() => resolveHostIcon(iconDraft.value));
const previewTint = computed(() => hostTint(colorDraft.value ?? (props.profile ? hostHue(props.profile.id) : 210)));
const picked = "ring-2 ring-zinc-800 ring-offset-2 ring-offset-white dark:ring-zinc-100 dark:ring-offset-zinc-800";
const initial = computed(() => (props.profile?.name.trim()[0] || "?").toUpperCase());

function save() {
  emit("save", { icon: iconDraft.value, color: colorDraft.value ?? null });
}
</script>

<template>
  <UiModal :show="profile !== null" class="w-3xl" @update:show="(v: boolean) => !v && emit('close')">
    <template #header>
      <DialogHeader :title="t('profile.customize')" :detail="profile?.name">
        <template #icon>
          <div :class="[previewTint.avatar, 'grid size-9 place-items-center rounded-xl bg-linear-to-br text-base font-bold text-white shadow-md inset-shadow-xs inset-shadow-white/30 transition-colors duration-150']">
            <component :is="previewIcon" v-if="previewIcon" :size="19" :stroke-width="1.9" />
            <span v-else>{{ initial }}</span>
          </div>
        </template>
      </DialogHeader>
      <UiInput v-model:value="query" class="min-w-0 flex-1" size="large" clearable autofocus :placeholder="t('icon.search')">
        <template #prefix><Search :size="16" :class="dim" /></template>
      </UiInput>
    </template>
    <template #header-extra>
      <UiPopover v-model:show="colorOpen" placement="bottom-end">
        <template #trigger>
          <!-- 底色反映当前颜色，自动时显示彩虹。 -->
          <button
            class="relative mr-3 grid size-9 cursor-pointer place-items-center overflow-hidden rounded-full border p-0 transition duration-150"
            :class="colorOpen ? 'border-blue-500 ring-2 ring-blue-500/40' : 'border-zinc-900/10 hover:border-blue-500/50 dark:border-white/10'"
            :title="t('icon.color')"
          >
            <span class="absolute inset-0" :class="colorDraft === undefined ? RAINBOW : previewTint.swatch" />
            <Palette :size="14" class="relative text-white drop-shadow-md" />
          </button>
        </template>
        <div class="grid grid-cols-6 gap-2.5">
          <button
            :class="[RAINBOW, 'grid size-6 cursor-pointer place-items-center rounded-full p-0 text-white transition-transform duration-100 hover:scale-115', colorDraft === undefined && picked]"
            :title="t('profile.resetColor')"
            @click="pickColor(undefined)"
          >
            <Wand2 :size="13" />
          </button>
          <button
            v-for="tint in HOST_TINTS"
            :key="tint.hue"
            :class="[tint.swatch, 'grid size-6 cursor-pointer place-items-center rounded-full p-0 transition-transform duration-100 hover:scale-115', colorDraft !== undefined && previewTint === tint && picked]"
            :title="`${tint.hue}`"
            @click="pickColor(tint.hue)"
          />
        </div>
      </UiPopover>
    </template>
    <div class="flex flex-col gap-4">
      <!-- 每行 14 个图标，每组正好一行或半行。 -->
      <div class="max-h-140 overflow-x-hidden overflow-y-auto pr-2">
        <section v-for="group in groups" :key="group.label" class="mb-4 last:mb-0">
          <div :class="[dim, 'mb-1.5 text-xs font-semibold']">{{ t(group.label) }}</div>
          <div class="grid grid-cols-14 gap-1">
            <button
              v-for="name in group.icons"
              :key="name"
              class="grid aspect-square min-w-0 cursor-pointer place-items-center rounded-lg border p-0 transition-colors duration-150"
              :class="iconDraft === name ? [accentText, 'border-blue-500/50 bg-blue-500/10 dark:bg-blue-400/15'] : [dim, 'border-transparent bg-transparent hover:bg-zinc-900/5 hover:text-zinc-800 dark:hover:bg-white/10 dark:hover:text-zinc-100']"
              :title="hostIconLabel(name, locale)"
              @click="iconDraft = name"
            >
              <component :is="resolveHostIcon(name)" :size="20" />
            </button>
          </div>
        </section>
        <div v-if="!groups.length" :class="[dim, 'py-14 text-center']">{{ t("icon.noMatch") }}</div>
      </div>
    </div>

    <template #footer>
      <div class="flex w-full items-center justify-between">
        <UiButton :disabled="iconDraft === null" @click="iconDraft = null">
          <template #icon><Eraser :size="14" /></template>
          {{ t("profile.resetIcon") }}
        </UiButton>
        <div class="flex gap-2">
          <UiButton @click="emit('close')">{{ t("common.cancel") }}</UiButton>
          <UiButton type="primary" :disabled="!dirty" @click="save">{{ t("common.save") }}</UiButton>
        </div>
      </div>
    </template>
  </UiModal>
</template>
