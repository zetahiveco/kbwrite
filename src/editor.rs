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
    /// Set while a slash-menu key was consumed this frame.
    ate_key: bool,
}

struct SlashState {
    block_id: u64,
    selected: usize,
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
            ate_key: false,
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

    pub fn show(&mut self, ui: &mut Ui, modified: &mut bool) {
        self.ate_key = false;

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
                if let Some(&kind) = items.get(slash.selected.min(items.len().saturating_sub(1))) {
                    self.apply_slash(slash.block_id, kind);
                }
                self.ate_key = true;
            }
        }

        let mut hovered_now = HashSet::new();
        let mut slash_open_request: Option<u64> = None;

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

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // ----- Gutter: handle + add button (visible on hover) -----
                let show_gutter = is_focused || in_slash || self.hovered.contains(&block_id);
                let gutter_width = 34.0;
                if show_gutter {
                    let handle = egui::Button::new(
                        icon("dots-six-vertical")
                            .size(14.0)
                            .color(Color32::from_gray(120)),
                    )
                    .small();
                    if ui
                        .add_sized([17.0, 20.0], handle)
                        .on_hover_text("Block actions")
                        .clicked()
                    {
                        self.block_menu(block_id);
                    }
                    let add = egui::Button::new(
                        icon("plus").size(14.0).color(Color32::from_gray(120)),
                    )
                    .small();
                    if ui
                        .add_sized([17.0, 20.0], add)
                        .on_hover_text("Insert block below")
                        .clicked()
                    {
                        self.insert_block_below(block_id, BlockKind::Paragraph);
                    }
                } else {
                    ui.add_space(gutter_width);
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
                            .fill(Color32::from_gray(28))
                            .inner_margin(8.0)
                            .corner_radius(6.0)
                            .show(ui, |ui| {
                                let out = TextEdit::multiline(&mut text)
                                    .id(Id::new(("block", block_id)))
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(ui.available_width() - 16.0)                                    .frame(egui::Frame::NONE)
                                    .code_editor()
                                    .show(ui);
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
                        let checked = self.blocks[idx].checked;

                        // Row prefix (bullet / number / checkbox / quote).
                        let mut prefix_width = 0.0;
                        match kind {
                            BlockKind::Bullet => {
                                ui.add_space(6.0);
                                let (r, _) = ui.allocate_exact_size(
                                    Vec2::new(14.0, ui.spacing().interact_size.y),
                                    Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    r.center(),
                                    2.5,
                                    Color32::from_gray(200),
                                );
                                prefix_width = 20.0;
                            }
                            BlockKind::Numbered => {
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(format!("{numbered_index}."))
                                        .color(Color32::from_gray(170))
                                        .size(14.0),
                                );
                                prefix_width = 22.0;
                            }
                            BlockKind::Todo => {
                                let (r, resp) = ui.allocate_exact_size(
                                    Vec2::new(16.0, 16.0),
                                    Sense::click(),
                                );
                                let rounding = 4.0;
                                let stroke =
                                    egui::Stroke::new(1.4, Color32::from_gray(150));
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
                                if resp.clicked() {
                                    self.blocks[idx].checked = !checked;
                                    *modified = true;
                                }
                                prefix_width = 22.0;
                            }
                            BlockKind::Quote => {
                                let (r, _) = ui.allocate_exact_size(
                                    Vec2::new(4.0, ui.spacing().interact_size.y),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    r,
                                    2.0,
                                    Color32::from_rgb(90, 140, 220),
                                );
                                ui.add_space(4.0);
                                prefix_width = 10.0;
                            }
                            _ => {}
                        }
                        let _ = prefix_width;

                        let out = TextEdit::singleline(&mut text)
                            .id(Id::new(("block", block_id)))
                            .font(text_style_for(kind))
                            .desired_width(ui.available_width())
                            .frame(egui::Frame::NONE)
                            .show(ui);

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

                        // Backspace on empty block merges with previous.
                        let backspace_merge = text.is_empty()
                            && out.response.has_focus()
                            && ui.input(|i| i.key_pressed(Key::Backspace))
                            && self.blocks.len() > 1
                            && !in_slash;

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
                                let cur = self.blocks.remove(idx);
                                if let Some(prev) =
                                    self.blocks.iter_mut().find(|b| b.id == prev_id)
                                {
                                    prev.text.push_str(&cur.text);
                                }
                                self.focus_block = Some(prev_id);
                                *modified = true;
                            }
                        }
                    }
                }
            });
        }

        self.hovered = hovered_now;

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

        let area_id = Id::new("slash_menu").with(block_id);
        Area::new(area_id)
            .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 4.0))
            .order(Order::Foreground)
            .movable(false)
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style())
                    .fill(Color32::from_gray(32))
                    .inner_margin(4.0)
                    .corner_radius(8.0)
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .max_height(280.0)
                            .show(ui, |ui| {
                                for (i, kind) in items.iter().enumerate() {
                                    let is_sel = i == selected.min(items.len().saturating_sub(1));
                                    let bg = if is_sel {
                                        Color32::from_rgb(45, 70, 110)
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    let job = icon_job(
                                        kind.icon_name(),
                                        kind.label(),
                                        14.0,
                                        Color32::from_gray(225),
                                    );
                                    let btn = egui::Button::new(job)
                                        .fill(bg)
                                        .min_size(Vec2::new(ui.available_width() - 8.0, 24.0));
                                    let resp = ui.add(btn);
                                    if resp.clicked() {
                                        self.apply_slash(block_id, *kind);
                                    }
                                }
                            });
                    });
            });
    }

    fn filtered_slash_items(&self, block_id: u64) -> Vec<BlockKind> {
        let filter = self
            .blocks
            .iter()
            .find(|b| b.id == block_id)
            .map(|b| b.text.trim_start_matches('/').trim().to_lowercase())
            .unwrap_or_default();
        BlockKind::slash_menu_items()
            .iter()
            .filter(|k| {
                filter.is_empty()
                    || k.label().to_lowercase().contains(&filter)
                    || k.keywords()
                        .iter()
                        .any(|kw| kw.contains(&filter) || filter.contains(kw))
            })
            .copied()
            .collect()
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
