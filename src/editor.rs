//! Block-based editor with a Notion-style "/" slash menu.

use crate::blocks::{parse, serialize, Block, BlockKind};
use eframe::egui::{self, Area, Color32, Frame, Id, Key, Order, RichText, ScrollArea, Sense, TextEdit, Ui, Vec2};
use std::collections::HashSet;

pub struct Editor {
    pub blocks: Vec<Block>,
    next_id: u64,
    /// Block to focus on the next frame (and keep focused).
    focus_block: Option<u64>,
    /// Open "/" block-type menu.
    slash: Option<SlashState>,
    /// Blocks whose text area was hovered on the previous frame (gutter icons).
    hovered: HashSet<u64>,
    /// Last block the user focused (target for AI actions).
    pub last_focused: Option<u64>,
    /// Persisted rectangles for anchoring the slash popup.
    block_rects: std::collections::HashMap<u64, egui::Rect>,
    /// Full row rects from the previous frame (drag-select hit testing).
    row_rects: std::collections::HashMap<u64, egui::Rect>,
    /// Blocks currently selected (Cmd/Ctrl+A or drag-select).
    selected_blocks: HashSet<u64>,
    /// Anchor y of an in-progress drag selection (None = not dragging).
    select_drag: Option<f32>,
    /// Gutter button rects from the previous frame (so a press on the
    /// handle/plus button is not treated as the start of a drag-select).
    gutter_btn_rects: Vec<egui::Rect>,
    /// Set while a slash-menu key was consumed this frame.
    ate_key: bool,
    /// Open "Link to note" file picker (searchable dropdown).
    link_pick: Option<LinkPickState>,
    /// Focus the picker's search field on the next frame.
    link_focus_requested: bool,
    /// True on the frame the picker was opened — its opening click must not
    /// be treated as an "outside click" that closes it.
    link_pick_fresh: bool,
}

struct SlashState {
    block_id: u64,
    selected: usize,
}

/// One entry in the "/" menu: a block kind, or the special "Link to note" action.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SlashEntry {
    Kind(BlockKind),
    Link,
}

impl SlashEntry {
    fn label(&self) -> &'static str {
        match self {
            SlashEntry::Kind(k) => k.label(),
            SlashEntry::Link => "Link to note",
        }
    }
    fn icon_name(&self) -> &'static str {
        match self {
            SlashEntry::Kind(k) => k.icon_name(),
            SlashEntry::Link => "link",
        }
    }
    fn matches(&self, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        if self.label().to_lowercase().contains(filter) {
            return true;
        }
        let kws: &[&str] = match self {
            SlashEntry::Kind(k) => k.keywords(),
            SlashEntry::Link => &["link", "note", "page", "file", "wiki", "internal", "reference"],
        };
        kws.iter().any(|kw| kw.contains(filter) || filter.contains(kw))
    }
}

/// Open "Link to note" picker state (dropdown with a search field).
struct LinkPickState {
    block_id: u64,
    search: String,
    selected: usize,
    /// Last row index the list auto-scrolled to (avoid fighting the user's
    /// own scrolling every frame).
    scrolled_for: i64,
}

/// A note offered by the link picker.
#[derive(Clone)]
pub struct LinkCandidate {
    pub title: String,
    pub rel: String,
}

impl Editor {
    pub fn new() -> Self {
        let mut ed = Self {
            blocks: Vec::new(),
            next_id: 1,
            focus_block: None,
            slash: None,
            hovered: HashSet::new(),
            last_focused: None,
            block_rects: std::collections::HashMap::new(),
            row_rects: std::collections::HashMap::new(),
            selected_blocks: HashSet::new(),
            select_drag: None,
            gutter_btn_rects: Vec::new(),
            ate_key: false,
            link_pick: None,
            link_focus_requested: false,
            link_pick_fresh: false,
        };
        ed.blocks.push(Block::new(ed.next_id, BlockKind::Paragraph));
        ed.next_id += 1;
        ed
    }

    pub fn load_markdown(&mut self, content: &str) {
        self.blocks = parse(content);
        self.next_id = self.blocks.iter().map(|b| b.id).max().unwrap_or(0) + 1;
        self.focus_block = None;
        self.slash = None;
        self.last_focused = None;
        self.selected_blocks.clear();
        self.select_drag = None;
        self.row_rects.clear();
        self.link_pick = None;
    }

    pub fn to_markdown(&self) -> String {
        serialize(&self.blocks)
    }

    /// Insert the AI result below the currently focused block.
    pub fn insert_ai_result(&mut self, result: &str) {
        let anchor = self.last_focused.or_else(|| self.blocks.last().map(|b| b.id));
        let pos = anchor
            .and_then(|id| self.blocks.iter().position(|b| b.id == id))
            .unwrap_or(self.blocks.len().saturating_sub(1));
        let mut new_blocks = Vec::new();
        for (i, line) in result.lines().enumerate() {
            let kind = if line.starts_with("# ") {
                BlockKind::Heading1
            } else if line.starts_with("## ") {
                BlockKind::Heading2
            } else if line.starts_with("### ") {
                BlockKind::Heading3
            } else if line.starts_with("- ") || line.starts_with("* ") {
                BlockKind::Bullet
            } else {
                BlockKind::Paragraph
            };
            let text = line
                .trim_start_matches("# ")
                .trim_start_matches("## ")
                .trim_start_matches("### ")
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .to_string();
            let mut b = Block::new(self.next_id, kind);
            self.next_id += 1;
            b.text = if line.is_empty() && i > 0 { String::new() } else { text };
            new_blocks.push(b);
        }
        if new_blocks.is_empty() {
            return;
        }
        let end = pos + 1;
        let first_new = new_blocks[0].id;
        let last_new = new_blocks.last().unwrap().id;
        self.blocks
            .splice(end..end, new_blocks);
        self.focus_block = Some(last_new);
        self.last_focused = Some(first_new);
    }

    pub fn show(&mut self, ui: &mut Ui, modified: &mut bool, notes: &[LinkCandidate]) {
        self.ate_key = false;
        let cmd = |i: &egui::InputState| {
            if cfg!(target_os = "macos") {
                i.modifiers.command
            } else {
                i.modifiers.ctrl
            }
        };
        let cmd_a = ui.input(|i| cmd(i) && i.key_pressed(Key::A));
        let (primary_pressed, primary_down) = ui
            .input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
        let pointer_pos = ui.input(|i| i.pointer.latest_pos());
        self.row_rects.clear();

        // Start / clear multi-block selection on click.
        if primary_pressed && pointer_pos.is_some() {
            let pos = pointer_pos.unwrap();
            let in_text = self.block_rects.values().any(|r| r.contains(pos));
            let on_gutter_btn = self.gutter_btn_rects.iter().any(|r| r.contains(pos));
            if in_text {
                self.selected_blocks.clear();
            } else if !on_gutter_btn && ui.clip_rect().min.x <= pos.x {
                self.select_drag = Some(pos.y);
                self.selected_blocks.clear();
            }
        }
        self.gutter_btn_rects.clear();

        // Handle slash-menu keyboard navigation before widgets consume keys.
        if let Some(slash) = &self.slash {
            let item_count = self.filtered_slash_items(slash.block_id).len();
            let input = ui.ctx().input(|i| {
                (
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::Enter) || i.key_pressed(Key::Tab),
                    i.key_pressed(Key::Escape),
                )
            });
            let (down, up, confirm, close) = input;
            if down && item_count > 0 {
                if let Some(s) = &mut self.slash {
                    s.selected = (s.selected + 1) % item_count;
                }
                self.ate_key = true;
            } else if up && item_count > 0 {
                if let Some(s) = &mut self.slash {
                    s.selected = (s.selected + item_count - 1) % item_count;
                }
                self.ate_key = true;
            } else if close {
                self.slash = None;
                self.ate_key = true;
            } else if confirm {
                let items = self.filtered_slash_items(slash.block_id);
                if let Some(&entry) = items.get(slash.selected.min(items.len().saturating_sub(1))) {
                    self.apply_slash_entry(slash.block_id, entry);
                }
                self.ate_key = true;
            }
        }

        // Link-picker keyboard navigation (arrows/enter/escape). Skipped on the
        // frame the picker was opened: the same Enter press that confirmed
        // "Link to note" would otherwise immediately confirm row 0 too.
        if !self.link_pick_fresh {
            if let Some(pick) = self
                .link_pick
                .as_ref()
                .map(|p| (p.block_id, p.search.clone(), p.selected))
            {
            let (block_id, search, selected) = pick;
            let items = filter_candidates(&search, notes);
            let count = items.len();
            let input = ui.ctx().input(|i| {
                (
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::Enter),
                    i.key_pressed(Key::Escape),
                )
            });
            let (down, up, confirm, close) = input;
            if close {
                self.link_pick = None;
                self.ate_key = true;
            } else if down && count > 0 {
                if let Some(p) = self.link_pick.as_mut() {
                    p.selected = (p.selected + 1) % count;
                }
                self.ate_key = true;
            } else if up && count > 0 {
                if let Some(p) = self.link_pick.as_mut() {
                    p.selected = (p.selected + count - 1) % count;
                }
                self.ate_key = true;
            } else if confirm {
                let idx = selected.min(count.saturating_sub(1));
                if let Some(c) = items.get(idx) {
                    let title = c.title.clone();
                    self.insert_link(block_id, &title, modified);
                }
                self.ate_key = true;
            }
            }
        }

        let mut hovered_now = HashSet::new();
        let mut slash_open_request: Option<u64> = None;
        let dark = ui.visuals().dark_mode;
        let mut select_all_blocks = false;
        // Deferred backspace merge (applied after the loop so `blocks` is
        // never mutated while it is being iterated).
        let mut pending_merge: Option<(u64, u64)> = None;

        // Precompute numbered-list indices.
        let mut numbered_index = 0usize;

        for idx in 0..self.blocks.len() {
            let block_id = self.blocks[idx].id;
            if self.blocks[idx].kind == BlockKind::Numbered {
                numbered_index += 1;
            } else if idx == 0 || self.blocks[idx - 1].kind == BlockKind::Numbered {
                numbered_index = 0;
            }

            let is_focused = self.focus_block == Some(block_id);
            let in_slash = self.slash.as_ref().map(|s| s.block_id) == Some(block_id);
            let is_selected = self.selected_blocks.contains(&block_id);

            let row_resp = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // ----- Gutter: handle + add button (visible on hover) -----
                let show_gutter = is_focused || in_slash || self.hovered.contains(&block_id);
                // Always reserve the exact same gutter rect whether or not the
                // buttons are drawn, so the text never shifts when they appear.
                // Layout: [2px] handle(22) [2px] plus(22) [24px prefix] → text.
                //
                // The icons are PAINTED (not egui Buttons) because interactive
                // widgets have a minimum size of `interact_size` (40px wide),
                // which made them overflow their slots and crowd the text.
                // Layout: [2px] handle(22) [2px] plus(22) [2px] [24px prefix] → text.
                let gutter_width = 48.0;
                let (gutter_rect, gutter_resp) = ui
                    .allocate_exact_size(Vec2::new(gutter_width, 20.0), Sense::hover());
                if gutter_resp.hovered() {
                    // Hovering the gutter strip keeps the buttons visible.
                    hovered_now.insert(block_id);
                }
                if show_gutter {
                    let handle_rect = egui::Rect::from_min_size(
                        gutter_rect.left_top() + Vec2::new(2.0, 0.0),
                        Vec2::new(22.0, 20.0),
                    );
                    let plus_rect = egui::Rect::from_min_size(
                        gutter_rect.left_top() + Vec2::new(26.0, 0.0),
                        Vec2::new(22.0, 20.0),
                    );
                    let icon_font =
                        egui::FontId::new(14.0, egui::FontFamily::Name("phosphor-regular".into()));
                    let icon_col = Color32::from_gray(120);
                    let hover_bg = ui.visuals().widgets.hovered.bg_fill;

                    let handle_resp = ui.interact(
                        handle_rect,
                        Id::new(("gutter_handle", block_id)),
                        Sense::click(),
                    );
                    let plus_resp = ui.interact(
                        plus_rect,
                        Id::new(("gutter_plus", block_id)),
                        Sense::click(),
                    );

                    // The click-sense widgets above sit on top of the outer
                    // hover-sense gutter rect and block its hover when the
                    // pointer is over a button — keep the buttons visible by
                    // counting button hover too (prevents flicker).
                    if handle_resp.hovered() || plus_resp.hovered() {
                        hovered_now.insert(block_id);
                    }
                    self.gutter_btn_rects.push(handle_rect);
                    self.gutter_btn_rects.push(plus_rect);

                    if handle_resp.hovered() {
                        ui.painter().rect_filled(handle_rect, 4.0, hover_bg);
                    }
                    if plus_resp.hovered() {
                        ui.painter().rect_filled(plus_rect, 4.0, hover_bg);
                    }
                    ui.painter().text(
                        handle_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon("dots-six-vertical").text().to_string(),
                        icon_font.clone(),
                        icon_col,
                    );
                    ui.painter().text(
                        plus_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon("plus").text().to_string(),
                        icon_font,
                        icon_col,
                    );

                    if handle_resp.on_hover_text("Block actions").clicked() {
                        self.block_menu(block_id);
                    }
                    if plus_resp.on_hover_text("Insert block below").clicked() {
                        self.insert_block_below(block_id, BlockKind::Paragraph);
                    }
                }

                // ----- Uniform prefix strip: every block's text starts at the
                // same x. List markers / checkboxes / quote bars are drawn in
                // this fixed 24px strip instead of shifting the text. -----
                let kind = self.blocks[idx].kind;
                let sense = if kind == BlockKind::Todo {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (prefix_rect, prefix_resp) =
                    ui.allocate_exact_size(Vec2::new(24.0, 20.0), sense);
                match kind {
                    BlockKind::Bullet => {
                        ui.painter().circle_filled(
                            prefix_rect.right_center() - Vec2::new(6.0, 0.0),
                            2.5,
                            if dark { Color32::from_gray(200) } else { Color32::from_gray(120) },
                        );
                    }
                    BlockKind::Numbered => {
                        ui.put(
                            prefix_rect,
                            egui::Label::new(
                                RichText::new(format!("{numbered_index}."))
                                    .color(if dark { Color32::from_gray(170) } else { Color32::from_gray(110) })
                                    .size(14.0),
                            )
                            .halign(egui::Align::Max),
                        );
                    }
                    BlockKind::Todo => {
                        let r = egui::Rect::from_center_size(
                            prefix_rect.right_center() - Vec2::new(5.0, 0.0),
                            Vec2::new(16.0, 16.0),
                        );
                        let rounding = 4.0;
                        let stroke = egui::Stroke::new(
                            1.4,
                            if dark { Color32::from_gray(150) } else { Color32::from_gray(120) },
                        );
                        let checked = self.blocks[idx].checked;
                        if checked {
                            ui.painter()
                                .rect_filled(r, rounding, Color32::from_rgb(80, 140, 255));
                            ui.painter().line_segment(
                                [r.center() - Vec2::new(3.5, 0.5), r.center() - Vec2::new(1.0, 3.0)],
                                egui::Stroke::new(1.6, Color32::WHITE),
                            );
                            ui.painter().line_segment(
                                [r.center() - Vec2::new(1.0, 3.0), r.center() + Vec2::new(3.5, 2.5)],
                                egui::Stroke::new(1.6, Color32::WHITE),
                            );
                        } else {
                            ui.painter().rect_stroke(r, rounding, stroke, egui::StrokeKind::Middle);
                        }
                        if prefix_resp.clicked() {
                            self.blocks[idx].checked = !checked;
                            *modified = true;
                        }
                    }
                    BlockKind::Quote => {
                        let bar = egui::Rect::from_min_size(
                            prefix_rect.right_top() - Vec2::new(6.0, 2.0),
                            Vec2::new(4.0, prefix_rect.height() + 4.0),
                        );
                        ui.painter().rect_filled(bar, 2.0, Color32::from_rgb(90, 140, 220));
                    }
                    _ => {}
                }

                // ----- Block content -----
                let kind = self.blocks[idx].kind;
                match kind {
                    BlockKind::Divider => {
                        let avail = ui.available_width();
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(avail, 14.0),
                            Sense::click(),
                        );                        ui.painter().line_segment(
                            [
                                rect.left_top() + Vec2::new(4.0, rect.height() / 2.0),
                                rect.right_top() + Vec2::new(-4.0, rect.height() / 2.0),
                            ],
                            egui::Stroke::new(1.0, Color32::from_gray(90)),
                        );
                        if resp.clicked() {
                            self.focus_block = Some(block_id);
                        }
                        self.block_rects.insert(block_id, resp.rect);
                    }
                    BlockKind::Code => {
                        let mut text = std::mem::take(&mut self.blocks[idx].text);
                        Frame::default()
                            .fill(if dark { Color32::from_gray(28) } else { Color32::from_gray(238) })
                            .inner_margin(8.0)
                            .corner_radius(6.0)
                            .show(ui, |ui| {
                                let out = TextEdit::multiline(&mut text)
                                    .id(Id::new(("block", block_id)))
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(ui.available_width() - 16.0)                                    .frame(egui::Frame::NONE)
                                    .code_editor()
                                    .show(ui);
                                if self.focus_block == Some(block_id) {
                                    if out.response.has_focus() {
                                        self.focus_block = None;
                                    } else {
                                        out.response.request_focus();
                                    }
                                }
                                if out.response.has_focus() {
                                    self.hovered.insert(block_id);
                                    hovered_now.insert(block_id);
                                    self.last_focused = Some(block_id);
                                }
                                if out.response.hovered() {
                                    hovered_now.insert(block_id);
                                }
                            });
                        self.blocks[idx].text = text;
                    }
                    _ => {
                        let mut text = std::mem::take(&mut self.blocks[idx].text);

                        let out = TextEdit::singleline(&mut text)
                            .id(Id::new(("block", block_id)))
                            .font(text_style_for(kind))
                            .desired_width(ui.available_width())
                            .frame(egui::Frame::NONE)
                            .show(ui);

                        // Move keyboard focus into this block when an action
                        // (Enter, slash pick, + button, …) targeted it. Cleared
                        // once the focus has landed so we never fight the user
                        // clicking into a different block.
                        if self.focus_block == Some(block_id) {
                            if out.response.has_focus() {
                                self.focus_block = None;
                            } else {
                                out.response.request_focus();
                            }
                        }

                        // Arrow up/down moves the cursor between blocks.
                        if out.response.has_focus() && !in_slash {
                            let (down, up) = ui.input(|i| {
                                (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp))
                            });
                            if down && idx + 1 < self.blocks.len() {
                                self.focus_block = Some(self.blocks[idx + 1].id);
                            } else if up && idx > 0 {
                                self.focus_block = Some(self.blocks[idx - 1].id);
                            }
                        }

                        // Cmd/Ctrl+A: if this block's text is already fully
                        // selected, select ALL blocks instead (Notion-style).
                        if out.response.has_focus()
                            && cmd_a
                            && !in_slash
                            && out
                                .cursor_range
                                .map(|sel| {
                                    let (a, b) =
                                        (sel.primary.index.0, sel.secondary.index.0);
                                    let (lo, hi) = (a.min(b), a.max(b));
                                    lo == 0
                                        && hi >= self.blocks[idx].text.chars().count()
                                })
                                .unwrap_or(false)
                        {
                            select_all_blocks = true;
                            self.ate_key = true;
                        }

                        if out.response.has_focus() {
                            self.hovered.insert(block_id);
                            hovered_now.insert(block_id);
                            self.last_focused = Some(block_id);
                        }
                        if out.response.hovered() {
                            hovered_now.insert(block_id);
                        }

                        self.block_rects.insert(block_id, out.response.rect);

                        // "/" opens the slash menu (typed in an otherwise-empty block).
                        if out.response.changed() && text == "/" {
                            slash_open_request = Some(block_id);
                        }

                        let enter = out.response.lost_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter))
                            && !self.ate_key;

                        // Backspace at the very start of a block merges it
                        // into the previous block (Notion-style).
                        let cursor_at_start = out
                            .cursor_range
                            .map(|c| {
                                let (a, b) = (c.primary.index.0, c.secondary.index.0);
                                a == 0 && b == 0
                            })
                            .unwrap_or(false);
                        let backspace_merge = out.response.has_focus()
                            && ui.input(|i| i.key_pressed(Key::Backspace))
                            && !in_slash
                            && (text.is_empty() || cursor_at_start)
                            && self.blocks.len() > 1;

                        self.blocks[idx].text = text;

                        if enter {
                            let prev_kind = self.blocks[idx].kind;
                            let cur_empty = self.blocks[idx].text.trim().is_empty();
                            let remainder = out
                                .cursor_range
                                .map(|c| c.primary.index.0)
                                .filter(|&i| i <= self.blocks[idx].text.len())
                                .map(|i| self.blocks[idx].text.split_off(i))
                                .unwrap_or_default();
                            let new_id = self.insert_block_below(block_id, BlockKind::Paragraph);
                            // Auto-continue lists: pressing Enter in a list item makes a new
                            // item; pressing Enter on an empty list item exits the list.
                            let (prev_kind, new_kind) = match prev_kind {
                                BlockKind::Bullet | BlockKind::Numbered | BlockKind::Todo => {
                                    if cur_empty {
                                        (BlockKind::Paragraph, BlockKind::Paragraph)
                                    } else {
                                        (prev_kind, prev_kind)
                                    }
                                }
                                BlockKind::Heading1
                                | BlockKind::Heading2
                                | BlockKind::Heading3
                                | BlockKind::Quote => (prev_kind, BlockKind::Paragraph),
                                other => (other, BlockKind::Paragraph),
                            };
                            self.blocks[idx].kind = prev_kind;
                            if let Some(nb) = self.blocks.iter_mut().find(|b| b.id == new_id) {
                                nb.kind = new_kind;
                                nb.text = remainder;
                                nb.checked = false;
                            }
                            self.focus_block = Some(new_id);
                            *modified = true;
                        } else if backspace_merge {
                            let prev_id = if idx > 0 {
                                self.blocks[idx - 1].id
                            } else {
                                0
                            };
                            if prev_id != 0 {
                                pending_merge = Some((prev_id, block_id));
                            }
                        }
                    }
                }
            });

            // Record the row rect (extended across the full width) for
            // drag-select hit testing, and tint selected rows.
            let mut row_r = row_resp.response.rect;
            row_r.max.x = row_r.min.x + 4000.0;
            self.row_rects.insert(block_id, row_r);
            if is_selected || self.selected_blocks.contains(&block_id) {
                ui.painter().rect_filled(
                    row_resp.response.rect,
                    4.0,
                    Color32::from_rgba_unmultiplied(130, 175, 255, 26),
                );
            }
        }

        self.hovered = hovered_now;

        // Apply a deferred backspace merge (safe now: the block loop is done).
        if let Some((prev_id, cur_id)) = pending_merge {
            if let Some(cur_pos) = self.blocks.iter().position(|b| b.id == cur_id) {
                let cur = self.blocks.remove(cur_pos);
                if let Some(prev) = self.blocks.iter_mut().find(|b| b.id == prev_id) {
                    prev.text.push_str(&cur.text);
                }
                self.focus_block = Some(prev_id);
                self.last_focused = Some(prev_id);
                self.selected_blocks.clear();
                self.slash = None;
                *modified = true;
            }
        }

        // Cmd/Ctrl+A → select all blocks.
        if select_all_blocks {
            self.selected_blocks = self.blocks.iter().map(|b| b.id).collect();
        }

        // Update the drag selection while the pointer is held down.
        if primary_down && pointer_pos.is_some() {
            if let Some(anchor) = self.select_drag {
                let pos = pointer_pos.unwrap();
                let (top, bottom) = if pos.y < anchor {
                    (pos.y, anchor)
                } else {
                    (anchor, pos.y)
                };
                self.selected_blocks.retain(|id| {
                    self.row_rects
                        .get(id)
                        .map(|r| r.top() <= bottom && r.bottom() >= top)
                        .unwrap_or(false)
                });
                for (id, r) in &self.row_rects {
                    if r.top() <= bottom && r.bottom() >= top {
                        self.selected_blocks.insert(*id);
                    }
                }
            }
        } else {
            self.select_drag = None;
        }

        // Delete / escape for multi-block selection (only when no text field
        // has focus, so typing in a block is never hijacked).
        if !self.selected_blocks.is_empty() {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.selected_blocks.clear();
            }
            let delete_pressed =
                ui.input(|i| i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete));
            let no_text_focus = ui.ctx().memory(|m| m.focused().is_none());
            if delete_pressed && no_text_focus {
                self.blocks.retain(|b| !self.selected_blocks.contains(&b.id));
                self.selected_blocks.clear();
                self.last_focused = None;
                if self.blocks.is_empty() {
                    self.blocks
                        .push(Block::new(self.next_id, BlockKind::Paragraph));
                    self.next_id += 1;
                }
                *modified = true;
            }
        }

        // Slash menu trigger from this frame.
        if let Some(block_id) = slash_open_request {
            if self.slash.is_none() {
                self.slash = Some(SlashState { block_id, selected: 0 });
            }
        }

        // Close the slash menu when its block no longer starts with "/".
        if let Some(slash) = &self.slash {
            let still_slashing = self
                .blocks
                .iter()
                .find(|b| b.id == slash.block_id)
                .map(|b| b.text.starts_with('/'))
                .unwrap_or(false);
            if !still_slashing {
                self.slash = None;
            }
        }

        // Render the slash popup.
        if let Some(slash) = &self.slash {
            let block_id = slash.block_id;
            if let Some(rect) = self.block_rects.get(&block_id).copied() {
                self.render_slash_menu(ui, block_id, rect);
            }
        }

        // Render the link picker popup.
        if let Some(pick) = &self.link_pick {
            let block_id = pick.block_id;
            if let Some(rect) = self.block_rects.get(&block_id).copied() {
                self.render_link_picker(ui, block_id, rect, notes, modified);
            } else {
                self.link_pick = None;
            }
        }

        // Clicking on empty space below the content adds a new paragraph.
        let space_rect = ui.available_rect_before_wrap();
        let min_space = Vec2::new(space_rect.width(), 120.0);
        let (space, space_resp) =
            ui.allocate_exact_size(min_space, Sense::click());
        if space_resp.clicked() {
            if let Some(last) = self.blocks.last().map(|b| b.id) {
                if self
                    .blocks
                    .iter()
                    .find(|b| b.id == last)
                    .map(|b| b.text.trim().is_empty() && b.kind == BlockKind::Paragraph)
                    .unwrap_or(false)
                {
                    self.focus_block = Some(last);
                } else {
                    let id = self.insert_block_below(last, BlockKind::Paragraph);
                    self.focus_block = Some(id);
                }
                *modified = true;
            }
        }
        let _ = space;
    }

    fn render_slash_menu(&mut self, ui: &mut Ui, block_id: u64, rect: egui::Rect) {
        let items = self.filtered_slash_items(block_id);
        let selected = self.slash.as_ref().map(|s| s.selected).unwrap_or(0);

        // Theme-aware colors (the menu previously used hardcoded dark colors,
        // which looked broken in light mode).
        let dark = ui.visuals().dark_mode;
        let (menu_fill, sel_fill, text_col) = if dark {
            (
                Color32::from_gray(32),
                Color32::from_rgb(45, 70, 110),
                Color32::from_gray(225),
            )
        } else {
            (
                Color32::from_gray(252),
                Color32::from_rgb(219, 233, 253),
                Color32::from_gray(30),
            )
        };

        // Fixed-width rows: a content-sized popup oscillates in width
        // (shrinking/growing from the right) as the selection changes.
        let menu_width = 220.0;

        let area_id = Id::new("slash_menu").with(block_id);
        Area::new(area_id)
            .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 4.0))
            .order(Order::Foreground)
            .movable(false)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style())
                    .fill(menu_fill)
                    .inner_margin(4.0)
                    .corner_radius(8.0)
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(280.0)
                            .show(ui, |ui| {
                                for (i, entry) in items.iter().enumerate() {
                                    let is_sel = i == selected.min(items.len().saturating_sub(1));
                                    let job = icon_job(
                                        entry.icon_name(),
                                        entry.label(),
                                        14.0,
                                        text_col,
                                    );
                                    let btn = egui::Button::new(job)
                                        .fill(if is_sel { sel_fill } else { Color32::TRANSPARENT })
                                        .min_size(Vec2::new(menu_width - 8.0, 26.0));
                                    let resp = ui.add(btn);
                                    if resp.clicked() {
                                        self.apply_slash_entry(block_id, *entry);
                                    }
                                }
                            });
                    });
            });
    }

    fn filtered_slash_items(&self, block_id: u64) -> Vec<SlashEntry> {
        let filter = self
            .blocks
            .iter()
            .find(|b| b.id == block_id)
            .map(|b| b.text.trim_start_matches('/').trim().to_lowercase())
            .unwrap_or_default();
        let mut items: Vec<SlashEntry> = BlockKind::slash_menu_items()
            .iter()
            .map(|k| SlashEntry::Kind(*k))
            .collect();
        items.push(SlashEntry::Link);
        items.retain(|e| e.matches(&filter));
        items
    }

    fn apply_slash_entry(&mut self, block_id: u64, entry: SlashEntry) {
        match entry {
            SlashEntry::Kind(kind) => self.apply_slash(block_id, kind),
            SlashEntry::Link => {
                // Drop the "/" filter text, but keep anything typed before it.
                if let Some(b) = self.blocks.iter_mut().find(|b| b.id == block_id) {
                    if let Some(pos) = b.text.find('/') {
                        b.text.truncate(pos);
                    }
                }
                self.slash = None;
                self.link_pick = Some(LinkPickState {
                    block_id,
                    search: String::new(),
                    selected: 0,
                    scrolled_for: i64::MIN,
                });
                self.link_focus_requested = true;
                self.link_pick_fresh = true;
                self.focus_block = None;
            }
        }
    }

    /// Insert a `[[wikilink]]` to the chosen note at the end of the block.
    fn insert_link(&mut self, block_id: u64, title: &str, modified: &mut bool) {
        let link = format!("[[{}]]", title);
        if let Some(b) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            b.text.push_str(&link);
        }
        self.link_pick = None;
        self.focus_block = Some(block_id);
        self.last_focused = Some(block_id);
        self.selected_blocks.clear();
        *modified = true;
    }

    /// Searchable dropdown of vault notes, anchored below the block.
    fn render_link_picker(
        &mut self,
        ui: &mut Ui,
        block_id: u64,
        rect: egui::Rect,
        notes: &[LinkCandidate],
        modified: &mut bool,
    ) {
        let dark = ui.visuals().dark_mode;
        let (menu_fill, sel_fill, text_col, muted_col) = if dark {
            (
                Color32::from_gray(32),
                Color32::from_rgb(45, 70, 110),
                Color32::from_gray(225),
                Color32::from_gray(130),
            )
        } else {
            (
                Color32::from_gray(252),
                Color32::from_rgb(219, 233, 253),
                Color32::from_gray(30),
                Color32::from_gray(130),
            )
        };

        let Some(pick) = self.link_pick.as_ref() else {
            return;
        };
        let (search, selected, mut scrolled_for) =
            (pick.search.clone(), pick.selected, pick.scrolled_for);
        let filtered = filter_candidates(&search, notes);
        // Skip the "click outside closes" check on the frame the picker was
        // opened: the click on the slash-menu item is still being processed
        // and would otherwise close the picker before it is ever visible.
        let fresh = std::mem::take(&mut self.link_pick_fresh);

        // Fixed popup width — the Area floats freely, so `available_width`
        // would otherwise stretch it across the whole screen.
        const PICKER_WIDTH: f32 = 300.0;

        let mut search_str = search;
        let mut picked: Option<String> = None;
        let area_id = Id::new("link_picker").with(block_id);
        let area = Area::new(area_id)
            .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 4.0))
            .order(Order::Foreground)
            .movable(false)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style())
                    .fill(menu_fill)
                    .inner_margin(6.0)
                    .corner_radius(8.0)
                    .show(ui, |ui| {
                        ui.set_width(PICKER_WIDTH);
                        // Search field at the top of the dropdown.
                        let resp = TextEdit::singleline(&mut search_str)
                            .id(Id::new(("link_search", block_id)))
                            .hint_text("Search notes…")
                            .desired_width(PICKER_WIDTH - 14.0)
                            .font(egui::TextStyle::Body)
                            .show(ui);
                        if self.link_focus_requested {
                            resp.response.request_focus();
                            self.link_focus_requested = false;
                        }
                        ui.separator();
                        if filtered.is_empty() {
                            ui.label(RichText::new("No matching notes").size(12.0).color(muted_col));
                        } else {
                            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                                for (i, c) in filtered.iter().enumerate() {
                                    let is_sel = i == selected.min(filtered.len() - 1);
                                    let job = icon_job("file-text", &c.rel, 13.5, text_col);
                                    let btn = egui::Button::new(job)
                                        .fill(if is_sel {
                                            sel_fill
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .min_size(Vec2::new(PICKER_WIDTH - 10.0, 26.0));
                                    let resp = ui.add(btn);
                                    // Ignore clicks on the opening frame: the
                                    // click that selected "Link to note" in the
                                    // slash menu is still active and would hit
                                    // whichever row sits under the pointer.
                                    if !fresh && resp.clicked() {
                                        picked = Some(c.title.clone());
                                    }
                                    // Keep the keyboard-selected row in view.
                                    if is_sel && scrolled_for != i as i64 {
                                        resp.scroll_to_me(Some(egui::Align::Center));
                                        scrolled_for = i as i64;
                                    }
                                }
                            });
                        }
                    });
            });
        // Persist the search text + scroll position back into the picker state.
        if let Some(p) = self.link_pick.as_mut() {
            if p.block_id == block_id {
                p.search = search_str;
                p.scrolled_for = scrolled_for;
            }
        }
        // Close when clicking anywhere outside the dropdown.
        if !fresh && ui.input(|i| i.pointer.any_click()) {
            if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
                if !area.response.rect.contains(p) {
                    self.link_pick = None;
                    return;
                }
            }
        }
        if let Some(title) = picked {
            self.insert_link(block_id, &title, modified);
        }
    }

    fn apply_slash(&mut self, block_id: u64, kind: BlockKind) {
        if let Some(b) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            b.kind = kind;
            b.text.clear();
            b.checked = false;
        }
        self.slash = None;
        self.focus_block = Some(block_id);
    }

    fn block_menu(&mut self, block_id: u64) {
        // Simple built-in menu: cycle type via a small popup is complex;
        // here we open the slash menu in "change type" mode by pre-filling "/".
        if let Some(b) = self.blocks.iter_mut().find(|b| b.id == block_id) {
            b.text = "/".to_string();
        }
        self.slash = Some(SlashState { block_id, selected: 0 });
        self.focus_block = Some(block_id);
    }

    fn insert_block_below(&mut self, block_id: u64, kind: BlockKind) -> u64 {
        let pos = self
            .blocks
            .iter()
            .position(|b| b.id == block_id)
            .map(|p| p + 1)
            .unwrap_or(self.blocks.len());
        let b = Block::new(self.next_id, kind);
        self.next_id += 1;
        let id = b.id;
        self.blocks.insert(pos, b);
        id
    }
}

fn text_style_for(kind: BlockKind) -> egui::TextStyle {
    match kind {
        BlockKind::Heading1 | BlockKind::Heading2 => egui::TextStyle::Heading,
        BlockKind::Heading3 | BlockKind::Quote => egui::TextStyle::Body,
        _ => egui::TextStyle::Body,
    }
}

/// Filter link-picker candidates by title or vault-relative path.
fn filter_candidates(query: &str, notes: &[LinkCandidate]) -> Vec<LinkCandidate> {
    let q = query.trim().to_lowercase();
    notes
        .iter()
        .filter(|c| {
            q.is_empty()
                || c.title.to_lowercase().contains(&q)
                || c.rel.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

fn icon(name: &str) -> RichText {
    match egui_phosphor_icons::Icon::from_name(name) {
        Some(i) => i.regular(),
        None => RichText::new("\u{2022}"),
    }
}

/// Icon + text composed with the correct font families.
fn icon_job(name: &str, text: &str, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &icon(name).text().to_string(),
        4.0,
        egui::TextFormat::simple(
            egui::FontId::new(size, egui::FontFamily::Name("phosphor-regular".into())),
            color,
        ),
    );
    job.append(
        text,
        0.0,
        egui::TextFormat::simple(egui::FontId::new(size, egui::FontFamily::Proportional), color),
    );
    job
}
