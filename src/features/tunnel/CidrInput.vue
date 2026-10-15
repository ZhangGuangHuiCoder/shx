<script setup lang="ts">
// CIDR 输入框：四段地址和前缀各一格，自动跳格、支持粘贴，回车补全（completeNetwork）。
// 完全在前缀之外的地址段显示为灰色 0 且跳过。绑定值为拼接文本，未填完时只含已填部分。
import { ref, watch } from "vue";
import { completeNetwork, pastedItems } from "@/features/tunnel/tunnel";
import { dim } from "@/shared/ui-kit/styles";

const model = defineModel<string>({ required: true });
defineProps<{
  /** 显示示例占位。 */
  example?: boolean;
  label: string;
}>();
const emit = defineEmits<{
  enter: [];
  /** 粘贴了多个网段，交由外部分行处理。 */
  many: [items: string[]];
}>();

const LIMITS = [255, 255, 255, 255, 32];
const EXAMPLE = ["192", "168", "1", "0", "24"];
const PREFIX = 4;
const width = (index: number) => String(LIMITS[index]).length;

const parts = ref<string[]>(["", "", "", "", ""]);
const fields = ref<HTMLInputElement[]>([]);

/** 只留数字，限制范围，去掉前导零。 */
function clean(index: number, text: string): string {
  const digits = text.replace(/\D/g, "").slice(0, width(index));
  if (!digits) return "";
  return String(Math.min(Number(digits), LIMITS[index]));
}

/** 该段是否完全在前缀之外。前缀输入中会变化，所以只隐藏不清除原值。 */
const idleIn = (list: string[], index: number) => index < PREFIX && list[PREFIX] !== "" && 8 * index >= Number(list[PREFIX]);
const idle = (index: number) => idleIn(parts.value, index);

function compose(list: string[]): string {
  const address = list
    .slice(0, PREFIX)
    .map((part, index) => (idleIn(list, index) ? "0" : part))
    .join(".")
    .replace(/\.+$/, "");
  return list[PREFIX] ? `${address}/${list[PREFIX]}` : address;
}

function parse(text: string): string[] {
  const [address = "", prefix = ""] = text.trim().split("/");
  const octets = address.split(".");
  return [...[0, 1, 2, 3].map((index) => clean(index, octets[index] ?? "")), clean(PREFIX, prefix)];
}

// 外部修改值时重新填充各格，忽略自身引起的变化。
watch(model, (text) => text === compose(parts.value) || (parts.value = parse(text)), { immediate: true });

function commit(list: string[]) {
  parts.value = list;
  model.value = compose(list);
}

/** 聚焦第 index 格，跳过无效段。 */
function go(index: number, caret: "all" | "start" | "end" = "all", step = 1) {
  while (index >= 0 && index < PREFIX && idle(index)) index += step;
  const field = fields.value[index];
  if (!field) return;
  field.focus();
  if (caret === "all") field.select();
  else field.setSelectionRange(caret === "start" ? 0 : field.value.length, caret === "start" ? 0 : field.value.length);
}

/**
 * 从 start 格起填入一串输入：“.”跳下一段，“/”跳前缀（兼容中文“。”“、”），格满自动进位。
 * 输入法、听写或自动填充可能一次给出整串。返回最终所在格。
 */
function feed(start: number, text: string): number {
  const list = [...parts.value];
  let index = start;
  let buffer = "";
  const next = (to: number) => {
    list[index] = clean(index, buffer);
    buffer = "";
    // 跳过无效段。
    while (to < PREFIX && idleIn(list, to)) to++;
    index = to;
  };
  for (const char of text) {
    if (/\d/.test(char)) {
      const overflow = buffer.length === width(index) || (buffer !== "" && Number(buffer + char) > LIMITS[index]);
      if (overflow && index < PREFIX) next(index + 1);
      buffer += char;
    } else if (/[.。\s]/.test(char) && index < PREFIX - 1) {
      next(index + 1);
    } else if ((char === "/" || char === "、") && index < PREFIX) {
      next(PREFIX);
    }
  }
  // 停在后面的格但未填入时，保留原值。
  if (buffer || index === start) list[index] = clean(index, buffer);
  commit(list);
  return index;
}

/** 该格已满。 */
const full = (index: number, text: string) => text.length === width(index) || (text !== "" && Number(text) * 10 > LIMITS[index]);

function onInput(index: number, event: Event) {
  const field = event.target as HTMLInputElement;
  const end = feed(index, field.value);
  field.value = parts.value[index];
  if (end !== index) go(end, "end");
  else if (full(index, field.value) && field.selectionStart === field.value.length) go(index + 1);
}

function onKeydown(index: number, event: KeyboardEvent) {
  const field = event.target as HTMLInputElement;
  const at = (position: number) => field.selectionStart === position && field.selectionEnd === position;
  if (event.key === "Enter") {
    event.preventDefault();
    commit(parse(completeNetwork(compose(parts.value))));
    emit("enter");
  } else if (event.key === "." || event.key === " " || event.key === "。") {
    event.preventDefault();
    if (index < PREFIX - 1 && field.value) go(index + 1);
  } else if (event.key === "/" || event.key === "、") {
    event.preventDefault();
    if (index < PREFIX) go(PREFIX);
  } else if (index > 0 && ((event.key === "Backspace" && !field.value) || (event.key === "ArrowLeft" && at(0)))) {
    event.preventDefault();
    go(index - 1, "end", -1);
  } else if (event.key === "ArrowRight" && at(field.value.length) && index < PREFIX) {
    event.preventDefault();
    go(index + 1, "start");
  }
}

/** 粘贴单个地址时拆分填入；多个时交由外部处理。 */
function onPaste(event: ClipboardEvent) {
  const items = pastedItems(event.clipboardData?.getData("text") ?? "");
  if (items.length > 1) {
    event.preventDefault();
    return emit("many", items);
  }
  if (items.length !== 1 || !/[./]/.test(items[0])) return;
  event.preventDefault();
  const list = parse(items[0]);
  commit(list);
  const last = list.reduce((found, part, index) => (part ? index : found), 0);
  go(last, "end");
}

/** 点击空白处时聚焦首个空格。 */
function onMousedown(event: MouseEvent) {
  if (event.target instanceof HTMLInputElement) return;
  event.preventDefault();
  const empty = parts.value.findIndex((part, index) => !part && !idle(index));
  go(empty < 0 ? PREFIX : empty, "end");
}
</script>

<template>
  <div class="flex cursor-text items-center font-mono text-xs" role="group" :aria-label="label" @paste="onPaste" @mousedown="onMousedown">
    <template v-for="(part, index) in parts" :key="index">
      <span v-if="index" :class="[dim, 'select-none last-of-type:px-1', example && 'opacity-45']" aria-hidden="true">{{ index === PREFIX ? "/" : "." }}</span>
      <input
        :ref="(el) => el && (fields[index] = el as HTMLInputElement)"
        class="box-content rounded-md bg-transparent p-1 text-center text-inherit outline-none transition duration-150 selection:bg-blue-500/30 placeholder:text-zinc-400 placeholder:opacity-60 hover:bg-zinc-900/5 focus:bg-blue-500/10 focus:ring-1 focus:ring-blue-500/50 dark:hover:bg-white/10 dark:placeholder:text-zinc-500"
        :class="[index === PREFIX ? 'w-4' : 'w-6', idle(index) && [dim, 'pointer-events-none opacity-50']]"
        type="text"
        inputmode="numeric"
        autocomplete="off"
        spellcheck="false"
        :value="idle(index) ? '0' : part"
        :placeholder="example ? EXAMPLE[index] : ''"
        :readonly="idle(index)"
        :tabindex="idle(index) ? -1 : undefined"
        :aria-disabled="idle(index) || undefined"
        :aria-label="`${label} ${index + 1}`"
        @input="onInput(index, $event)"
        @keydown="onKeydown(index, $event)"
        @focus="($event.target as HTMLInputElement).select()"
      />
    </template>
  </div>
</template>
