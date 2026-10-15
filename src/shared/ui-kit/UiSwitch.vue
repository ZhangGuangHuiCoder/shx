<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from "reka-ui";

withDefaults(defineProps<{ value?: boolean; disabled?: boolean }>(), { value: false });
const emit = defineEmits<{ "update:value": [value: boolean] }>();
</script>

<template>
  <SwitchRoot
    :model-value="value"
    :disabled="disabled"
    class="group relative h-5 w-9 shrink-0 cursor-pointer rounded-full border border-zinc-900/10 bg-zinc-900/10 p-0 inset-shadow-xs inset-shadow-zinc-900/10 transition duration-200 ease-out outline-none focus-visible:ring-3 focus-visible:ring-blue-500/30 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-transparent data-[state=checked]:bg-emerald-500 dark:border-white/10 dark:bg-white/15 dark:data-[state=checked]:bg-emerald-500"
    @update:model-value="(v: boolean) => emit('update:value', v)"
  >
    <SwitchThumb
      class="pointer-events-none absolute top-0.5 left-0.5 block size-3.5 rounded-full bg-white shadow-sm shadow-black/30 transition-transform duration-200 ease-out group-data-[state=checked]:translate-x-4"
    />
  </SwitchRoot>
</template>
