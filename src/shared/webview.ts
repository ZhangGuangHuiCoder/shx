// 让 webview 表现得像桌面应用。
import { ClipboardPaste, Copy, Scissors, TextSelect } from "@lucide/vue";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { contextMenu, openContextMenu, type MenuEntry } from "@/shared/contextMenu";
import { t } from "@/shared/i18n";

const TEXT_FIELDS = "input, textarea, [contenteditable]";

/** 关闭自动更正等，保证输入原样保存。 */
function plain(field: Element) {
  field.setAttribute("autocomplete", "off");
  field.setAttribute("autocorrect", "off");
  field.setAttribute("autocapitalize", "off");
  field.setAttribute("spellcheck", "false");
}

function plainAll(root: Element | Document) {
  if (root instanceof Element && root.matches(TEXT_FIELDS)) plain(root);
  root.querySelectorAll(TEXT_FIELDS).forEach(plain);
}

const TEXT_INPUT_TYPES = ["text", "password", "search", "url", "tel"];

type TextField = HTMLInputElement | HTMLTextAreaElement;

/** 右键处的文本框；终端、编辑器自行处理，排除。 */
function editableField(target: EventTarget | null): TextField | null {
  const field = target instanceof Element ? target.closest<TextField>("input, textarea") : null;
  if (!field || field.closest(".xterm, .monaco-editor")) return null;
  if (field instanceof HTMLInputElement && !TEXT_INPUT_TYPES.includes(field.type)) return null;
  return field;
}

/** 文本框编辑菜单，以右键时的选区为准。 */
function fieldMenu(field: TextField): MenuEntry[] {
  const start = field.selectionStart ?? 0;
  const end = field.selectionEnd ?? 0;
  const selected = field.value.slice(start, end);
  const editable = !field.readOnly && !field.disabled;
  // 密码不允许复制到剪贴板。
  const copyable = selected !== "" && field.type !== "password";
  /** 替换选区并触发 input 事件。 */
  const replace = (text: string) => {
    field.focus();
    field.setRangeText(text, start, end, "end");
    field.dispatchEvent(new Event("input", { bubbles: true }));
  };
  return [
    {
      label: t("common.cut"),
      icon: Scissors,
      disabled: !copyable || !editable,
      action: async () => {
        await writeText(selected);
        replace("");
      },
    },
    { label: t("common.copy"), icon: Copy, disabled: !copyable, action: () => writeText(selected) },
    {
      label: t("common.paste"),
      icon: ClipboardPaste,
      disabled: !editable,
      action: async () => {
        const text = await readText().catch(() => "");
        if (text) replace(text);
      },
    },
    "divider",
    {
      label: t("common.selectAll"),
      icon: TextSelect,
      disabled: field.value === "",
      action: () => {
        field.focus();
        field.select();
      },
    },
  ];
}

export function hardenWebview() {
  // 屏蔽浏览器右键菜单，文本框上显示编辑菜单；自带菜单的组件会先处理。
  document.addEventListener("contextmenu", (event) => {
    if (event.defaultPrevented) return;
    event.preventDefault();
    const field = editableField(event.target);
    if (field) openContextMenu(event, fieldMenu(field));
    else contextMenu.show = false;
  });

  // 对后续动态创建的文本框同样生效。
  document.documentElement.setAttribute("spellcheck", "false");
  plainAll(document);
  new MutationObserver((records) => {
    for (const record of records) {
      record.addedNodes.forEach((node) => node instanceof Element && plainAll(node));
    }
  }).observe(document.documentElement, { childList: true, subtree: true });

  // 拒绝系统文本替换（智能引号、破折号等）。
  document.addEventListener(
    "beforeinput",
    (event) => {
      if (event.inputType === "insertReplacementText") event.preventDefault();
    },
    true,
  );
}
