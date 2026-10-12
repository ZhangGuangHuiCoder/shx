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

- **Terminal** — xterm.js with search, and optional keep-alive: the shell runs in tmux on the
  server, so programs survive a dropped connection. One host can hold several sessions.
- **Files** — an SFTP file manager that follows the terminal's `cd`, with a folder tree, permissions,
  compress and extract, and a built-in Monaco editor.
- **Transfers** — a queue for uploads and downloads with pause, resume and retry; folders can be
  packed on the server and downloaded as one archive.
- **Command assistant** — nearly 300 built-in commands in 15 groups, with search, favourites and
  your own commands.
- **Hosts** — groups, pinning, icons and colours; sign in with a password, a private key or
  ssh-agent.
- **Local first** — light and dark themes, English and Chinese, encrypted configuration backups.

## Download

Get the installer for macOS (universal) or Windows from
[Releases](https://github.com/ZhangGuangHuiCoder/shx/releases). The installers are not signed, so
the system asks for confirmation the first time you open the app.

## Build

Needs Node ≥ 20.19, pnpm and stable Rust.

```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

## Updates & Releases

The app never checks for updates by itself. Check for Updates… (in the shx menu on macOS, the Help
menu on Windows) looks on [GitHub Releases](https://github.com/ZhangGuangHuiCoder/shx/releases) for
a newer version and offers to install it. Downloaded updates are verified against the public key
built into the app.

To publish a new version:

```bash
pnpm version patch
git push --follow-tags
```

A `v*` tag triggers [release.yml](.github/workflows/release.yml), which builds the macOS and Windows
installers, signs the update packages and publishes the release. The repository needs the
`TAURI_SIGNING_PRIVATE_KEY` secret (the content of the signing key). Keep a backup of that key:
without it, installed copies can no longer update themselves.

## Data & Security

- **Storage**: host profiles, remembered passwords and imported private keys are kept in the app's
  localStorage (`shx.*` keys), unencrypted. Use "remember password" only on a machine you trust.
- **Host keys**: confirmed on first connection and saved by the app; a changed key triggers a
  warning. `~/.ssh` and the system keychain are not read or written.
- **Backup**: File → Back Up Configuration… / Restore Configuration… reads and writes an encrypted
  `.shxbak` file. The key is built into the app, so this is not password protection.

## Stack

Tauri 2 · Rust · russh · russh-sftp · tokio · Vue 3 · Pinia · Naive UI · xterm.js · Monaco · TypeScript · Vite

## License

[Apache-2.0](LICENSE)
