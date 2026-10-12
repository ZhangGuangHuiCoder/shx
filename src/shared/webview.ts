// 让 webview 的表现像桌面应用，而不是网页。
import { ClipboardPaste, Copy, Scissors, TextSelect } from "@lucide/vue";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { contextMenu, openContextMenu, type MenuEntry } from "./contextMenu";
import { t } from "./i18n";

const TEXT_FIELDS = "input, textarea, [contenteditable]";

/** 主机名、路径和密码必须按输入的原样保存。 */
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

/** 存放自由文本、有选区可供剪切、复制或替换的输入框类型。 */
const TEXT_INPUT_TYPES = ["text", "password", "search", "url", "tel"];

type TextField = HTMLInputElement | HTMLTextAreaElement;

/** 右键点击处的文本框；归组件（终端、编辑器）自己管的除外。 */
function editableField(target: EventTarget | null): TextField | null {
  const field = target instanceof Element ? target.closest<TextField>("input, textarea") : null;
  if (!field || field.closest(".xterm, .monaco-editor")) return null;
  if (field instanceof HTMLInputElement && !TEXT_INPUT_TYPES.includes(field.type)) return null;
  return field;
}

/** 文本框的剪切 / 复制 / 粘贴 / 全选菜单，以右键点击那一刻的状态为准。 */
function fieldMenu(field: TextField): MenuEntry[] {
  const start = field.selectionStart ?? 0;
  const end = field.selectionEnd ?? 0;
  const selected = field.value.slice(start, end);
  const editable = !field.readOnly && !field.disabled;
  // 密码绝不能经剪贴板离开输入框。
  const copyable = selected !== "" && field.type !== "password";
  /** 替换选中的内容，并通知框架值已变化。 */
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
      // 剪贴板里不是文本时读取会失败，此时什么都不做。
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
  // 替换浏览器的右键菜单（重新加载、检查、查询…）：默认不显示任何菜单，
  // 文本框上显示常用的编辑命令。有自己菜单的组件会先一步处理这个事件。
  document.addEventListener("contextmenu", (event) => {
    if (event.defaultPrevented) return;
    event.preventDefault();
    const field = editableField(event.target);
    if (field) openContextMenu(event, fieldMenu(field));
    else contextMenu.show = false;
  });

  // 关闭所有文本框的自动更正、自动补全、自动大写和拼写检查，
  // 包括 UI 库之后才创建的那些。
  document.documentElement.setAttribute("spellcheck", "false");
  plainAll(document);
  new MutationObserver((records) => {
    for (const record of records) {
      record.addedNodes.forEach((node) => node instanceof Element && plainAll(node));
    }
  }).observe(document.documentElement, { childList: true, subtree: true });

  // 仍然漏过来的修改（系统文本替换、智能引号和破折号）表现为对用户已输入文字的替换，
  // 一律拒绝。
  document.addEventListener(
    "beforeinput",
    (event) => {
      if (event.inputType === "insertReplacementText") event.preventDefault();
    },
    true,
  );
}
