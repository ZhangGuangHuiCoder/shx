<script setup lang="ts">
// 一条命令在列表里的样子：名称一行，命令一行；标出搜索命中的部分和待填写的占位。
import { computed } from "vue";
import { commandParts, markHits } from "./commands";

const props = withDefaults(defineProps<{ name: string; command: string; /** 搜索词，小写。 */ words?: string[] }>(), { words: () => [] });
const nameParts = computed(() => markHits(props.name, props.words));
const parts = computed(() => commandParts(props.command, props.words));
</script>

<template>
  <div class="command-text">
    <div class="ellipsis name">
      <template v-for="(part, i) in nameParts" :key="i"><mark v-if="part.hit">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template>
    </div>
    <div class="ellipsis command">
      <template v-for="(part, i) in parts" :key="i"
        ><span v-if="part.placeholder" class="slot">{{ part.text }}</span
        ><mark v-else-if="part.hit">{{ part.text }}</mark
        ><template v-else>{{ part.text }}</template></template
      >
    </div>
  </div>
</template>

<style scoped>
.command-text {
  flex: 1;
  min-width: 0;
}

.name {
  font-weight: 500;
  line-height: 18px;
}

.command {
  margin-top: 2px;
  color: var(--text-dim);
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 11.5px;
  line-height: 17px;
}

.command::before {
  content: "$ ";
  opacity: 0.55;
}

/* 待填写的部分：一眼能看出这条命令还不能直接执行。 */
.slot {
  padding: 1px 3px;
  border-radius: 4px;
  background: var(--accent-soft);
  color: var(--accent);
}

mark {
  border-radius: 3px;
  background: color-mix(in srgb, var(--pin) 34%, transparent);
  color: var(--text);
}
</style>
