//! Main application: sidebar, note list, editor, settings and AI windows.

use crate::ai;
use crate::editor::Editor;
use crate::settings::Settings;
use crate::vault::{NoteMeta, Vault};
use eframe::egui::{
    self, Align, Color32, FontFamily, FontId, Key, Layout, RichText, ScrollArea, TextEdit, Ui,
    Window,
};
use egui_phosphor_icons::Icon;
use std::sync::mpsc::{self, Receiver};

const PHOSPHOR: &str = "phosphor-regular";

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
}

pub struct KbWriteApp {
    settings: Settings,
    vault: Option<Vault>,
    notes: Vec<NoteMeta>,
    current_note: Option<NoteMeta>,
    editor: Editor,
    dirty: bool,

    view: ViewMode,
    raw_text: String,

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
    pending_open: Option<usize>,
    pending_delete: Option<std::path::PathBuf>,
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

        // Register the Phosphor icon fonts (idempotent with main.rs).
        {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor_icons::add_fonts(&mut fonts);
            cc.egui_ctx.set_fonts(fonts);
        }

        let vault = Vault::open(&settings.vault_path).ok();
        let mut app = Box::new(Self {
            vault,
            settings,
            notes: Vec::new(),
            current_note: None,
            editor: Editor::new(),
            dirty: false,
            view: ViewMode::Editor,
            raw_text: String::new(),
            search: String::new(),
            new_note_title: String::new(),
            creating_new_note: false,
            show_settings: false,
            ai: AiState::default(),
            ai_rx: None,
            active_block_for_ai: None,
            toasts: Vec::new(),
            pending_open: None,
            pending_delete: None,
        });
        app.refresh_notes();

        if app.notes.is_empty() {
            let _ = app.ensure_default_note();
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
        if let Some(vault) = &self.vault {
            self.notes = vault.list_notes();
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
            .max_size(420.0)
            .frame(
                egui::Frame::NONE
                    .fill(if self.settings.dark_mode {
                        Color32::from_gray(24)
                    } else {
                        Color32::from_gray(242)
                    }),
            )
            .show(ui, |ui| {
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
                        if icon_button(ui, "sun-moon", "Toggle light/dark mode") {
                            self.toggle_theme(&ctx);
                        }
                        if icon_button(ui, "plus", "New note (Cmd/Ctrl+N)") {
                            self.creating_new_note = true;
                        }
                    });
                });
                ui.add_space(6.0);
                // Search box
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(icon("magnifying-glass").size(13.0).color(muted(self.settings.dark_mode)));
                    let search = TextEdit::singleline(&mut self.search)
                        .hint_text("Search notes…")
                        .desired_width(ui.available_width() - 12.0)
                        .frame(egui::Frame::NONE);
                    ui.add(search);
                    ui.add_space(4.0);
                });
                ui.separator();
                ui.add_space(2.0);

                // Note list
                ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    let query = self.search.trim().to_lowercase();
                    let notes = self.notes.clone();
                    for (i, note) in notes
                        .iter()
                        .filter(|n| query.is_empty() || n.rel.to_lowercase().contains(&query))
                        .enumerate()
                    {
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
                        let job = icon_job(
                            "file-text",
                            &note.rel,
                            13.5,
                            text_color,
                        );
                        if ui
                            .selectable_label(is_current, job)
                            .clicked()
                        {
                            self.pending_open = Some(i);
                        }
                    }
                });

                // Vault path footer
                ui.separator();
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
                ui.add_space(4.0);
            });

        // Deferred actions (avoid mutating while borrowing notes).
        if let Some(i) = self.pending_open.take() {
            self.open_note(i);
        }
        if let Some(path) = self.pending_delete.take() {
            let was_current = self
                .current_note
                .as_ref()
                .map(|c| c.path == path)
                .unwrap_or(false);
            match self.vault.as_ref().map(|v| v.delete_note(&path)) {
                Some(Ok(())) => {
                    self.toast("Note deleted");
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

    fn draw_main(&mut self, ui: &mut Ui) {
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
                        if icon_button(ui, "trash", "Delete this note") {
                            if let Some(path) = self.current_note.as_ref().map(|n| n.path.clone()) {
                                self.pending_delete = Some(path);
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

                        // Editor / Raw tabs
                        let editor_sel = self.view == ViewMode::Editor;
                        let raw_sel = self.view == ViewMode::Raw;
                        if ui
                            .add(egui::Button::new(icon_job(
                                "split-horizontal",
                                "Editor",
                                13.0,
                                if editor_sel {
                                    accent(self.settings.dark_mode)
                                } else {
                                    normal_text(self.settings.dark_mode)
                                },
                            )))
                            .clicked()
                        {
                            if self.view == ViewMode::Raw {
                                self.sync_raw_to_editor();
                                self.raw_text.clear();
                            }
                            self.view = ViewMode::Editor;
                        }
                        if ui
                            .add(egui::Button::new(icon_job(
                                "markdown-logo",
                                "Raw",
                                13.0,
                                if raw_sel {
                                    accent(self.settings.dark_mode)
                                } else {
                                    normal_text(self.settings.dark_mode)
                                },
                            )))
                            .on_hover_text("Edit raw markdown")
                            .clicked()
                        {
                            if self.view == ViewMode::Editor {
                                self.raw_text = self.editor.to_markdown();
                            }
                            self.view = ViewMode::Raw;
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
                            self.editor.show(ui, &mut modified);
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
                .anchor(egui::Align2::RIGHT_TOP, [-16.0, 16.0 + 34.0 * i as f32])
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
