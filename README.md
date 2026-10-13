<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/slogan-dark.svg" />
    <img src="src/assets/slogan-light.svg" width="240" alt="shx — Shell × Files." />
  </picture>
</p>

<p align="center">
  One Window. Total Control.
</p>

<p align="center">English · <a href="README.zh-CN.md">简体中文</a></p>

Shx is a desktop SSH client that puts the terminal and an SFTP file manager side by side in one
window. No account, no cloud: everything is stored on your machine.

## Features

- **Terminal** — xterm.js with search and multiple sessions; optional tmux keep-alive so programs
  survive a dropped connection.
- **Files** — an SFTP file manager that follows the terminal's `cd`: folder tree, favorites,
  permission editing, compress and extract, built-in Monaco editor.
- **Transfers** — an upload and download queue with pause, resume and retry; folders can be
  packed on the server before downloading.
- **Command assistant** — nearly 300 built-in commands, with search, favorites and custom commands.
- **Hosts** — groups, pinning, icons and colors; password, private key and ssh-agent
  authentication.
- **Tunnel** — forwards this computer's traffic through an SSH host, globally or per network.
  No root access or extra software on the server.
- **Also** — light and dark themes, English and Chinese, encrypted configuration backups.

## Download

Get the installer for macOS (universal) or Windows from
[Releases](https://github.com/ZhangGuangHuiCoder/shx/releases). The installers are unsigned, so
the system asks for confirmation on first launch.

The app checks for updates at launch, and on demand from the Check for Updates… menu item.
Downloaded updates are verified against the public key built into the app.

## Build

Requires Node ≥ 20.19, pnpm and stable Rust.

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

To release, run `pnpm release`: it bumps the patch version and pushes the tag, and
[release.yml](.github/workflows/release.yml) builds, signs and publishes the installers. The
repository needs the `TAURI_SIGNING_PRIVATE_KEY` secret. Back up that key: without it, installed
copies can no longer be updated.

## Data & Security

- **Storage**: host profiles, remembered passwords and imported private keys are kept in the app's
  localStorage, unencrypted. Use "remember password" only on a trusted machine.
- **Host keys**: confirmed on first connection and saved by the app; a changed key triggers a
  warning. `~/.ssh` and the system keychain are not read or written.
- **Backup**: `.shxbak` files are encrypted with a key built into the app, so this is not
  password protection.
- **Tunnel**: needs administrator permission to start. The elevated helper only creates a virtual
  network interface and routes, which are removed when the tunnel is turned off or shx quits; the
  SSH connection and credentials stay in the unprivileged main process.
  - Forwards IPv4 TCP only, with DNS queries sent over TCP; other UDP and ICMP are dropped, IPv6
    bypasses the tunnel, and DNS settings are not changed.
  - On macOS, "Start without a password" installs a root-owned setuid helper. It cannot identify
    its caller, so other programs on this computer can also use it to create a virtual interface
    and routes. Turning the option off uninstalls it.

## Stack

Tauri 2 · Rust · russh · russh-sftp · tokio · tun-rs · smoltcp · Vue 3 · Pinia · Naive UI · xterm.js · Monaco · TypeScript · Vite

## License

[Apache-2.0](LICENSE). Bundled third-party components keep their own licenses:

- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) —
  [SIL Open Font License 1.1](public/licenses/JetBrainsMono-OFL.txt)
- [Wintun](https://www.wintun.net) 0.14.1 (Windows tunnel driver, shipped unmodified) —
  [Prebuilt Binaries License](public/licenses/Wintun-LICENSE.txt)
