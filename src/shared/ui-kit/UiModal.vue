<script setup lang="ts">
// 模态对话框，宽度由调用方的 w-* 类决定。
// 默认带标题栏和底栏；bare 只有玻璃面；raw 完全由内容自绘。
import { DialogClose, DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from "reka-ui";
import { X } from "@lucide/vue";
import { useAttrs } from "vue";
import { dim, fg, glassPop, hoverFill } from "@/shared/ui-kit/styles";

defineOptions({ inheritAttrs: false });

withDefaults(
  defineProps<{
    show?: boolean;
    title?: string;
    closable?: boolean;
    maskClosable?: boolean;
    autoFocus?: boolean;
    closeOnEsc?: boolean;
    bare?: boolean;
    raw?: boolean;
  }>(),
  { closable: true, maskClosable: true, autoFocus: true, closeOnEsc: true },
);

const emit = defineEmits<{ "update:show": [open: boolean] }>();
const attrs = useAttrs();
</script>

<template>
  <DialogRoot :open="show" :modal="true" @update:open="(v: boolean) => emit('update:show', v)">
    <DialogPortal>
      <Transition enter-active-class="transition duration-200 ease-out" enter-from-class="opacity-0" leave-active-class="transition duration-150 ease-in" leave-to-class="opacity-0">
        <DialogOverlay class="fixed inset-0 z-40 bg-zinc-900/20 dark:bg-black/40" />
      </Transition>
      <!-- 居中层，指针穿透到遮罩，点外部可关闭。 -->
      <div class="pointer-events-none fixed inset-0 z-40 grid place-items-center p-6">
        <Transition
          enter-active-class="transition duration-300 ease-out"
          enter-from-class="translate-y-2 scale-95 opacity-0"
          leave-active-class="transition duration-150 ease-in"
          leave-to-class="translate-y-1 scale-98 opacity-0"
        >
          <DialogContent
            :class="[
              'pointer-events-auto max-h-full max-w-full outline-none',
              raw
                ? 'flex flex-col rounded-2xl'
                : bare
                  ? [glassPop, 'overflow-hidden rounded-3xl']
                  : [glassPop, 'flex flex-col rounded-3xl'],
              (attrs.class as string) ?? '',
            ]"
            :style="(attrs.style as Record<string, string>) ?? undefined"
            @open-auto-focus="(e: Event) => !autoFocus && e.preventDefault()"
            @pointer-down-outside="(e: Event) => !maskClosable && e.preventDefault()"
            @interact-outside="(e: Event) => !maskClosable && e.preventDefault()"
            @escape-key-down="(e: Event) => !closeOnEsc && e.preventDefault()"
          >
            <!-- reka 要求 Title/Description，这里补充读屏专用的。 -->
            <DialogTitle class="sr-only">{{ title || "Dialog" }}</DialogTitle>
            <DialogDescription class="sr-only">{{ title || "Dialog" }}</DialogDescription>

            <header v-if="!bare && !raw && ($slots.header || title || closable || $slots['header-extra'])" class="flex items-center gap-2 px-5 pt-5">
              <div class="flex min-w-0 flex-1 items-center gap-2">
                <slot name="header">
                  <h2 :class="[fg, 'm-0 truncate text-base font-semibold']">{{ title }}</h2>
                </slot>
              </div>
              <div class="ml-auto flex shrink-0 items-center gap-1">
                <slot name="header-extra" />
                <DialogClose
                  v-if="closable"
                  :class="[dim, hoverFill, 'grid size-7 cursor-pointer place-items-center rounded-full transition outline-none hover:text-zinc-800 focus-visible:ring-2 focus-visible:ring-blue-500/60 dark:hover:text-zinc-100']"
                  aria-label="Close"
                >
                  <X :size="15" />
                </DialogClose>
              </div>
            </header>

            <div :class="bare || raw ? '' : [fg, 'min-h-0 flex-1 overflow-y-auto px-5 pt-4 pb-5 text-sm']">
              <slot />
            </div>

            <footer v-if="!bare && !raw && $slots.footer" class="flex justify-end gap-2 px-5 pb-5">
              <slot name="footer" />
            </footer>
          </DialogContent>
        </Transition>
      </div>
    </DialogPortal>
  </DialogRoot>
</template>
