<script setup lang="ts">
// 新建、编辑主机的对话框。从上到下依次是：叫什么、连到哪里、怎么登录、其他选项。
import { Fingerprint, HeartPulse, KeyRound, LockKeyhole, Pencil, Plus, Trash2 } from "@lucide/vue";
import { open } from "@tauri-apps/plugin-dialog";
import { NButton, NCheckbox, NInput, NInputNumber, NModal, NRadioButton, NRadioGroup, NSelect, NSpace, NSwitch } from "naive-ui";
import { computed, reactive, ref, watch, type Component } from "vue";
import { errorMessage, type AuthKind, type Profile } from "../../shared/api";
import DialogHeader from "../../shared/components/DialogHeader.vue";
import { t } from "../../shared/i18n";
import { confirm, message } from "../../shared/ui";
import { useApp } from "../../stores/app";

const props = defineProps<{ profile: Profile | null }>();
const show = defineModel<boolean>("show", { required: true });
const app = useApp();

const blank = (): Profile => ({
  id: "", name: "", group: "", host: "", port: 22, username: "root", auth: "password", keyId: null, saveSecret: true, note: "", keepAlive: false,
});
const form = reactive<Profile>(blank());

/** 登录方式，图标和首页卡片上的标记相同。 */
const AUTH: { value: AuthKind; icon: Component; label: () => string }[] = [
  { value: "password", icon: LockKeyhole, label: () => t("profile.password") },
  { value: "key", icon: KeyRound, label: () => t("profile.key") },
  { value: "agent", icon: Fingerprint, label: () => "SSH Agent" },
];
const secret = ref("");
const pasting = ref(false);
const pasted = ref("");
const pastedName = ref("");

watch(show, (visible) => {
  if (!visible) return;
  // 以空白表单为底，清掉被编辑的配置中没有设置的可选字段（上一次编辑的主机留下的）。
  for (const key of Object.keys(form)) Reflect.deleteProperty(form, key);
  Object.assign(form, blank(), props.profile ? { ...props.profile } : {});
  secret.value = "";
  pasting.value = false;
  typedGroup = "";
});

/** 已有的分组，供选择；也可以直接输入一个新的。 */
const groupOptions = computed(() => [...new Set(app.profiles.map((p) => p.group).filter(Boolean))].map((name) => ({ label: name, value: name })));
/** 分组框里已经输入、还没有选定的文字。 */
let typedGroup = "";

function setGroup(value: string | null) {
  form.group = value?.trim() ?? "";
  typedGroup = "";
}

/** 输完没按回车就去填别的：输入的文字照样算数，不悄悄丢掉。 */
function keepTypedGroup() {
  if (typedGroup.trim()) setGroup(typedGroup);
}
const keyOptions = computed(() => app.keys.map((k) => ({ value: k.id, label: k.encrypted ? t("profile.hasPassphrase", { name: k.name }) : k.name })));
/** 名称留空时会保存为主机地址，所以校验的是最终生效的值。 */
const nameError = computed<string | null>(() => {
  const effective = form.name.trim() || form.host.trim();
  if (!effective) return null;
  return app.profiles.some((p) => p.id !== form.id && p.name === effective) ? t("profile.nameTaken") : null;
});
const valid = computed(
  () =>
    !!form.host.trim() &&
    !!form.username.trim() &&
    // 端口框可以被清空，这时它的值是 null。
    Number.isInteger(form.port) &&
    (form.auth !== "key" || !!form.keyId) &&
    !nameError.value,
);
const secretPlaceholder = computed(() => t(props.profile?.saveSecret ? "profile.secretKeep" : "profile.secretAsk"));

async function importKey(source: { path?: string; content?: string; name?: string }) {
  try {
    form.keyId = (await app.importKey(source)).id;
    pasting.value = false;
    pasted.value = pastedName.value = "";
  } catch (e) {
    message.error(errorMessage(e));
  }
}

async function importFile() {
  const path = await open({ title: t("profile.pickKey") }).catch(() => null);
  if (typeof path === "string") await importKey({ path });
}

async function removeKey() {
  const key = app.keys.find((k) => k.id === form.keyId);
  if (!key || !(await confirm(t("profile.deleteKeyTitle"), t("profile.deleteKeyBody", { name: key.name }), { danger: true, ok: t("common.delete") }))) return;
  try {
    app.deleteKey(key.id);
    form.keyId = null;
  } catch (e) {
    message.error(errorMessage(e));
  }
}

function save() {
  const host = form.host.trim();
  app.saveProfile({ ...form, host, name: form.name.trim() || host, username: form.username.trim() }, secret.value || undefined);
  show.value = false;
}
</script>

<template>
  <NModal v-model:show="show" preset="card" style="width: 520px" :bordered="false" :mask-closable="false">
    <template #header>
      <DialogHeader :icon="profile ? Pencil : Plus" :title="t(profile ? 'profile.editTitle' : 'sidebar.new')" :detail="profile?.name" />
    </template>

    <div class="profile">
      <div class="pair">
        <label class="field">
          <span class="label">{{ t("common.name") }}</span>
          <NInput v-model:value="form.name" :status="nameError ? 'error' : undefined" :placeholder="t('profile.namePlaceholder')" />
          <span v-if="nameError" class="problem">{{ nameError }}</span>
        </label>
        <div class="field">
          <span class="label">{{ t("profile.group") }}</span>
          <NSelect
            :value="form.group || null"
            :options="groupOptions"
            filterable
            tag
            clearable
            :placeholder="t('common.optional')"
            @update:value="setGroup"
            @search="typedGroup = $event"
            @blur="keepTypedGroup"
          />
        </div>
      </div>

      <!-- 和 ssh 命令里的写法一样：用户名 @ 主机 : 端口。标签一行，输入框一行。 -->
      <div class="address">
        <span class="label required">{{ t("profile.username") }}</span>
        <span />
        <span class="label required">{{ t("profile.host") }}</span>
        <span />
        <span class="label">{{ t("profile.port") }}</span>
        <NInput v-model:value="form.username" />
        <span class="sep">@</span>
        <NInput v-model:value="form.host" :placeholder="t('profile.hostPlaceholder')" />
        <span class="sep">:</span>
        <NInputNumber v-model:value="form.port" :min="1" :max="65535" :show-button="false" placeholder="22" />
      </div>

      <div class="field">
        <span class="label">{{ t("profile.auth") }}</span>
        <NRadioGroup v-model:value="form.auth" class="auth">
          <NRadioButton v-for="kind in AUTH" :key="kind.value" :value="kind.value">
            <span class="auth-kind"><component :is="kind.icon" :size="13" :stroke-width="2" />{{ kind.label() }}</span>
          </NRadioButton>
        </NRadioGroup>
      </div>

      <div v-if="form.auth === 'key'" class="field">
        <span class="label required">{{ t("profile.key") }}</span>
        <div class="key-line">
          <NSelect v-model:value="form.keyId" :options="keyOptions" :placeholder="t('profile.selectKey')" />
          <NButton @click="importFile">{{ t("profile.importFile") }}</NButton>
          <NButton :type="pasting ? 'primary' : 'default'" :secondary="pasting" @click="pasting = !pasting">{{ t("common.paste") }}</NButton>
          <NButton :disabled="!form.keyId" :title="t('profile.deleteKeyTitle')" @click="removeKey">
            <template #icon><Trash2 :size="14" /></template>
          </NButton>
        </div>
        <template v-if="pasting">
          <NInput v-model:value="pasted" type="textarea" class="mono" :autosize="{ minRows: 3, maxRows: 7 }" placeholder="-----BEGIN OPENSSH PRIVATE KEY-----" />
          <div class="key-line">
            <NInput v-model:value="pastedName" :placeholder="t('profile.keyNamePlaceholder')" />
            <NButton type="primary" :disabled="!pasted.trim()" @click="importKey({ content: pasted, name: pastedName })">{{ t("profile.saveKey") }}</NButton>
          </div>
        </template>
        <span class="hint">{{ t("profile.keyNote") }}</span>
      </div>

      <div v-if="form.auth !== 'agent'" class="field">
        <div class="label-line">
          <span class="label">{{ t(form.auth === "key" ? "profile.passphrase" : "profile.password") }}</span>
          <NCheckbox v-model:checked="form.saveSecret" size="small">{{ t(form.auth === "key" ? "profile.rememberPassphrase" : "profile.rememberPassword") }}</NCheckbox>
        </div>
        <NInput v-model:value="secret" type="password" show-password-on="click" :disabled="!form.saveSecret" :placeholder="secretPlaceholder" />
      </div>
      <div v-else class="notice"><Fingerprint :size="15" />{{ t("profile.agentHint") }}</div>

      <!-- 整块都能点；右边的开关只用来显示状态。 -->
      <div
        class="toggle"
        :class="{ on: form.keepAlive }"
        role="switch"
        tabindex="0"
        :aria-checked="!!form.keepAlive"
        @click="form.keepAlive = !form.keepAlive"
        @keydown.space.prevent="form.keepAlive = !form.keepAlive"
        @keydown.enter.prevent="form.keepAlive = !form.keepAlive"
      >
        <div class="toggle-icon"><HeartPulse :size="16" :stroke-width="1.9" /></div>
        <div class="toggle-text">
          <div class="toggle-title">{{ t("profile.keepAlive") }}</div>
          <div class="hint">{{ t("profile.keepAliveHint") }}</div>
        </div>
        <NSwitch :value="!!form.keepAlive" size="small" class="toggle-switch" tabindex="-1" />
      </div>

      <label class="field">
        <span class="label">{{ t("profile.note") }}</span>
        <NInput v-model:value="form.note" type="textarea" :autosize="{ minRows: 1, maxRows: 3 }" :placeholder="t('common.optional')" />
      </label>
    </div>

    <template #footer>
      <NSpace justify="end">
        <NButton @click="show = false">{{ t("common.cancel") }}</NButton>
        <NButton type="primary" :disabled="!valid" @click="save">{{ t("common.save") }}</NButton>
      </NSpace>
    </template>
  </NModal>
</template>

<style scoped>
.profile {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.label {
  color: var(--text-dim);
  font-size: 12px;
  font-weight: 600;
}

.required::after {
  content: "*";
  margin-left: 3px;
  color: var(--danger);
}

.problem {
  color: var(--danger);
  font-size: 12px;
}

.hint {
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.5;
}

/* 名称占大头，分组在它右边。 */
.pair {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 0.62fr);
  align-items: start;
  gap: 12px;
}

.address {
  display: grid;
  grid-template-columns: minmax(0, 0.62fr) auto minmax(0, 1.5fr) auto 78px;
  align-items: center;
  gap: 6px 8px;
}

.sep {
  color: var(--text-dim);
  font-family: var(--font-mono);
  font-weight: 600;
}

/* 三种登录方式平分整行。 */
.auth {
  display: flex;
}

.auth :deep(.n-radio-button) {
  flex: 1;
  padding: 0;
  text-align: center;
}

.auth-kind {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.key-line {
  display: flex;
  gap: 6px;
}

.key-line :deep(.n-select),
.key-line :deep(.n-input) {
  flex: 1;
  min-width: 0;
}

.mono :deep(textarea) {
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
}

/* 标签在左，“记住”在同一行的右边：它说的就是下面这个输入框。 */
.label-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.label-line :deep(.n-checkbox__label) {
  color: var(--text-dim);
  font-size: 12px;
}

.notice {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px 12px;
  border-radius: 10px;
  background: var(--fill);
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.5;
}

.notice svg {
  flex-shrink: 0;
}

.toggle {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--bg);
  outline: none;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s;
}

.toggle:hover,
.toggle:focus-visible {
  border-color: var(--accent-border);
}

.toggle.on {
  border-color: var(--accent-border);
  background: var(--accent-soft);
}

.toggle-icon {
  flex-shrink: 0;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--fill);
  color: var(--text-dim);
  transition: background 0.12s, color 0.12s;
}

.toggle.on .toggle-icon {
  background: var(--accent);
  color: #fff;
}

.toggle-text {
  flex: 1;
  min-width: 0;
}

.toggle-title {
  font-weight: 600;
  line-height: 18px;
}

.toggle-switch {
  flex-shrink: 0;
  pointer-events: none;
}
</style>
