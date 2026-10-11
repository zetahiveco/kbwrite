# Welcome to KbWrite

KbWrite is a local-first, Obsidian-style knowledge base written in native Rust with egui.

## Quick start

- Your notes are plain markdown files, stored in a **vault** folder on disk.
- Create new notes with the **+** button in the sidebar or `Cmd/Ctrl+N`.
- Save with `Cmd/Ctrl+S` — KbWrite also autosaves when you switch notes or exit.
- Change the vault folder in **Settings** (`Cmd/Ctrl+,`).

## Blocks and the "/" command

Press `/` on an empty line to open the block menu:

- # Heading 1 / ## Heading 2 / ### Heading 3
- Bullet list
- Numbered list
- To-do list (click the checkbox to toggle)
- Quote
- Code block
- Divider

Press `Enter` inside a list to continue it; press `Enter` again on an empty list item to exit.

## Inline AI text

1. Open **Settings** and paste your OpenAI-compatible **base URL**, **model** and **API key**
   (works with OpenAI, Ollama, LM Studio, OpenRouter, …).
2. Click **AI Assist** in the toolbar or press `Cmd/Ctrl+I`.
3. Type an instruction like *"Fix grammar"*, *"Summarize"* or *"Turn into bullets"*
   and the result is inserted right below your focused block.
