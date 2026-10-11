//! Obsidian-style graph explorer: notes as nodes, `[[links]]` as edges.

use crate::vault::{NoteMeta, Vault};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Sense, Ui, Vec2,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// Force-directed layout tuning.
const REPULSION: f32 = 120_000.0;
const SPRING_K: f32 = 0.045;
const REST_LEN: f32 = 120.0;
const CENTER_PULL: f32 = 0.008;
const DAMPING: f32 = 0.82;
const STEPS: usize = 3;
const MAX_SPEED: f32 = 24.0;

pub struct GraphExplorer {
    nodes: Vec<GraphNode>,
    edges: Vec<(usize, usize)>,
    /// Number of links per node (drives the node radius).
    degree: Vec<usize>,
    /// Layout-space positions (screen = center + offset + pos * zoom).
    pos: Vec<Vec2>,
    vel: Vec<Vec2>,
    offset: Vec2,
    zoom: f32,
    /// Notes version the graph was built from (rebuild when it changes).
    version: u64,
}

struct GraphNode {
    path: PathBuf,
    title: String,
}

impl GraphExplorer {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            degree: Vec::new(),
            pos: Vec::new(),
            vel: Vec::new(),
            offset: Vec2::ZERO,
            zoom: 1.0,
            version: u64::MAX,
        }
    }

    /// (notes, links) currently in the graph, for the toolbar hint.
    pub fn stats(&self) -> (usize, usize) {
        (self.nodes.len(), self.edges.len())
    }

    /// Draw the graph. Returns the path of a node the user clicked to open.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        vault: &Vault,
        notes: &[NoteMeta],
        version: u64,
        dark: bool,
        current: Option<&Path>,
    ) -> Option<PathBuf> {
        if version != self.version {
            self.rebuild(vault, notes);
            self.version = version;
        }

        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let center = rect.center();

        // --- Pan ---
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // --- Zoom around the pointer ---
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let new_zoom = (self.zoom * (1.0 + scroll * 0.0015)).clamp(0.15, 4.0);
            if let Some(p) = response.hover_pos() {
                // Keep the graph point under the cursor fixed while zooming.
                let gp = (p - center - self.offset) / self.zoom;
                self.offset = p - center - gp * new_zoom;
            }
            self.zoom = new_zoom;
        }

        // --- Physics ---
        self.simulate();

        // --- Hover highlight ---
        let hovered = response.hover_pos().and_then(|p| self.node_at(p, center));

        // --- Draw ---
        let painter = ui.painter_at(rect);
        let to_screen = |p: Vec2| center + self.offset + p * self.zoom;

        let edge_col = if dark {
            Color32::from_gray(75)
        } else {
            Color32::from_gray(195)
        };
        let hi_col = accent(dark);
        for &(a, b) in &self.edges {
            let sa = to_screen(self.pos[a]);
            let sb = to_screen(self.pos[b]);
            let lit = hovered.map(|h| h == a || h == b).unwrap_or(false);
            painter.line_segment(
                [sa, sb],
                egui::Stroke::new(if lit { 1.8 } else { 1.0 }, if lit { hi_col } else { edge_col }),
            );
        }

        let node_col = if dark {
            Color32::from_rgb(120, 155, 215)
        } else {
            Color32::from_rgb(90, 125, 195)
        };
        for (i, node) in self.nodes.iter().enumerate() {
            let s = to_screen(self.pos[i]);
            let r = (4.5 + self.degree[i] as f32 * 0.9).min(14.0) * self.zoom.max(0.5);
            let is_cur = current.map(|c| c == node.path.as_path()).unwrap_or(false);
            let hot = is_cur || hovered == Some(i);
            let fill = if hot { hi_col } else { node_col };
            if hot {
                painter.circle_filled(s, r + 3.5, fill.gamma_multiply(0.35));
            }
            painter.circle_filled(s, r, fill);
            if self.zoom >= 0.55 || hot {
                painter.text(
                    s + Vec2::new(0.0, r + 10.0),
                    Align2::CENTER_CENTER,
                    &node.title,
                    FontId::proportional(11.0),
                    if dark {
                        Color32::from_gray(170)
                    } else {
                        Color32::from_gray(80)
                    },
                );
            }
        }

        if self.nodes.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No notes to graph yet — create some notes and link them with [[links]].",
                FontId::proportional(14.0),
                if dark {
                    Color32::from_gray(130)
                } else {
                    Color32::from_gray(120)
                },
            );
        }

        // --- Click a node → open the note ---
        if response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                if let Some(i) = self.node_at(p, center) {
                    return Some(self.nodes[i].path.clone());
                }
            }
        }
        None
    }

    /// Nearest node under the pointer (within hit radius), if any.
    fn node_at(&self, p: Pos2, center: Pos2) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, pos) in self.pos.iter().enumerate() {
            let s = center + self.offset + *pos * self.zoom;
            let d = s.distance(p);
            if d < 14.0 && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Run a few force-directed simulation steps (repulsion + springs + pull to center).
    fn simulate(&mut self) {
        let n = self.pos.len();
        if n == 0 {
            return;
        }
        for _ in 0..STEPS {
            let mut fx = vec![0.0f32; n];
            let mut fy = vec![0.0f32; n];

            // Node-node repulsion.
            for i in 0..n {
                for j in (i + 1)..n {
                    let dx = self.pos[i].x - self.pos[j].x;
                    let dy = self.pos[i].y - self.pos[j].y;
                    let d2 = (dx * dx + dy * dy).max(120.0);
                    let d = d2.sqrt();
                    let f = REPULSION / d2;
                    let (ux, uy) = (dx / d, dy / d);
                    fx[i] += ux * f;
                    fy[i] += uy * f;
                    fx[j] -= ux * f;
                    fy[j] -= uy * f;
                }
            }

            // Edge springs.
            for &(a, b) in &self.edges {
                let dx = self.pos[b].x - self.pos[a].x;
                let dy = self.pos[b].y - self.pos[a].y;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let f = (d - REST_LEN) * SPRING_K;
                let (ux, uy) = (dx / d, dy / d);
                fx[a] += ux * f;
                fy[a] += uy * f;
                fx[b] -= ux * f;
                fy[b] -= uy * f;
            }

            // Integrate (with a gentle pull toward the center).
            let mut max_sp = 0.0f32;
            for i in 0..n {
                fx[i] -= self.pos[i].x * CENTER_PULL;
                fy[i] -= self.pos[i].y * CENTER_PULL;
                self.vel[i].x = (self.vel[i].x + fx[i]) * DAMPING;
                self.vel[i].y = (self.vel[i].y + fy[i]) * DAMPING;
                let sp = self.vel[i].length();
                max_sp = max_sp.max(sp);
                if sp > MAX_SPEED {
                    self.vel[i] = self.vel[i] / sp * MAX_SPEED;
                }
                self.pos[i] += self.vel[i];
            }
            if max_sp < 0.05 {
                break; // layout settled
            }
        }
    }

    /// (Re)build nodes + edges from the vault, keeping existing node positions.
    fn rebuild(&mut self, vault: &Vault, notes: &[NoteMeta]) {
        let old_pos: HashMap<PathBuf, Vec2> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.path.clone(), self.pos[i]))
            .collect();

        self.nodes = notes
            .iter()
            .map(|n| GraphNode {
                path: n.path.clone(),
                title: n.title.clone(),
            })
            .collect();
        let n = self.nodes.len();
        self.pos = (0..n)
            .map(|i| {
                old_pos.get(&self.nodes[i].path).copied().unwrap_or_else(|| {
                    // Seed new nodes on a golden-angle spiral so they spread out.
                    let a = i as f32 * 2.399963 + 1.0;
                    let r = 30.0 * ((i + 1) as f32).sqrt();
                    Vec2::new(a.cos() * r, a.sin() * r)
                })
            })
            .collect();
        self.vel = vec![Vec2::ZERO; n];

        // Resolve link targets: match by title or vault-relative path
        // (case-insensitive, ".md" optional) — like Obsidian's wikilinks.
        let mut by_key: HashMap<String, usize> = HashMap::new();
        for (i, note) in notes.iter().enumerate() {
            by_key.insert(key(&note.title), i);
            by_key.insert(key(note.rel.trim_end_matches(".md")), i);
            by_key.insert(key(&note.rel), i);
        }

        let mut seen = HashSet::new();
        self.edges.clear();
        self.degree = vec![0; n];
        for (i, note) in notes.iter().enumerate() {
            let Ok(content) = vault.read_note(&note.path) else {
                continue;
            };
            for link in extract_links(&content) {
                let Some(j) = by_key.get(&key(&link)).copied() else {
                    continue;
                };
                if j == i {
                    continue; // ignore self-links
                }
                let e = if i < j { (i, j) } else { (j, i) };
                if seen.insert(e) {
                    self.edges.push(e);
                    self.degree[e.0] += 1;
                    self.degree[e.1] += 1;
                }
            }
        }
    }
}

fn key(s: &str) -> String {
    // ".md" is optional in links: [[Note]] and [[Note.md]] both resolve.
    s.trim()
        .trim_end_matches(".md")
        .trim_end()
        .to_lowercase()
}

/// Extract link targets from markdown: `[[wikilinks]]` and `[text](relative.md)`.
fn extract_links(content: &str) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut links = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' && chars.get(i + 1) == Some(&'[') {
            // Wikilink: [[target|display]] / [[target#heading]]
            let mut j = i + 2;
            let mut buf = String::new();
            while j < chars.len() && !(chars[j] == ']' && chars.get(j + 1) == Some(&']')) {
                buf.push(chars[j]);
                j += 1;
            }
            if j < chars.len() {
                let target = buf
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !target.is_empty() {
                    links.push(target);
                }
                i = j + 2;
            } else {
                i += 1;
            }
        } else if chars[i] == '[' {
            // Markdown link: [text](target) — only local targets (skip http/mail).
            let mut j = i + 1;
            let mut depth = 1;
            while j < chars.len() && depth > 0 {
                match chars[j] {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            if depth == 0 && chars.get(j) == Some(&'(') {
                let mut k = j + 1;
                let mut target = String::new();
                while k < chars.len() && chars[k] != ')' {
                    target.push(chars[k]);
                    k += 1;
                }
                if k < chars.len() && !target.is_empty() {
                    let t = target.replace("%20", " ");
                    if !t.starts_with("http://")
                        && !t.starts_with("https://")
                        && !t.starts_with("mailto:")
                        && !t.contains("://")
                    {
                        links.push(t.trim_start_matches("./").trim().to_string());
                    }
                }
                i = k + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    links
}

fn accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(150, 190, 255)
    } else {
        Color32::from_rgb(40, 90, 200)
    }
}