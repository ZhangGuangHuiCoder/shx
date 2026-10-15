<script setup lang="ts">
// 新建/编辑主机对话框。
import { Fingerprint, KeyRound, LockKeyhole, Pencil, Plus, Trash2 } from "@lucide/vue";
import { open } from "@tauri-apps/plugin-dialog";
import { computed, reactive, ref, watch, type Component } from "vue";
import { errorMessage, type AuthKind, type Profile } from "@/shared/api";
import DialogHeader from "@/shared/ui-kit/DialogHeader.vue";
import SegmentedControl from "@/shared/ui-kit/SegmentedControl.vue";
import UiAutocomplete from "@/shared/ui-kit/UiAutocomplete.vue";
import UiButton from "@/shared/ui-kit/UiButton.vue";
import UiCheckbox from "@/shared/ui-kit/UiCheckbox.vue";
import UiInput from "@/shared/ui-kit/UiInput.vue";
import UiModal from "@/shared/ui-kit/UiModal.vue";
import UiNumberInput from "@/shared/ui-kit/UiNumberInput.vue";
import UiSelect from "@/shared/ui-kit/UiSelect.vue";
import UiSwitch from "@/shared/ui-kit/UiSwitch.vue";
import { t } from "@/shared/i18n";
import { confirm, message } from "@/shared/ui";
import { useApp } from "@/shared/store";
import { dangerText, dim, notice, sectionLabel } from "@/shared/ui-kit/styles";



const props = defineProps<{ profile: Profile | null }>();
const show = defineModel<boolean>("show", { required: true });
const app = useApp();

const blank = (): Profile => ({
  id: "", name: "", group: "", host: "", port: 22, username: "root", auth: "password", keyId: null, saveSecret: true, note: "", keepAlive: false,
});
const form = reactive<Profile>(blank());

const AUTH: { value: AuthKind; icon: Component; label: () => string }[] = [
  { value: "password", icon: LockKeyhole, label: () => t("profile.password") },
  { value: "key", icon: KeyRound, label: () => t("profile.key") },
  { value: "agent", icon: Fingerprint, label: () => "SSH Agent" },
];
const authOptions = computed(() => AUTH.map(({ value, icon, label }) => ({ value, icon, label: label() })));
const secret = ref("");
const pasting = ref(false);
const pasted = ref("");
const pastedName = ref("");

watch(show, (visible) => {
  if (!visible) return;
  // 以空白表单为底，清除上次编辑残留的可选字段。
  for (const key of Object.keys(form)) Reflect.deleteProperty(form, key);
  Object.assign(form, blank(), props.profile ? { ...props.profile } : {});
  secret.value = "";
  pasting.value = false;
});

const groupOptions = computed(() => [...new Set(app.profiles.map((p) => p.group).filter(Boolean))].map((name) => ({ label: name, value: name })));
const keyOptions = computed(() => app.keys.map((k) => ({ value: k.id, label: k.encrypted ? t("profile.hasPassphrase", { name: k.name }) : k.name })));
/** 名称为空时取主机地址，按最终值校验。 */
const nameError = computed<string | null>(() => {
  const effective = form.name.trim() || form.host.trim();
  if (!effective) return null;
  return app.profiles.some((p) => p.id !== form.id && p.name === effective) ? t("profile.nameTaken") : null;
});
const valid = computed(
  () =>
    !!form.host.trim() &&
    !!form.username.trim() &&
    // 端口清空时为 null。
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
  <UiModal v-model:show="show" :mask-closable="false" class="w-lg">
    <template #header>
      <DialogHeader :icon="profile ? Pencil : Plus" :title="t(profile ? 'profile.editTitle' : 'sidebar.new')" :detail="profile?.name" />
    </template>

    <div class="flex flex-col gap-4">
      <div class="grid grid-cols-5 items-start gap-3">
        <label class="col-span-3 flex min-w-0 flex-col gap-2">
          <span :class="sectionLabel">{{ t("common.name") }}</span>
          <UiInput v-model:value="form.name" :status="nameError ? 'error' : undefined" :placeholder="t('profile.namePlaceholder')" />
          <span v-if="nameError" :class="[dangerText, 'text-xs']">{{ nameError }}</span>
        </label>
        <div class="col-span-2 flex min-w-0 flex-col gap-2">
          <span :class="sectionLabel">{{ t("profile.group") }}</span>
          <UiAutocomplete v-model:value="form.group" :options="groupOptions" clearable :placeholder="t('common.optional')" />
        </div>
      </div>

      <!-- 与 ssh 写法一致：用户名 @ 主机 : 端口。 -->
      <div class="flex items-end gap-2">
        <label class="flex w-1/4 min-w-0 flex-col gap-2">
          <span :class="sectionLabel">{{ t("profile.username") }}<span class="ml-1 text-red-500">*</span></span>
          <UiInput v-model:value="form.username" />
        </label>
        <span :class="[dim, 'flex h-9 items-center font-mono font-semibold']">@</span>
        <label class="flex min-w-0 flex-1 flex-col gap-2">
          <span :class="sectionLabel">{{ t("profile.host") }}<span class="ml-1 text-red-500">*</span></span>
          <UiInput v-model:value="form.host" :placeholder="t('profile.hostPlaceholder')" />
        </label>
        <span :class="[dim, 'flex h-9 items-center font-mono font-semibold']">:</span>
        <label class="flex w-20 shrink-0 flex-col gap-2">
          <span :class="sectionLabel">{{ t("profile.port") }}</span>
          <UiNumberInput v-model:value="form.port" :min="1" :max="65535" placeholder="22" />
        </label>
      </div>

      <div class="flex min-w-0 items-center justify-between gap-3">
        <span :class="sectionLabel">{{ t("profile.auth") }}</span>
        <SegmentedControl v-model="form.auth" :options="authOptions" :label="t('profile.auth')" />
      </div>

      <div v-if="form.auth === 'key'" class="flex min-w-0 flex-col gap-2">
        <span :class="sectionLabel">{{ t("profile.key") }}<span class="ml-1 text-red-500">*</span></span>
        <div class="flex gap-2">
          <UiSelect v-model:value="form.keyId" class="min-w-0 flex-1" :options="keyOptions" :placeholder="t('profile.selectKey')" :empty-text="t('profile.noKeys')" />
          <UiButton @click="importFile">{{ t("profile.importFile") }}</UiButton>
          <UiButton :selected="pasting" @click="pasting = !pasting">{{ t("common.paste") }}</UiButton>
          <UiButton :disabled="!form.keyId" :title="t('profile.deleteKeyTitle')" @click="removeKey">
            <template #icon><Trash2 :size="14" /></template>
          </UiButton>
        </div>
        <template v-if="pasting">
          <UiInput v-model:value="pasted" type="textarea" class="font-mono text-xs/relaxed!" :autosize="{ minRows: 3, maxRows: 7 }" placeholder="-----BEGIN OPENSSH PRIVATE KEY-----" />
          <div class="flex gap-2">
            <UiInput v-model:value="pastedName" class="min-w-0 flex-1" :placeholder="t('profile.keyNamePlaceholder')" />
            <UiButton type="primary" :disabled="!pasted.trim()" @click="importKey({ content: pasted, name: pastedName })">{{ t("profile.saveKey") }}</UiButton>
          </div>
        </template>
        <span :class="[dim, 'text-xs/normal']">{{ t("profile.keyNote") }}</span>
      </div>

      <div v-if="form.auth !== 'agent'" class="flex min-w-0 flex-col gap-2">
        <div class="flex items-center justify-between gap-3">
          <span :class="sectionLabel">{{ t(form.auth === "key" ? "profile.passphrase" : "profile.password") }}</span>
          <UiCheckbox v-model:checked="form.saveSecret" :class="[dim, 'text-xs']">{{ t(form.auth === "key" ? "profile.rememberPassphrase" : "profile.rememberPassword") }}</UiCheckbox>
        </div>
        <UiInput v-model:value="secret" type="password" show-password-on="click" :disabled="!form.saveSecret" :placeholder="secretPlaceholder" />
      </div>
      <div v-else :class="notice"><Fingerprint :size="15" class="shrink-0" />{{ t("profile.agentHint") }}</div>

      <!-- 只有开关本身可点击，文字不切换。 -->
      <div class="flex min-w-0 items-center gap-3">
        <div class="min-w-0 flex-1">
          <div id="profile-keep-alive" :class="sectionLabel">{{ t("profile.keepAlive") }}</div>
          <div :class="[dim, 'mt-0.5 text-xs/normal']">{{ t("profile.keepAliveHint") }}</div>
        </div>
        <UiSwitch v-model:value="form.keepAlive" class="shrink-0" aria-labelledby="profile-keep-alive" />
      </div>

      <label class="flex min-w-0 flex-col gap-2">
        <span :class="sectionLabel">{{ t("profile.note") }}</span>
        <UiInput v-model:value="form.note" type="textarea" :autosize="{ minRows: 1, maxRows: 3 }" :placeholder="t('common.optional')" />
      </label>
    </div>

    <template #footer>
      <div class="flex justify-end gap-2">
        <UiButton @click="show = false">{{ t("common.cancel") }}</UiButton>
        <UiButton type="primary" :disabled="!valid" @click="save">{{ t("common.save") }}</UiButton>
      </div>
    </template>
  </UiModal>
</template>
