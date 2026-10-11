# KbWrite

A local-first, Obsidian-style knowledge base written in **native Rust** with
[egui](https://github.com/emilk/egui) / **eframe** — no Electron, no browser, just one fast binary.

Your notes are plain **markdown files** in a folder on disk (your *vault*), so they stay yours:
portable, diffable, and easy to back up.

## Features

- **Markdown vault** — every note is a `.md` file on disk. Point KbWrite at any folder.
- **Notion-style `/` blocks** — type `/` on an empty line to open the block menu:
  - Heading 1 / 2 / 3
  - Bullet list, numbered list
  - To-do list with checkboxes
  - Quote, code block, divider
  - Smart list continuation: `Enter` continues a list, `Enter` on an empty item exits it.
- **Inline AI text** — works with any OpenAI-compatible endpoint (OpenAI, Ollama, LM Studio,
  OpenRouter, …). Select a block, press `Cmd/Ctrl+I`, type an instruction like
  *"fix grammar"*, *"summarize"* or *"turn into bullets"* — the result is inserted right below.
- **Phosphor icons** throughout, via [egui_phosphor_icons](https://docs.rs/egui_phosphor_icons/latest/egui_phosphor_icons/).
- **Light & dark mode** — one-click theme switch in the sidebar.
- **Editor / Raw tabs** — write in the block editor, or drop down to raw markdown anytime.
- **Fast & native** — built with Rust and GPU rendering.

## Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Cmd/Ctrl + N` | New note |
| `Cmd/Ctrl + S` | Save note |
| `Cmd/Ctrl + I` | Inline AI assist |
| `Cmd/Ctrl + ,` | Settings |
| `/` | Block menu (Notion-style) |

## Getting started

### Download

Grab a prebuilt binary for macOS, Windows or Linux from the
[Releases](https://github.com/zetahiveco/kbwrite/releases) page.

### Build from source

```bash
cargo build --release
./target/release/kbwrite
```

### Configure

- **Vault folder** — KbWrite creates a default vault at `~/Documents/KbWriteVault`.
  Change it in **Settings** (`Cmd/Ctrl+,`).
- **Inline AI** — in Settings, paste your OpenAI-compatible **base URL**, **model** and
  **API key**. For a local Ollama setup, use e.g. `http://localhost:11434/v1`
  with model `llama3.1`.

Settings are stored in `~/.kbwrite/settings.json`.

## Screenshots

_Coming soon_

## Tech stack

- [Rust](https://www.rust-lang.org/) + [eframe/egui](https://github.com/emilk/egui) for the UI
- [egui_phosphor_icons](https://docs.rs/egui_phosphor_icons/latest/egui_phosphor_icons/) for icons
- [ureq](https://docs.rs/ureq) for OpenAI-compatible HTTP calls
- Plain markdown files for storage

## License

MIT © Harish Deivanayagam

---

Made by [harishdeivanayagam](https://github.com/harishdeivanayagam) · [zetahive.co](https://zetahive.co)

