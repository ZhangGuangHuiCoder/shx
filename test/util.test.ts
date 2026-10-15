import { afterEach, describe, expect, it } from "vitest";
import { systemLocale } from "@/shared/i18n.ts";
import {
  baseName,
  formatDuration,
  formatMode,
  formatOctal,
  formatShortTime,
  formatSize,
  formatTime,
  isArchive,
  joinPath,
  parentPath,
  ruleMatcher,
  shellQuote,
  statusDotColor,
} from "@/shared/util.ts";
import { locale } from "@/shared/i18n.ts";

describe("formatSize", () => {
  it("不足 1KB 显示整数字节", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1023)).toBe("1023 B");
  });

  it("按 1024 进位并保留一位小数", () => {
    expect(formatSize(1024)).toBe("1.0 KB");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(1024 * 1024)).toBe("1.0 MB");
    expect(formatSize(2.5 * 1024 * 1024 * 1024)).toBe("2.5 GB");
  });

  it("三位数以上不保留小数", () => {
    expect(formatSize(100 * 1024)).toBe("100 KB");
    expect(formatSize(99.5 * 1024)).toBe("99.5 KB");
  });

  it("最大单位止于 TB", () => {
    expect(formatSize(2 * 1024 ** 4)).toBe("2.0 TB");
    expect(formatSize(4096 * 1024 ** 4)).toBe("4096 TB");
  });
});

describe("formatTime / formatShortTime", () => {
  // 用本地时区构造，避免 UTC 偏移导致的断言不稳定。
  const d = new Date(2024, 2, 5, 9, 7); // 2024-03-05 09:07 本地时间
  const epoch = Math.floor(d.getTime() / 1000);

  it("空值返回空字符串", () => {
    expect(formatTime(null)).toBe("");
    expect(formatTime(0)).toBe("");
    expect(formatShortTime(null)).toBe("");
  });

  it("formatTime 是完整的年-月-日 时:分", () => {
    expect(formatTime(epoch)).toBe("2024-03-05 09:07");
  });

  it("formatShortTime 今年只写月-日 时:分", () => {
    expect(formatShortTime(epoch, new Date(2024, 5, 1))).toBe("03-05 09:07");
  });

  it("formatShortTime 往年只写年-月-日", () => {
    expect(formatShortTime(epoch, new Date(2025, 0, 1))).toBe("2024-03-05");
  });
});

describe("formatDuration", () => {
  afterEach(() => (locale.value = systemLocale()));

  it("非法值返回空字符串", () => {
    expect(formatDuration(-1)).toBe("");
    expect(formatDuration(Number.POSITIVE_INFINITY)).toBe("");
    expect(formatDuration(Number.NaN)).toBe("");
  });

  it("英文文案", () => {
    locale.value = "en";
    expect(formatDuration(0)).toBe("0s");
    expect(formatDuration(30)).toBe("30s");
    expect(formatDuration(65)).toBe("1m 5s");
    expect(formatDuration(3600)).toBe("1h 0m");
    expect(formatDuration(3661)).toBe("1h 1m");
  });

  it("中文文案", () => {
    locale.value = "zh";
    expect(formatDuration(30)).toBe("30秒");
    expect(formatDuration(65)).toBe("1分5秒");
    expect(formatDuration(3600)).toBe("1小时0分");
  });
});

describe("formatMode", () => {
  it("null 返回空字符串", () => {
    expect(formatMode(null, "file")).toBe("");
  });

  it("普通权限位", () => {
    expect(formatMode(0o755, "dir")).toBe("drwxr-xr-x");
    expect(formatMode(0o644, "file")).toBe("-rw-r--r--");
    expect(formatMode(0o777, "link")).toBe("lrwxrwxrwx");
  });

  it("setuid/setgid/sticky 的小写位（同时有执行位）", () => {
    expect(formatMode(0o4755, "file")).toBe("-rwsr-xr-x");
    expect(formatMode(0o2755, "file")).toBe("-rwxr-sr-x");
    expect(formatMode(0o1777, "dir")).toBe("drwxrwxrwt");
  });

  it("特殊位没有执行位时大写", () => {
    expect(formatMode(0o4644, "file")).toBe("-rwSr--r--");
    expect(formatMode(0o1666, "file")).toBe("-rw-rw-rwT");
  });
});

describe("formatOctal", () => {
  it("null 返回空字符串", () => {
    expect(formatOctal(null)).toBe("");
  });

  it("至少三位，带特殊位时四位", () => {
    expect(formatOctal(0o7)).toBe("007");
    expect(formatOctal(0o644)).toBe("644");
    expect(formatOctal(0o755)).toBe("755");
    expect(formatOctal(0o4755)).toBe("4755");
  });
});

describe("路径工具", () => {
  it("joinPath 只在缺少斜杠时补", () => {
    expect(joinPath("/a/b", "c")).toBe("/a/b/c");
    expect(joinPath("/a/b/", "c")).toBe("/a/b/c");
  });

  it("parentPath 逐级向上，根目录的直接子项回到根", () => {
    expect(parentPath("/a/b/c")).toBe("/a/b");
    expect(parentPath("/a")).toBe("/");
    expect(parentPath("/")).toBe("/");
    expect(parentPath("/a/b//")).toBe("/a");
  });

  it("baseName 同时认斜杠和反斜杠，忽略结尾分隔符", () => {
    expect(baseName("/a/b/c.txt")).toBe("c.txt");
    expect(baseName("/a/b/")).toBe("b");
    expect(baseName("C:\\x\\y")).toBe("y");
  });
});

describe("shellQuote", () => {
  it("普通字符串整体加单引号", () => {
    expect(shellQuote("abc")).toBe("'abc'");
  });

  it("单引号按 shell 惯例拆成四段", () => {
    expect(shellQuote("a'b")).toBe("'a'\\''b'");
  });
});

describe("ruleMatcher", () => {
  const match = ruleMatcher([".conf", "*.log", "Dockerfile", "  ", "nginx*"]);

  it("扩展名按结尾匹配且不区分大小写", () => {
    expect(match("nginx.CONF")).toBe(true);
    expect(match("a.conf.bak")).toBe(false);
  });

  it("通配符 * 与 ?", () => {
    expect(match("error.log")).toBe(true);
    expect(match("nginx.conf")).toBe(true);
    expect(match("f?o.txt")).toBe(false);
    expect(ruleMatcher(["f?o.txt"])("foo.txt")).toBe(true);
  });

  it("完整文件名精确匹配", () => {
    expect(match("dockerfile")).toBe(true);
    expect(match("Dockerfilex")).toBe(false);
  });

  it("规则中的正则字符按字面量处理", () => {
    expect(ruleMatcher(["a+b.txt"])("a+b.txt")).toBe(true);
    expect(ruleMatcher(["a+b.txt"])("axb.txt")).toBe(false);
  });
});

describe("isArchive", () => {
  it("常见压缩格式（含双后缀、大小写混用）", () => {
    expect(isArchive("a.zip")).toBe(true);
    expect(isArchive("a.tar.gz")).toBe(true);
    expect(isArchive("A.TGZ")).toBe(true);
    expect(isArchive("backup.TAR.BZ2")).toBe(true);
  });

  it("普通文件不是压缩包", () => {
    expect(isArchive("a.txt")).toBe(false);
    expect(isArchive("a.gz.txt")).toBe(false);
  });
});

describe("statusDotColor", () => {
  it("三种状态对应三个颜色类", () => {
    expect(statusDotColor("connected")).toBe("bg-emerald-500");
    expect(statusDotColor("closed")).toBe("bg-red-500");
    expect(statusDotColor("connecting")).toBe("bg-amber-500");
  });
});
