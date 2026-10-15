<script setup lang="ts">
// 两条不可删除的内置规则：本地网络（可停用）和兜底走向。
import { Globe } from "@lucide/vue";
import type { TunnelVia } from "@/shared/api";
import { t } from "@/shared/i18n";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import ViaSwitch from "@/features/tunnel/ViaSwitch.vue";
import { dim, line, well } from "@/shared/ui-kit/styles";

const card = "flex h-10 items-center gap-2 rounded-xl border border-zinc-900/10 bg-white/70 pr-2 pl-3 shadow-xs dark:border-white/10 dark:bg-white/5";

defineProps<{
  lan: { enabled: boolean; via: TunnelVia };
  fallback: TunnelVia;
  localNets: string[];
}>();
const emit = defineEmits<{
  "update:lan": [value: { enabled: boolean; via: TunnelVia }];
  "update:fallback": [value: TunnelVia];
}>();
</script>

<template>
  <div :class="[line, well, 'flex shrink-0 flex-col gap-2 border-t p-2']">
    <!-- 本机网段自动探测，可停用。 -->
    <div :class="card">
      <UiCheckbox :checked="lan.enabled" :aria-label="t('tunnel.ruleEnabled')" @update:checked="(on: boolean) => emit('update:lan', { ...lan, enabled: on })" />
      <div class="flex min-w-0 flex-1 items-center gap-2 text-sm" :class="!lan.enabled && 'opacity-50'" :title="t('tunnel.lanTip')">
        <span class="shrink-0 font-semibold">{{ t("tunnel.lan") }}</span>
        <span :class="[dim, 'truncate text-xs', localNets.length && 'font-mono']">{{ localNets.length ? localNets.join(t("common.listSeparator")) : t("tunnel.lanHint") }}</span>
      </div>
      <ViaSwitch :class="!lan.enabled && 'opacity-50'" :model-value="lan.via" @update:model-value="(via: TunnelVia) => emit('update:lan', { ...lan, via })" />
      <!-- 占位对齐用户规则的删除按钮。 -->
      <span class="w-6 shrink-0" aria-hidden="true" />
    </div>
    <!-- 兜底规则，只能改走向。 -->
    <div :class="card">
      <Globe :size="15" :class="[dim, 'shrink-0']" aria-hidden="true" />
      <div class="flex min-w-0 flex-1 items-center gap-2 text-sm">
        <span class="shrink-0 font-semibold">{{ t("tunnel.fallback") }}</span>
        <span :class="[dim, 'truncate text-xs']">{{ t("tunnel.fallbackHint") }}</span>
      </div>
      <ViaSwitch :model-value="fallback" @update:model-value="(via: TunnelVia) => emit('update:fallback', via)" />
      <span class="w-6 shrink-0" aria-hidden="true" />
    </div>
  </div>
</template>
