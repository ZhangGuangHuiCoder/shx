<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/slogan-dark.svg" />
    <img src="src/assets/slogan-light.svg" width="240" alt="Shell Xplore" />
  </picture>
</p>

<p align="center">始于 Shell，不止于 Shell。</p>

<p align="center"><a href="README.md">English</a> · 简体中文</p>

Shell Xplore 是一款桌面 SSH 客户端，把与远程主机相关的操作集中在同一窗口：终端、文件、隧道，更多能力持续扩展。无需账号，不经云端，所有数据保存在本机。

## 特性

- **终端**：基于 xterm.js，支持查找和多会话；可选 tmux 保活，断线后程序继续运行。
- **文件**：SFTP 文件管理器，跟随终端 `cd`；目录树、收藏、权限修改、压缩解压、内置 Monaco 编辑器。
- **传输**：上传下载队列，支持暂停、续传和重试；目录可在服务器端打包后下载。
- **命令助手**：内置近 300 条常用命令，支持搜索、收藏和自定义。
- **主机**：分组、置顶、图标与颜色；支持密码、私钥和 ssh-agent 认证。
- **隧道**：经 SSH 主机转发本机流量，按路由规则自定义哪些网段走隧道、哪些直连；远端与本地网段重叠时由隧道接管。服务器端无需 root 权限或额外组件。
- **其他**：深色与浅色主题，中英文界面，配置支持加密备份与恢复。

## 下载

在 [Releases](https://github.com/ZhangGuangHuiCoder/shx/releases) 下载 macOS（通用）或 Windows的安装包。安装包未签名，首次打开时系统会要求确认。

应用启动时自动检查更新，也可通过菜单“检查更新…”手动检查。更新包下载后以内置公钥验证签名。

## 构建

依赖 Node ≥ 20.19、pnpm 和 Rust 稳定版。

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

发布新版本运行 `pnpm release`：升级补丁版本号并推送标签，由
[release.yml](.github/workflows/release.yml) 构建、签名并发布安装包。仓库 Secrets 需配置
`TAURI_SIGNING_PRIVATE_KEY`。请妥善备份私钥：一旦丢失，已安装的应用将无法更新。

## 数据与安全

- **存储**：主机配置、已保存的密码和导入的私钥存放在应用的 localStorage 中，未加密。请仅在可信设备上使用“记住密码”。
- **主机指纹**：首次连接时确认并由应用保存，指纹变更时发出警告。不读写 `~/.ssh` 和系统钥匙串。
- **备份**：`.shxbak` 文件以内置于程序的密钥加密，不等同于口令保护。
- **隧道**：开启时需管理员权限。提权的助手进程仅负责创建虚拟网卡和路由，关闭隧道或退出 Shell Xplore 时一并移除；SSH 连接与凭据留在未提权的主进程。
  - 仅转发 IPv4 TCP，DNS 查询改用 TCP 转发；其他 UDP 与 ICMP 被丢弃，IPv6 不经隧道，不修改 DNS 设置。
  - 规则里更具体的网段优先，未命中规则的流量按“其余流量”一项处理。走隧道的网段与本机所在网段重叠时（例如两边都是 192.168.1.0/24），隧道接管本地路由，该网段的地址指向远端；把规则表里的“本地网络”设为直连即可保留本地访问。虚拟机、容器使用的内部网段不受影响，除非规则点名。
  - macOS 的“免密助手”安装后是一个 root 所有的 setuid 程序。它无法识别调用者，本机其他程序也可调用它创建虚拟网卡和路由；可在隧道对话框里随时卸载。

## 技术栈

Tauri 2 · Rust · russh · russh-sftp · tokio · tun-rs · smoltcp · Vue 3 · Pinia · reka-ui · Tailwind CSS · xterm.js · Monaco · TypeScript · Vite

## 协议

[Apache-2.0](LICENSE)。随程序分发的第三方组件沿用各自的协议：

- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono)
- [SIL Open Font License 1.1](https://github.com/JetBrains/JetBrainsMono/blob/master/OFL.txt)
- [Wintun](https://www.wintun.net)
- [Prebuilt Binaries License](https://www.wintun.net/docs/license)
