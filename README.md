# KbWrite

A local-first, Obsidian-style knowledge base. The interface is React, the block editor is [BlockNote](https://www.blocknotejs.org/), and the desktop shell is [Tauri](https://tauri.app/).

Your notes are plain **markdown files** in a folder on disk (your *vault*), so they stay yours: portable, diffable, and easy to back up.

Site: [kbwrite.ink](https://kbwrite.ink). Source, issues, and downloads: [github.com/zetahiveco/kbwrite](https://github.com/zetahiveco/kbwrite)

## Features

- **Markdown vault** — every note is a `.md` file on disk. Point KbWrite at any folder.
- **BlockNote editor** — type `/` for headings, lists, to-dos, quotes, code, tables, images, video, audio, PDF, CSV, DOCX, and XLSX. Those files are stored in the vault.
- **Wikilinks** — `/` → **Link to note**, or type `[[Note]]` yourself.
- **Graph view** — notes are nodes, `[[links]]` and local markdown links are edges.
- **MCP server** — start it in Settings, then connect Claude, Codex, or OpenCode so they can read and write the vault.
- **Rust plugins** — install from a folder or a GitHub URL in Settings. A plugin can add commands, dialogs, and block types, the same kinds of extension Obsidian community plugins use. [Plugin docs](https://kbwrite.ink/plugins).
- **Updates** — KbWrite → Check for Updates. A toast at the bottom right checks, installs, then offers Restart.
- **Light and dark mode.**
- **Raw markdown** — switch to the source whenever you want.
- **File tree** — create, rename, and delete notes and folders.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Cmd/Ctrl + N` | New note |
| `Cmd/Ctrl + S` | Save note |
| `Cmd/Ctrl + ,` | Settings |
| `/` | Block menu |

## Install

One command installs the latest release. You can also download a build from
[Releases](https://github.com/zetahiveco/kbwrite/releases).

### macOS

Apple silicon and Intel are detected. The app is copied into `/Applications`, or `~/Applications` if that folder is not writable.

```bash
curl -fsSL https://kbwrite.ink/install.sh | sh
```

### Linux

Installs an AppImage to `~/.local/bin/kbwrite`. If the release only has a `.deb`, the script installs that package instead (sudo may ask for a password).

```bash
curl -fsSL https://kbwrite.ink/install.sh | sh
```

If `kbwrite` is not found, add `~/.local/bin` to your `PATH`.

### Windows

In PowerShell:

```powershell
irm https://kbwrite.ink/install.ps1 | iex
```

This downloads the latest installer and opens it. x64 and ARM64 are detected.

## Getting started

### Build from source

```bash
npm install
npm run tauri dev
```

A release build:

```bash
npm run tauri build
```

### Configure

- **Vault folder** — KbWrite creates a default vault at `~/Documents/KbWriteVault`. Change it in **Settings** (`Cmd/Ctrl+,`).
- **MCP** — in Settings, press **Start**. Copy the command for Claude, Codex, or OpenCode and run it once. The server listens on `127.0.0.1:47321` only while KbWrite is open.

Settings are stored in `~/.kbwrite/settings.json`. Existing settings files from earlier KbWrite versions still load.

## Tech stack

- [Tauri](https://tauri.app/) for the desktop shell and filesystem access
- [React](https://react.dev/) for the interface
- [BlockNote](https://www.blocknotejs.org/) for the block editor
- Plain markdown files for storage

## License

MIT © Harish Deivanayagam

---

Made by [harishdeivanayagam](https://github.com/harishdeivanayagam) · [zetahive.co](https://zetahive.co)
