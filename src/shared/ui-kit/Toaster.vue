<script setup lang="ts">
import { CheckCircle2, Info, Loader2, TriangleAlert, X, XCircle } from "@lucide/vue";
import { glassPop, hoverFill } from "@/shared/ui-kit/styles";
import { destroy, toasts, type ToastType } from "@/shared/ui-kit/toast-store";

const icons: Record<ToastType, typeof Info> = {
  success: CheckCircle2,
  error: XCircle,
  warning: TriangleAlert,
  info: Info,
  loading: Loader2,
};

const tones: Record<ToastType, string> = {
  success: "text-emerald-500",
  error: "text-red-500",
  warning: "text-amber-500",
  info: "text-blue-500",
  loading: "text-blue-500",
};
</script>

<template>
  <div class="pointer-events-none fixed top-4 right-4 z-50 flex flex-col items-end gap-2" aria-live="polite">
    <TransitionGroup
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-x-4 scale-95 opacity-0"
      leave-active-class="transition duration-150 ease-in"
      leave-to-class="translate-x-3 scale-95 opacity-0"
      move-class="transition duration-200 ease-out"
    >
      <div
        v-for="t in toasts"
        :key="t.id"
        :class="[glassPop, 'pointer-events-auto flex max-w-sm items-center gap-2.5 rounded-2xl py-2.5 pr-3 pl-3.5 text-sm text-zinc-800 dark:text-zinc-100']"
      >
        <component :is="icons[t.type]" :size="16" :class="['shrink-0', tones[t.type], { 'animate-spin': t.type === 'loading' }]" />
        <span class="min-w-0 break-words">{{ t.content }}</span>
        <button
          v-if="t.closable"
          type="button"
          :class="['grid size-5 shrink-0 cursor-pointer place-items-center rounded-md text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200', hoverFill]"
          aria-label="Close"
          @click="destroy(t.id)"
        >
          <X :size="12" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>
