//! Main application: sidebar, note list, editor, settings and AI windows.

use crate::ai;
use crate::editor::{Editor, LinkCandidate};
use crate::graph::GraphExplorer;
use crate::settings::Settings;
use crate::vault::{NoteMeta, TreeNode, Vault};
use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Key, Layout, RichText, ScrollArea, Sense, TextEdit,
    Ui, Vec2, Window,
};
use egui_phosphor_icons::Icon;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

/// Kind of inline (VS Code-style) rename/create row in the sidebar.
#[derive(Debug, Clone, PartialEq)]
enum EditKind {
    NewFile,
    NewFolder,
    Rename,
}

/// Inline (VS Code-style) rename/create row shown inside the file tree.
#[derive(Debug, Clone)]
struct InlineEdit {
    kind: EditKind,
    /// Directory a new file/folder is created in.
    dir: PathBuf,
    /// Existing path being renamed (for `EditKind::Rename`).
    target: Option<PathBuf>,
    is_dir: bool,
    name: String,
}

impl InlineEdit {
    fn new_file(dir: &Path) -> Self {
        Self {
            kind: EditKind::NewFile,
            dir: dir.to_path_buf(),
            target: None,
            is_dir: false,
            name: String::new(),
        }
    }
    fn new_folder(dir: &Path) -> Self {
        Self {
            kind: EditKind::NewFolder,
            dir: dir.to_path_buf(),
            target: None,
            is_dir: true,
            name: String::new(),
        }
    }
    fn rename(path: &Path, is_dir: bool) -> Self {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        Self {
            kind: EditKind::Rename,
            dir: path.parent().unwrap_or(path).to_path_buf(),
            target: Some(path.to_path_buf()),
            is_dir,
            name,
        }
    }
}

const PHOSPHOR: &str = "phosphor-regular";

/// Font definitions: Phosphor icons + the platform's default UI and code fonts.
pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor_icons::add_fonts(&mut fonts);

    // Default system UI font (SF Pro on macOS, Segoe UI on Windows, DejaVu/Noto on Linux).
    if let Some(data) = system_font(&[
        "/System/Library/Fonts/SFNS.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ]) {
        fonts
            .font_data
            .insert("system-ui".to_owned(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "system-ui".to_owned());
    }

    // Default system code/monospace font (Menlo on macOS, Consolas on Windows, DejaVu Sans Mono on Linux).
    if let Some(data) = system_font(&[
        "/System/Library/Fonts/Menlo.ttc",
        "C:\\Windows\\Fonts\\consola.ttf",
        "C:\\Windows\\Fonts\\cascadiamono.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    ]) {
        fonts
            .font_data
            .insert("system-mono".to_owned(), egui::FontData::from_owned(data).into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "system-mono".to_owned());
    }

    fonts
}

fn system_font(paths: &[&str]) -> Option<Vec<u8>> {
    let path = paths.iter().find(|p| std::path::Path::new(p).is_file())?;
    std::fs::read(path).ok()
}

/// Standalone icon rendered with the Phosphor font family.
fn icon(name: &str) -> RichText {
    Icon::from_name(name)
        .map(|i| i.regular())
        .unwrap_or_else(|| RichText::new("\u{2022}"))
}

/// Raw glyph string (for LayoutJob composition).
fn glyph(name: &str) -> String {
    Icon::from_name(name)
        .map(|i| i.0.to_string())
        .unwrap_or_else(|| "\u{2022}".to_string())
}

/// Icon + text in one label, each with the right font family.
fn icon_job(name: &str, text: &str, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &glyph(name),
        4.0,
        egui::TextFormat::simple(FontId::new(size, FontFamily::Name(PHOSPHOR.into())), color),
    );
    job.append(
        text,
        0.0,
        egui::TextFormat::simple(FontId::new(size, FontFamily::Proportional), color),
    );
    job
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ViewMode {
    Editor,
    Raw,
    Graph,
}

pub struct KbWriteApp {
    settings: Settings,
    vault: Option<Vault>,
    vault_error: Option<String>,
    notes: Vec<NoteMeta>,
    /// File tree (folders + notes) mirroring the vault on disk.
    notes_tree: Vec<TreeNode>,
    /// Currently expanded folders.
    open_dirs: HashSet<PathBuf>,
    /// Active inline rename/create row (VS Code-style).
    editing: Option<InlineEdit>,
    edit_focus_requested: bool,
    current_note: Option<NoteMeta>,
    editor: Editor,
    dirty: bool,

    view: ViewMode,
    raw_text: String,

    /// Obsidian-style graph explorer state.
    graph: GraphExplorer,
    /// Bumped whenever notes change on disk, so the graph can rebuild.
    notes_version: u64,

    // Sidebar
    search: String,
    new_note_title: String,
    creating_new_note: bool,

    // Windows
    show_settings: bool,

    // AI
    ai_rx: Option<Receiver<Result<String, String>>>,
    ai: AiState,

    active_block_for_ai: Option<u64>,
    toasts: Vec<(String, std::time::Instant)>,
    pending_open: Option<PathBuf>,
    /// Path awaiting deletion confirmation (dialog shown).
    confirming_delete: Option<PathBuf>,
    pending_retry_vault: bool,
}

#[derive(Default)]
struct AiState {
    open: bool,
    instruction: String,
    result: String,
    busy: bool,
    error: Option<String>,
}

impl KbWriteApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Box<Self> {
        let settings = Settings::load();
        apply_theme(&cc.egui_ctx, settings.dark_mode);

        let (vault, vault_error) = match Vault::open(&settings.vault_path) {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(format!("Vault error: {e:#}"))),
        };
        let mut app = Box::new(Self {
            vault,
            vault_error,
            settings,
            notes: Vec::new(),
            notes_tree: Vec::new(),
            open_dirs: HashSet::new(),
            editing: None,
            edit_focus_requested: false,
            current_note: None,
            editor: Editor::new(),
            dirty: false,
            view: ViewMode::Editor,
            raw_text: String::new(),
            graph: GraphExplorer::new(),
            notes_version: 0,
            search: String::new(),
            new_note_title: String::new(),
            creating_new_note: false,
            show_settings: false,
            ai: AiState::default(),
            ai_rx: None,
            active_block_for_ai: None,
            toasts: Vec::new(),
            pending_open: None,
            confirming_delete: None,
            pending_retry_vault: false,
        });
        app.refresh_notes();
        app.expand_all_dirs();

        if app.notes.is_empty() {
            if let Err(e) = app.ensure_default_note() {
                app.vault_error = Some(format!("Could not create the Welcome note: {e:#}"));
            }
            app.refresh_notes();
        }
        app.open_note(0);
        app
    }

    fn ensure_default_note(&mut self) -> anyhow::Result<()> {
        let welcome = include_str!("welcome.md");
        if let Some(vault) = &self.vault {
            vault.create_note("Welcome", welcome)?;
        }
        Ok(())
    }

    fn refresh_notes(&mut self) {
        if let Some(vault) = &mut self.vault {
            self.notes = vault.list_notes();
            self.notes_tree = vault.tree();
        }
        self.notes_version += 1;
    }

    /// Expand every folder (used on first load so all notes are visible).
    fn expand_all_dirs(&mut self) {
        fn collect(nodes: &[TreeNode], set: &mut HashSet<PathBuf>) {
            for n in nodes {
                if n.is_dir {
                    set.insert(n.path.clone());
                    collect(&n.children, set);
                }
            }
        }
        collect(&self.notes_tree, &mut self.open_dirs);
    }

    /// Open a note by its path (no-op if it is no longer in the vault).
    fn open_path(&mut self, path: &Path) {
        if let Some(i) = self.notes.iter().position(|n| n.path == path) {
            self.open_note(i);
        } else {
            self.current_note = None;
        }
    }

    /// Start an inline create/rename row in the sidebar.
    fn start_edit(&mut self, edit: InlineEdit) {
        if edit.kind != EditKind::Rename {
            self.open_dirs.insert(edit.dir.clone());
        }
        self.editing = Some(edit);
        self.edit_focus_requested = true;
    }

    /// Apply an inline create/rename, keeping the row open on failure
    /// (like VS Code) so the user can pick a different name.
    fn confirm_edit(&mut self, edit: InlineEdit) {
        let name = edit.name.trim().trim_matches('/').trim_end_matches('/');
        let name = name.rsplit('/').next().unwrap_or(name).trim();
        if name.is_empty() {
            return;
        }
        let Some(vault) = self.vault.as_ref() else {
            return;
        };
        let result = match edit.kind {
            EditKind::NewFile => {
                let mut name = name.to_string();
                if !name.ends_with(".md") {
                    name.push_str(".md");
                }
                let path = edit.dir.join(&name);
                vault.create_file(&path, "").map(|_| path)
            }
            EditKind::NewFolder => {
                let path = edit.dir.join(name);
                vault.create_folder(&path).map(|_| path)
            }
            EditKind::Rename => {
                let Some(target) = edit.target.clone() else {
                    return;
                };
                let mut name = name.to_string();
                if !edit.is_dir && !name.ends_with(".md") {
                    name.push_str(".md");
                }
                let to = target.parent().unwrap_or(&edit.dir).join(&name);
                vault.rename_path(&target, &to).map(|_| to)
            }
        };
        match result {
            Ok(path) => {
                // Keep the renamed note open / open freshly created files.
                if edit.kind == EditKind::Rename {
                    if let Some(target) = &edit.target {
                        if self
                            .current_note
                            .as_ref()
                            .map(|n| &n.path == target)
                            .unwrap_or(false)
                        {
                            if let Some(cn) = &mut self.current_note {
                                cn.path = path.clone();
                                cn.rel = path
                                    .strip_prefix(&self.settings.vault_path)
                                    .unwrap_or(&path)
                                    .to_string_lossy()
                                    .to_string();
                                cn.title = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                            }
                        }
                    }
                }
                self.refresh_notes();
                match edit.kind {
                    EditKind::NewFile => self.open_path(&path),
                    EditKind::NewFolder => {
                        self.open_dirs.insert(path.clone());
                    }
                    EditKind::Rename => {}
                }
            }
            Err(e) => {
                self.toast(format!("{e:#}"));
                self.editing = Some(edit);
                self.edit_focus_requested = true;
            }
        }
    }

    fn open_note(&mut self, idx: usize) {
        self.sync_raw_to_editor();
        if let Some(note) = self.notes.get(idx).cloned() {
            if let Some(vault) = &self.vault {
                if let Ok(content) = vault.read_note(&note.path) {
                    self.editor.load_markdown(&content);
                } else {
                    self.editor.load_markdown("");
                }
            }
            self.current_note = Some(note);
            self.dirty = false;
            self.view = ViewMode::Editor;
            self.raw_text.clear();
        }
    }

    /// If we are in Raw mode, push the raw markdown into the block editor.
    fn sync_raw_to_editor(&mut self) {
        if self.view == ViewMode::Raw && !self.raw_text.is_empty() {
            let raw = std::mem::take(&mut self.raw_text);
            if raw != self.editor.to_markdown() {
                self.editor.load_markdown(&raw);
                self.dirty = true;
            }
        }
    }

    fn save_current(&mut self) {
        self.sync_raw_to_editor();
        if let (Some(vault), Some(note)) = (&self.vault, &self.current_note.clone()) {
            let content = self.editor.to_markdown();
            if vault.write_note(&note.path, &content).is_ok() {
                self.dirty = false;
                // Links may have changed → let the graph rebuild.
                self.notes_version += 1;
            }
        }
    }

    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.settings.dark_mode = !self.settings.dark_mode;
        apply_theme(ctx, self.settings.dark_mode);
        let _ = self.settings.save();
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((
            msg.into(),
            std::time::Instant::now() + std::time::Duration::from_secs(4),
        ));
    }

    fn new_note(&mut self, title: &str) {
        match self.vault.as_ref().map(|v| v.create_note(title, "")) {
            Some(Ok(note)) => {
                self.refresh_notes();
                if let Some(i) = self.notes.iter().position(|n| n.path == note.path) {
                    self.open_note(i);
                }
                self.editor.load_markdown("");
                self.dirty = false;
            }
            Some(Err(e)) => self.toast(format!("Failed to create note: {e}")),
            None => self.toast("No vault open"),
        }
    }

    fn start_ai(&mut self) {
        self.sync_raw_to_editor();
        self.ai.open = true;
        self.ai.error = None;
        self.ai.result.clear();
        self.active_block_for_ai = self.editor.last_focused;
    }

    fn run_ai(&mut self) {
        let settings = self.settings.clone();
        let instruction = self.ai.instruction.trim().to_string();
        if instruction.is_empty() {
            return;
        }
        let target = self
            .active_block_for_ai
            .and_then(|id| self.editor.blocks.iter().find(|b| b.id == id))
            .map(|b| b.text.clone())
            .unwrap_or_default();
        let target = if target.trim().is_empty() {
            self.editor
                .blocks
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            target
        };
        if target.trim().is_empty() {
            self.ai.error = Some("There is no text to work on. Write something first.".into());
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.ai_rx = Some(rx);
        self.ai.busy = true;
        self.ai.error = None;
        self.ai.result.clear();

        let instruction = instruction.to_string();
        std::thread::spawn(move || {
            let result =
                ai::complete(&settings, &instruction, &target).map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
        });
    }
}

impl eframe::App for KbWriteApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Save when the user closes the window.
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_current();
            let _ = self.settings.save();
        }

        self.handle_shortcuts(&ctx);
        self.poll_ai();
        self.draw_sidebar(ui);
        self.draw_main(ui);
        self.draw_settings_window(&ctx);
        self.draw_ai_window(&ctx);
        self.draw_toasts(&ctx);

        if self.dirty {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn on_exit(&mut self) {
        self.save_current();
        let _ = self.settings.save();
    }
}

impl KbWriteApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |i: &egui::InputState| {
            if cfg!(target_os = "macos") {
                i.modifiers.command
            } else {
                i.modifiers.ctrl
            }
        };

        if ctx.input(|i| cmd(i) && i.key_pressed(Key::S)) {
            self.save_current();
            self.toast("Note saved");
        }

        if ctx.input(|i| cmd(i) && i.key_pressed(Key::N)) {
            self.creating_new_note = true;
        }

        if ctx.input(|i| cmd(i) && i.key_pressed(Key::I)) && self.current_note.is_some() {
            self.start_ai();
        }

        if ctx.input(|i| cmd(i) && i.key_pressed(Key::Comma)) {
            self.show_settings = !self.show_settings;
        }

        // Cmd/Ctrl+Backspace (always) or Delete (when not editing text) moves
        // the current note to the confirmation dialog.
        if let Some(path) = self.current_note.as_ref().map(|n| n.path.clone()) {
            let cmd_backspace = ctx.input(|i| cmd(i) && i.key_pressed(Key::Backspace));
            let no_text_focus = ctx.memory(|m| m.focused().is_none());
            let delete_key = no_text_focus && ctx.input(|i| i.key_pressed(Key::Delete));
            if cmd_backspace || delete_key {
                self.confirming_delete = Some(path);
            }
        }
    }

    fn poll_ai(&mut self) {
        if let Some(rx) = &self.ai_rx {
            match rx.try_recv() {
                Ok(Ok(result)) => {
                    self.sync_raw_to_editor();
                    self.ai.busy = false;
                    self.ai.result = result.clone();
                    self.editor.insert_ai_result(&result);
                    self.dirty = true;
                    self.ai_rx = None;
                }
                Ok(Err(e)) => {
                    self.ai.busy = false;
                    self.ai.error = Some(e);
                    self.ai_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ai_rx = None;
                }
            }
        }
    }

    fn draw_sidebar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(260.0)
            .min_size(180.0)
            .frame(
                egui::Frame::NONE
                    .fill(if self.settings.dark_mode {
                        Color32::from_gray(24)
                    } else {
                        Color32::from_gray(242)
                    }),
            )
            .show(ui, |ui| {
                // Bottom-up layout: the footer (vault path + errors) is placed
                // first so it stays pinned at the bottom, and the note list
                // gets all the remaining space above it. A plain top-down
                // layout lets the full-height ScrollArea push the footer
                // off-screen, hiding all diagnostics.
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    // ---------- Footer (pinned at the bottom) ----------
                    ui.add_space(4.0);
                    if let Some(err) = &self.vault_error {
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.label(
                                icon("warning")
                                    .size(12.0)
                                    .color(Color32::from_rgb(255, 140, 120)),
                            );
                            ui.label(
                                RichText::new(err)
                                    .size(11.5)
                                    .color(Color32::from_rgb(255, 140, 120)),
                            );
                        });
                    }
                    if let Some(vault) = &self.vault {
                        for err in vault.walk_errors.iter().take(1) {
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                ui.label(
                                    icon("warning")
                                        .size(12.0)
                                        .color(Color32::from_rgb(255, 140, 120)),
                                );
                                ui.label(
                                    RichText::new(err.clone())
                                        .size(11.0)
                                        .color(Color32::from_rgb(255, 140, 120)),
                                );
                            });
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            icon("folder-open")
                                .size(11.0)
                                .color(muted(self.settings.dark_mode)),
                        );
                        ui.label(
                            RichText::new(vault_display(&self.settings.vault_path))
                                .size(11.0)
                                .color(muted(self.settings.dark_mode)),
                        );
                    });
                    ui.add_space(2.0);
                    ui.separator();

                    // ---------- Scrollable middle: title, search, notes ----------
                    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                        ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                            ui.add_space(8.0);
                            // App title + actions
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                ui.label(
                                    icon("notebook")
                                        .size(18.0)
                                        .color(accent(self.settings.dark_mode)),
                                );
                                ui.label(RichText::new("KbWrite").size(17.0).strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add_space(6.0); // keep inside: trailing space AFTER this layout overflows
                                    if icon_button(ui, "gear-six", "Settings (Cmd/Ctrl+,)") {
                                        self.show_settings = !self.show_settings;
                                    }
                                    if icon_button(ui, "folder-plus", "New folder") {
                                        let dir = self.settings.vault_path.clone();
                                        self.start_edit(InlineEdit::new_folder(&dir));
                                    }
                                    if icon_button(ui, "file-plus", "New file (Cmd/Ctrl+N)") {
                                        let dir = self.settings.vault_path.clone();
                                        self.start_edit(InlineEdit::new_file(&dir));
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            // Search box
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                ui.label(
                                    icon("magnifying-glass")
                                        .size(13.0)
                                        .color(muted(self.settings.dark_mode)),
                                );
                                let search = TextEdit::singleline(&mut self.search)
                                    .hint_text("Search notes…")
                                    .desired_width(ui.available_width() - 12.0)
                                    .frame(egui::Frame::NONE);
                                ui.add(search);
                                ui.add_space(4.0);
                            });
                            ui.separator();
                            ui.add_space(2.0);

                            // Note list / file tree + empty states
                            let query = self.search.trim().to_lowercase();
                            let mut rows_rendered = 0usize;

                            if query.is_empty() {
                                // ---- File tree (VS Code-style) ----
                                let tree = self.notes_tree.clone();
                                self.draw_tree_nodes(ui, &self.settings.vault_path.clone(), &tree, 0);
                                rows_rendered = tree.len();
                            } else {
                                // ---- Flat search results ----
                                let notes = self.notes.clone();
                                for note in notes
                                    .iter()
                                    .filter(|n| n.rel.to_lowercase().contains(&query))
                                {
                                    rows_rendered += 1;
                                    let is_current = self
                                        .current_note
                                        .as_ref()
                                        .map(|c| c.path == note.path)
                                        .unwrap_or(false);
                                    let text_color = if is_current {
                                        accent(self.settings.dark_mode)
                                    } else {
                                        normal_text(self.settings.dark_mode)
                                    };
                                    let job = icon_job("file-text", &note.rel, 13.5, text_color);
                                    let row = ui.selectable_label(is_current, job);
                                    if row.clicked() {
                                        self.pending_open = Some(note.path.clone());
                                    }
                                }
                            }

                            if rows_rendered == 0 {
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);
                                    ui.label(
                                        icon("info")
                                            .size(13.0)
                                            .color(muted(self.settings.dark_mode)),
                                    );
                                });
                                if let Some(err) = &self.vault_error {
                                    ui.label(
                                        RichText::new(err.clone())
                                            .size(12.0)
                                            .color(Color32::from_rgb(255, 140, 120)),
                                    );
                                } else if !self
                                    .vault
                                    .as_ref()
                                    .map(|v| v.walk_errors.is_empty())
                                    .unwrap_or(false)
                                {
                                    // The vault exists but could not be read — most
                                    // commonly macOS privacy protection (TCC).
                                    let perm_denied = self
                                        .vault
                                        .as_ref()
                                        .map(|v| v.permission_denied)
                                        .unwrap_or(false);
                                    if perm_denied {
                                        ui.label(
                                            RichText::new("Access to the notes folder is blocked by macOS")
                                                .size(12.5)
                                                .strong()
                                                .color(Color32::from_rgb(255, 140, 120)),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "Allow the app you launched KbWrite from \
                                                 (Terminal, iTerm, …) to access the Documents \
                                                 folder:\n{}",
                                                self.settings.vault_path.display()
                                            ))
                                            .size(11.5)
                                            .color(muted(self.settings.dark_mode)),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            if ui.button("Open System Settings…").clicked() {
                                                let _ = std::process::Command::new("open")
                                                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_DocumentsFolder")
                                                    .spawn();
                                            }
                                            if ui.button("Try again").clicked() {
                                                self.pending_retry_vault = true;
                                            }
                                        });
                                    } else {
                                        for err in self
                                            .vault
                                            .as_ref()
                                            .map(|v| v.walk_errors.iter().take(1))
                                            .into_iter()
                                            .flatten()
                                        {
                                            ui.label(
                                                RichText::new(err.clone())
                                                    .size(12.0)
                                                    .color(Color32::from_rgb(255, 140, 120)),
                                            );
                                        }
                                        if ui.button("Try again").clicked() {
                                            self.pending_retry_vault = true;
                                        }
                                    }
                                } else if !query.is_empty() {
                                    ui.label(
                                        RichText::new(format!("No notes match “{query}”."))
                                            .size(12.0)
                                            .color(muted(self.settings.dark_mode)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("No notes yet — use New file (⌘N) or right-click here.")
                                            .size(12.0)
                                            .color(muted(self.settings.dark_mode)),
                                    );
                                }
                            }

                            if query.is_empty() && rows_rendered > 0 {
                                // Right-click on empty space: New File / New Folder.
                                let space_rect = ui.available_rect_before_wrap();
                                let (_, space_resp) = ui.allocate_exact_size(
                                    Vec2::new(space_rect.width(), 140.0),
                                    Sense::click(),
                                );
                                space_resp.context_menu(|ui| {
                                    if ui.button("New File…").clicked() {
                                        let dir = self.settings.vault_path.clone();
                                        self.start_edit(InlineEdit::new_file(&dir));
                                    }
                                    if ui.button("New Folder…").clicked() {
                                        let dir = self.settings.vault_path.clone();
                                        self.start_edit(InlineEdit::new_folder(&dir));
                                    }
                                });
                            }
                        });
                    });
                });
            });

        // Deferred actions (avoid mutating while borrowing notes).
        if let Some(path) = self.pending_open.take() {
            self.open_path(&path);
        }
        if self.pending_retry_vault {
            self.pending_retry_vault = false;
            self.save_current();
            match crate::vault::Vault::open(&self.settings.vault_path) {
                Ok(v) => {
                    self.vault = Some(v);
                    self.vault_error = None;
                    self.refresh_notes();
                    self.open_note(0);
                }
                Err(e) => self.vault_error = Some(format!("Vault error: {e:#}")),
            }
        }
        // Delete confirmation dialog (files + folders).
        if let Some(path) = self.confirming_delete.clone() {
            let is_dir = path.is_dir();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            let mut do_delete = false;
            let mut cancel = false;
            let mut open = true;
            Window::new("Confirm delete")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label(RichText::new(format!(
                        "Delete “{name}”{}?",
                        if is_dir { " and everything inside it" } else { "" }
                    ))
                    .size(14.0));
                    ui.label(
                        RichText::new("This cannot be undone.")
                            .size(12.0)
                            .color(muted(self.settings.dark_mode)),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Delete").color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(200, 60, 60)),
                            )
                            .clicked()
                        {
                            do_delete = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            cancel = true;
                        }
                    });
                });
            if do_delete {
                self.confirming_delete = None;
                let was_current = self
                    .current_note
                    .as_ref()
                    .map(|c| c.path == path || c.path.starts_with(&path))
                    .unwrap_or(false);
                match self.vault.as_ref().map(|v| v.delete_path(&path)) {
                    Some(Ok(())) => {
                        self.toast("Deleted");
                        self.refresh_notes();
                        if was_current {
                            if self.notes.is_empty() {
                                let _ = self.ensure_default_note();
                                self.refresh_notes();
                            }
                            self.open_note(0);
                        }
                    }
                    Some(Err(e)) => self.toast(format!("Delete failed: {e:#}")),
                    None => self.toast("No vault open"),
                }
            } else if cancel || !open {
                self.confirming_delete = None;
            }
        }

        // New note prompt
        if self.creating_new_note {
            let mut open = true;
            let mut create = false;
            let mut cancel = false;
            Window::new(icon_job("plus", "New note", 15.0, normal_text(self.settings.dark_mode)))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.add_space(4.0);
                    let title = TextEdit::singleline(&mut self.new_note_title)
                        .hint_text("Note title…")
                        .desired_width(260.0);
                    ui.add(title);
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        create = true;
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            create = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                            self.new_note_title.clear();
                        }
                    });
                });
            if create {
                let title = std::mem::take(&mut self.new_note_title);
                if !title.trim().is_empty() {
                    self.new_note(title.trim());
                }
                open = false;
            }
            if cancel {
                open = false;
            }
            self.creating_new_note = open;
        }
    }

    /// Render the vault file tree (VS Code-style), including any active
    /// inline create/rename row.
    fn draw_tree_nodes(&mut self, ui: &mut Ui, dir: &Path, nodes: &[TreeNode], depth: usize) {
        // Inline create row for THIS folder (rendered as its first child).
        let create_here = self
            .editing
            .as_ref()
            .map(|e| e.kind != EditKind::Rename && e.dir == dir)
            .unwrap_or(false);
        if create_here {
            self.draw_edit_row(ui, depth);
        }

        for node in nodes {
            // A renamed row is replaced by the inline edit field.
            let rename_here = self
                .editing
                .as_ref()
                .map(|e| {
                    e.kind == EditKind::Rename
                        && e.target.as_deref() == Some(node.path.as_path())
                })
                .unwrap_or(false);
            if rename_here {
                self.draw_edit_row(ui, depth);
                continue;
            }

            let indent = 10.0 + depth as f32 * 14.0;
            let dark = self.settings.dark_mode;
            let is_current = self
                .current_note
                .as_ref()
                .map(|c| c.path == node.path)
                .unwrap_or(false);
            let text_color = if is_current {
                accent(dark)
            } else {
                normal_text(dark)
            };

            ui.horizontal(|ui| {
                ui.add_space(indent);
                if node.is_dir {
                    let open = self.open_dirs.contains(&node.path);
                    let caret = if open { "caret-down" } else { "caret-right" };
                    let job = icon_job(caret, &node.name, 13.5, text_color);
                    let row = ui.selectable_label(false, job);
                    if row.clicked() {
                        if open {
                            self.open_dirs.remove(&node.path);
                        } else {
                            self.open_dirs.insert(node.path.clone());
                        }
                    }
                    row.context_menu(|ui| {
                        if ui.button("New File…").clicked() {
                            self.start_edit(InlineEdit::new_file(&node.path));
                        }
                        if ui.button("New Folder…").clicked() {
                            self.start_edit(InlineEdit::new_folder(&node.path));
                        }
                        ui.separator();
                        if ui.button("Rename").clicked() {
                            self.start_edit(InlineEdit::rename(&node.path, true));
                        }
                        if ui.button("Delete").clicked() {
                            self.confirming_delete = Some(node.path.clone());
                        }
                    });
                } else {
                    let job = icon_job("file-text", &node.name, 13.5, text_color);
                    let row = ui.selectable_label(is_current, job);
                    if row.clicked() {
                        self.pending_open = Some(node.path.clone());
                    }
                    row.context_menu(|ui| {
                        if ui.button("Rename").clicked() {
                            self.start_edit(InlineEdit::rename(&node.path, false));
                        }
                        if ui.button("Delete").clicked() {
                            self.confirming_delete = Some(node.path.clone());
                        }
                    });
                }
            });

            if node.is_dir && self.open_dirs.contains(&node.path) {
                self.draw_tree_nodes(ui, &node.path, &node.children, depth + 1);
            }
        }
    }

    /// The inline text field shown in place of a row while renaming/creating.
    fn draw_edit_row(&mut self, ui: &mut Ui, depth: usize) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let indent = 10.0 + depth as f32 * 14.0 + 14.0;
        let mut confirm = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let resp = TextEdit::singleline(&mut edit.name)
                .id(egui::Id::new("sidebar_inline_edit"))
                .hint_text(if edit.is_dir { "Folder name…" } else { "File name…" })
                .desired_width(ui.available_width() - 20.0)
                .font(egui::TextStyle::Body)
                .show(ui);
            if self.edit_focus_requested {
                resp.response.request_focus();
                self.edit_focus_requested = false;
            }
            if resp.response.has_focus() {
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    confirm = true;
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            }
            // Commit when clicking elsewhere, like VS Code.
            if resp.response.lost_focus() && !confirm && !cancel {
                confirm = true;
            }
        });
        if let Some(edit) = self.editing.clone() {
            if cancel {
                self.editing = None;
            } else if confirm {
                self.editing = None;
                self.confirm_edit(edit);
            }
        }
    }

    fn draw_main(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style()).fill(if self.settings.dark_mode {
                    Color32::from_gray(18)
                } else {
                    Color32::WHITE
                }),
            )
            .show(ui, |ui| {
                // Toolbar
                ui.horizontal(|ui| {
                    let title = self
                        .current_note
                        .as_ref()
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| "No note".to_string());
                    ui.label(
                        icon("file-text")
                            .size(16.0)
                            .color(accent(self.settings.dark_mode)),
                    );
                    ui.label(RichText::new(&title).size(16.0).strong());
                    if self.dirty {
                        ui.label(
                            RichText::new("• unsaved")
                                .size(12.0)
                                .color(muted(self.settings.dark_mode)),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, "floppy-disk", "Save (Cmd/Ctrl+S)") {
                            self.save_current();
                            self.toast("Note saved");
                        }
                        if icon_button(ui, "trash", "Delete this note (Cmd/Ctrl+⌫)") {
                            if let Some(path) = self.current_note.as_ref().map(|n| n.path.clone()) {
                                self.confirming_delete = Some(path);
                            }
                        }
                        if ui
                            .add(egui::Button::new(icon_job(
                                "sparkle",
                                "AI Assist",
                                13.0,
                                normal_text(self.settings.dark_mode),
                            )))
                            .on_hover_text("Inline AI text (Cmd/Ctrl+I)")
                            .clicked()
                        {
                            self.start_ai();
                        }
                        // Light/dark toggle: shows moon in light mode, sun in dark mode.
                        if icon_button(
                            ui,
                            if self.settings.dark_mode { "sun" } else { "moon" },
                            if self.settings.dark_mode {
                                "Switch to light mode"
                            } else {
                                "Switch to dark mode"
                            },
                        ) {
                            self.toggle_theme(&ctx);
                        }
                        // Single Editor/Raw toggle button.
                        let (mode_icon, mode_label, to_raw) = if self.view == ViewMode::Editor {
                            ("markdown-logo", "Raw", true)
                        } else {
                            ("pencil-simple", "Editor", false)
                        };
                        if ui
                            .add(egui::Button::new(icon_job(
                                mode_icon,
                                mode_label,
                                13.0,
                                if to_raw {
                                    normal_text(self.settings.dark_mode)
                                } else {
                                    accent(self.settings.dark_mode)
                                },
                            )))
                            .on_hover_text(if to_raw {
                                "Switch to raw markdown editing"
                            } else {
                                "Switch to the block editor"
                            })
                            .clicked()
                        {
                            if to_raw {
                                self.raw_text = self.editor.to_markdown();
                                self.view = ViewMode::Raw;
                            } else {
                                self.sync_raw_to_editor();
                                self.raw_text.clear();
                                self.view = ViewMode::Editor;
                            }
                        }

                        // Graph view toggle (Obsidian-style graph explorer).
                        if ui
                            .add(egui::Button::new(icon_job(
                                "graph",
                                "Graph",
                                13.0,
                                if self.view == ViewMode::Graph {
                                    accent(self.settings.dark_mode)
                                } else {
                                    normal_text(self.settings.dark_mode)
                                },
                            )))
                            .on_hover_text("Graph view — see how notes link together")
                            .clicked()
                        {
                            self.sync_raw_to_editor();
                            self.raw_text.clear();
                            self.view = if self.view == ViewMode::Graph {
                                ViewMode::Editor
                            } else {
                                ViewMode::Graph
                            };
                        }

                        ui.label(
                            RichText::new("Type  /  for blocks")
                                .size(11.5)
                                .color(muted(self.settings.dark_mode)),
                        );
                    });
                });
                ui.separator();

                if self.current_note.is_none() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            icon("note-blank")
                                .size(48.0)
                                .color(muted(self.settings.dark_mode)),
                        );
                        ui.label("Select or create a note to start writing.");
                        if ui.button("New note").clicked() {
                            self.creating_new_note = true;
                        }
                    });
                    return;
                }

                match self.view {
                    ViewMode::Editor => {
                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            ui.add_space(8.0);
                            let mut modified = false;
                            let link_notes: Vec<LinkCandidate> = self
                                .notes
                                .iter()
                                .map(|n| LinkCandidate {
                                    title: n.title.clone(),
                                    // Show/insert names without the ".md" suffix.
                                    rel: n.rel.trim_end_matches(".md").to_string(),
                                })
                                .collect();
                            self.editor.show(ui, &mut modified, &link_notes);
                            if modified {
                                self.dirty = true;
                            }
                            ui.add_space(120.0);
                        });
                    }
                    ViewMode::Raw => {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Raw markdown — changes apply when you switch back to Editor")
                                .size(11.0)
                                .color(muted(self.settings.dark_mode)),
                        );
                        let mut modified_raw = false;
                        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                            let out = TextEdit::multiline(&mut self.raw_text)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(ui.available_width())
                                .frame(egui::Frame::NONE)
                                .code_editor()
                                .show(ui);
                            if out.response.changed() {
                                modified_raw = true;
                            }
                        });
                        if modified_raw {
                            self.dirty = true;
                        }
                    }
                    ViewMode::Graph => {
                        let (nodes, links) = self.graph.stats();
                        ui.label(
                            RichText::new(format!(
                                "Graph view — {nodes} notes, {links} links · click a node to open it · drag to pan · scroll to zoom"
                            ))
                            .size(11.0)
                            .color(muted(self.settings.dark_mode)),
                        );
                        if let Some(vault) = self.vault.as_ref() {
                            let current = self.current_note.as_ref().map(|n| n.path.clone());
                            if let Some(path) = self.graph.show(
                                ui,
                                vault,
                                &self.notes,
                                self.notes_version,
                                self.settings.dark_mode,
                                current.as_deref(),
                            ) {
                                self.pending_open = Some(path);
                            }
                        }
                    }
                }
            });
    }

    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        Window::new(icon_job("gear-six", "Settings", 15.0, normal_text(self.settings.dark_mode)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);

                ui.heading(RichText::new("Appearance").size(14.0));
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let label = if self.settings.dark_mode { "Dark mode" } else { "Light mode" };
                    if ui
                        .add(egui::Button::new(icon_job(
                            if self.settings.dark_mode { "moon" } else { "sun" },
                            label,
                            13.0,
                            normal_text(self.settings.dark_mode),
                        )))
                        .clicked()
                    {
                        self.toggle_theme(ctx);
                    }
                });

                ui.add_space(10.0);
                ui.heading(RichText::new("Vault").size(14.0));
                ui.add_space(2.0);
                {
                    let mut vault = self.settings.vault_path.to_string_lossy().to_string();
                    let resp = ui.add(
                        TextEdit::singleline(&mut vault)
                            .hint_text("Path to your notes folder")
                            .desired_width(ui.available_width() - 110.0),
                    );
                    if resp.changed() {
                        self.settings.vault_path = vault.clone().into();
                    }
                    if ui.button("Reload vault").clicked() {
                        self.save_current();
                        if let Ok(v) = crate::vault::Vault::open(&self.settings.vault_path) {
                            self.vault = Some(v);
                            self.refresh_notes();
                            self.open_note(0);
                        }
                    }
                }

                ui.add_space(10.0);
                ui.heading(
                    RichText::new("Inline AI (OpenAI-compatible)").size(14.0),
                );
                ui.label(
                    RichText::new("Used by the AI Assist action. Works with OpenAI, Ollama, LM Studio, OpenRouter, or any compatible endpoint.")
                        .size(12.0)
                        .color(muted(self.settings.dark_mode)),
                );
                ui.add_space(4.0);
                egui::Grid::new("ai_settings")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Base URL");
                        ui.add(
                            TextEdit::singleline(&mut self.settings.openai_base_url)
                                .hint_text("https://api.openai.com/v1")
                                .desired_width(300.0),
                        );
                        ui.end_row();

                        ui.label("Model");
                        ui.add(
                            TextEdit::singleline(&mut self.settings.openai_model)
                                .hint_text("gpt-4o-mini")
                                .desired_width(300.0),
                        );
                        ui.end_row();

                        ui.label("API key");
                        ui.add(
                            TextEdit::singleline(&mut self.settings.openai_api_key)
                                .password(true)
                                .hint_text("sk-…")
                                .desired_width(300.0),
                        );
                        ui.end_row();
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save settings").clicked() {
                        self.save_current();
                        match crate::vault::Vault::open(&self.settings.vault_path) {
                            Ok(v) => {
                                self.vault = Some(v);
                                self.refresh_notes();
                            }
                            Err(e) => self.toast(format!("Vault error: {e:#}")),
                        }
                        if let Err(e) = self.settings.save() {
                            self.toast(format!("Failed to save settings: {e:#}"));
                        } else {
                            self.toast("Settings saved");
                        }
                        self.show_settings = false;
                    }
                });
            });
        self.show_settings = open;
    }

    fn draw_ai_window(&mut self, ctx: &egui::Context) {
        if !self.ai.open {
            return;
        }
        let mut open = self.ai.open;
        Window::new(icon_job("sparkle", "AI Assist", 15.0, normal_text(self.settings.dark_mode)))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Model: {}  ·  Target: {}",
                        self.settings.openai_model,
                        self.current_note
                            .as_ref()
                            .map(|n| n.title.clone())
                            .unwrap_or_default()
                    ))
                    .size(11.5)
                    .color(muted(self.settings.dark_mode)),
                );
                ui.add_space(4.0);
                let inst = TextEdit::singleline(&mut self.ai.instruction)
                    .hint_text("e.g. Fix grammar, Summarize, Make it a bullet list, Translate to French…")
                    .desired_width(ui.available_width());
                ui.add(inst);
                if ui.input(|i| i.key_pressed(Key::Enter)) && !self.ai.busy {
                    self.run_ai();
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if self.ai.busy {
                        ui.add(egui::Spinner::new().size(16.0));
                        ui.label("Thinking…");
                    } else if ui
                        .add(egui::Button::new(icon_job(
                            "play",
                            "Run",
                            13.0,
                            normal_text(self.settings.dark_mode),
                        )))
                        .clicked()
                    {
                        self.run_ai();
                    }
                    if !self.ai.result.is_empty() && ui.button("Copy result").clicked() {
                        let text = self.ai.result.clone();
                        ui.output_mut(|o| {
                            o.commands.push(egui::OutputCommand::CopyText(text))
                        });
                    }
                });
                if let Some(err) = &self.ai.error {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(err)
                            .color(Color32::from_rgb(255, 120, 120))
                            .size(12.5),
                    );
                }
                if !self.ai.result.is_empty() {
                    ui.add_space(4.0);
                    ui.separator();
                    ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        ui.label(&self.ai.result);
                    });
                    ui.label(
                        RichText::new("Result inserted below the focused block.")
                            .size(11.0)
                            .color(muted(self.settings.dark_mode)),
                    );
                }
            });
        self.ai.open = open;
    }

    fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = std::time::Instant::now();
        self.toasts.retain(|(_, until)| *until > now);
        let toasts = self.toasts.clone();
        for (i, (msg, _)) in toasts.iter().enumerate() {
            egui::Area::new(egui::Id::new(("toast", i)))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -(16.0 + 34.0 * i as f32)])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(if self.settings.dark_mode {
                            Color32::from_gray(40)
                        } else {
                            Color32::from_gray(250)
                        })
                        .corner_radius(8.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(msg.clone()).size(13.0));
                        });
                });
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(150, 190, 255)
    } else {
        Color32::from_rgb(40, 90, 200)
    }
}

fn normal_text(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(210)
    } else {
        Color32::from_gray(40)
    }
}

fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(130)
    } else {
        Color32::from_gray(130)
    }
}

fn vault_display(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    if s.len() > 40 {
        format!("…{}", &s[s.len() - 39..])
    } else {
        s.to_string()
    }
}

fn icon_button(ui: &mut Ui, name: &str, tooltip: &str) -> bool {
    let btn = egui::Button::new(icon(name).size(15.0)).small();
    ui.add(btn).on_hover_text(tooltip).clicked()
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    ctx.all_styles_mut(|style| {
        if dark {
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = Color32::from_gray(24);
            style.visuals.extreme_bg_color = Color32::from_gray(18);
            style.visuals.window_fill = Color32::from_gray(30);
        } else {
            style.visuals = egui::Visuals::light();
            style.visuals.panel_fill = Color32::from_gray(242);
            style.visuals.extreme_bg_color = Color32::from_gray(255);
            style.visuals.window_fill = Color32::from_gray(248);
        }
        style.visuals.widgets.inactive.bg_fill = if dark {
            Color32::from_gray(32)
        } else {
            Color32::from_gray(225)
        };
    });
}
