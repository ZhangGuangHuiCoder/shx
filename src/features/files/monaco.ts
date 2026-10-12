// 首次打开文件时才加载（动态导入）：Monaco 有好几 MB。
import * as monaco from "monaco-editor";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";
import JsonWorker from "monaco-editor/language/json/json.worker.js?worker";

self.MonacoEnvironment = {
  getWorker: (_id, label) => (label === "json" ? new JsonWorker() : new EditorWorker()),
};

// 这里只是改服务器上的文件，不是 IDE：JS/TS、CSS、HTML 的语言服务（补全、悬停、报错）没有打进来，
// vite.config.ts 把它们换成了空模块——光 TypeScript 的 worker 就有 6.8 MB。
// 语法高亮不靠它们，不受影响；JSON 的校验留着，改配置文件时用得上。
//
// 开发时 Vite 预打包的 Monaco 不走那个替换，语言服务还在：把它们的功能全部关掉，
// 和正式构建的表现一致。正式构建里这三个是空模块，下面的循环什么都不做。
const services: object[] = [monaco.typescript, monaco.css, monaco.html];
for (const defaults of services.flatMap((service) => Object.values(service))) {
  (defaults as { setModeConfiguration?: (configuration: object) => void } | null)?.setModeConfiguration?.({});
}

/** Monaco 无法按文件名识别的配置类文件所用的语言。 */
const EXTRA_LANGUAGES: [RegExp, string][] = [
  [/\.(conf|cfg|cnf|service|timer|socket|env|properties|desktop|repo)$/i, "ini"],
  [/^\.?(bashrc|bash_profile|bash_aliases|zshrc|zprofile|profile|zshenv)$/i, "shell"],
  [/^(crontab|fstab|hosts|authorized_keys|known_hosts)$/i, "plaintext"],
];

export function languageFor(fileName: string): string | undefined {
  return EXTRA_LANGUAGES.find(([pattern]) => pattern.test(fileName))?.[1];
}

export { monaco };
