<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/slogan-dark.svg" />
    <img src="src/assets/slogan-light.svg" width="240" alt="shx — Shell × Files." />
  </picture>
</p>

<p align="center">终端 × 文件，简单易用。</p>

<p align="center"><a href="README.md">English</a> · 简体中文</p>

Shx 是一个桌面 SSH 客户端：终端和 SFTP 文件管理器并排在同一个窗口里。不需要账号，不经过云端，
所有配置都保存在本机。

## 特性

- **终端**：xterm.js，支持查找；可选保活，shell 运行在服务器的 tmux 里，断线后程序继续运行。
  一台主机可以开多个会话。
- **文件**：跟随终端 `cd` 的 SFTP 文件管理器，带目录树、权限修改、压缩解压和内置的 Monaco 编辑器。
- **传输**：上传下载队列，可暂停、续传、重试；目录可以先在服务器上打包，再作为一个压缩包下载。
- **命令助手**：内置近 300 条常用命令，分 15 类，可搜索、收藏，也可以添加自己的命令。
- **主机**：分组、置顶、图标和颜色；支持密码、私钥和 ssh-agent 登录。
- **本地优先**：深浅主题，中英文界面，配置可以备份成加密文件、再恢复回来。

## 下载

在 [Releases](https://github.com/ZhangGuangHuiCoder/shx/releases) 下载 macOS（通用）或 Windows
的安装包。安装包没有签名，第一次打开时系统会要求确认。

## 构建

需要 Node ≥ 20.19、pnpm、Rust 稳定版。

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

## 更新和发布

应用不会自己检查更新。菜单里的“检查更新…”（macOS 在 shx 菜单，Windows 在“帮助”菜单）会到
[GitHub Releases](https://github.com/ZhangGuangHuiCoder/shx/releases) 上看有没有新版本，
有就提示更新。更新包下载后用内置的公钥验证签名。

发布新版本：

```bash
pnpm version patch
git push --follow-tags
```

`v*` 标签会触发 [release.yml](.github/workflows/release.yml)：构建 macOS 和 Windows 的安装包，
给更新包签名后发布。仓库的 Secrets 里需要有 `TAURI_SIGNING_PRIVATE_KEY`（签名私钥的内容）。
私钥请妥善备份：丢了以后，已安装的应用就无法再更新。

## 数据和安全

- **存储**：主机配置、记住的密码和导入的私钥保存在应用的 localStorage 里（键名以 `shx.` 开头），
  未加密。请只在信任的电脑上使用“记住密码”。
- **主机指纹**：首次连接时确认并由应用保存，指纹变化时提醒。不读写 `~/.ssh` 和系统钥匙串。
- **备份**：文件 → 备份配置… / 恢复配置… 读写加密的 `.shxbak` 文件。密钥内置在程序中，不等同于口令保护。

## 技术栈

Tauri 2 · Rust · russh · russh-sftp · tokio · Vue 3 · Pinia · Naive UI · xterm.js · Monaco · TypeScript · Vite

## 协议

[Apache-2.0](LICENSE)
