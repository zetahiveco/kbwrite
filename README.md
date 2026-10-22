# KbWrite

A local-first, Obsidian-style knowledge base. The interface is React, the block editor is [BlockNote](https://www.blocknotejs.org/), and the desktop shell is [Tauri](https://tauri.app/).

Your notes are plain **markdown files** in a folder on disk (your *vault*), so they stay yours: portable, diffable, and easy to back up.

Source, issues, and downloads: [github.com/zetahiveco/kbwrite](https://github.com/zetahiveco/kbwrite)

## Features

- **Markdown vault** — every note is a `.md` file on disk. Point KbWrite at any folder.
- **BlockNote editor** — type `/` for headings, lists, to-dos, quotes, code, tables, images, video, audio, PDF, CSV, DOCX, and XLSX. Those files are stored in the vault.
- **Wikilinks** — `/` → **Link to note**, or type `[[Note]]` yourself.
- **Graph view** — notes are nodes, `[[links]]` and local markdown links are edges.
- **MCP server** — start it in Settings, then connect Claude, Codex, or OpenCode so they can read and write the vault.
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

## Getting started

### Download

Prebuilt apps for macOS, Windows, and Linux are on the
[Releases](https://github.com/zetahiveco/kbwrite/releases) page.

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
