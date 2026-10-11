//! Block-based document model with markdown persistence.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Bullet,
    Numbered,
    Todo,
    Quote,
    Code,
    Divider,
}

impl BlockKind {
    pub fn label(&self) -> &'static str {
        match self {
            BlockKind::Paragraph => "Text",
            BlockKind::Heading1 => "Heading 1",
            BlockKind::Heading2 => "Heading 2",
            BlockKind::Heading3 => "Heading 3",
            BlockKind::Bullet => "Bullet list",
            BlockKind::Numbered => "Numbered list",
            BlockKind::Todo => "To-do list",
            BlockKind::Quote => "Quote",
            BlockKind::Code => "Code",
            BlockKind::Divider => "Divider",
        }
    }

    /// Icon name in the Phosphor icon set (kebab-case), used via `Icon::from_name`.
    pub fn icon_name(&self) -> &'static str {
        match self {
            BlockKind::Paragraph => "text-aa",
            BlockKind::Heading1 => "text-h-one",
            BlockKind::Heading2 => "text-h-two",
            BlockKind::Heading3 => "text-h-three",
            BlockKind::Bullet => "list-dashes",
            BlockKind::Numbered => "list-numbers",
            BlockKind::Todo => "check-square",
            BlockKind::Quote => "quotes",
            BlockKind::Code => "code",
            BlockKind::Divider => "minus",
        }
    }

    /// All block types offered in the "/" slash menu, in order.
    pub fn slash_menu_items() -> &'static [BlockKind] {
        &[
            BlockKind::Paragraph,
            BlockKind::Heading1,
            BlockKind::Heading2,
            BlockKind::Heading3,
            BlockKind::Bullet,
            BlockKind::Numbered,
            BlockKind::Todo,
            BlockKind::Quote,
            BlockKind::Code,
            BlockKind::Divider,
        ]
    }

    /// Keywords used to filter the slash menu (e.g. typing "/head").
    pub fn keywords(&self) -> &'static [&'static str] {
        match self {
            BlockKind::Paragraph => &["text", "paragraph", "plain"],
            BlockKind::Heading1 => &["heading", "title", "h1"],
            BlockKind::Heading2 => &["heading", "h2"],
            BlockKind::Heading3 => &["heading", "h3"],
            BlockKind::Bullet => &["bullet", "list", "unordered"],
            BlockKind::Numbered => &["number", "list", "ordered"],
            BlockKind::Todo => &["todo", "task", "check", "checkbox"],
            BlockKind::Quote => &["quote", "blockquote"],
            BlockKind::Code => &["code", "snippet"],
            BlockKind::Divider => &["divider", "separator", "hr", "line"],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: u64,
    pub kind: BlockKind,
    pub text: String,
    /// Used by `Todo` blocks.
    pub checked: bool,
}

impl Block {
    pub fn new(id: u64, kind: BlockKind) -> Self {
        Self {
            id,
            kind,
            text: String::new(),
            checked: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Markdown (de)serialization — line based, matching the block model.
// ---------------------------------------------------------------------------

pub fn serialize(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block.kind {
            BlockKind::Paragraph => out.push_str(&block.text),
            BlockKind::Heading1 => out.push_str(&format!("# {}", block.text)),
            BlockKind::Heading2 => out.push_str(&format!("## {}", block.text)),
            BlockKind::Heading3 => out.push_str(&format!("### {}", block.text)),
            BlockKind::Bullet => out.push_str(&format!("- {}", block.text)),
            BlockKind::Todo => {
                out.push_str(&format!(
                    "- [{}] {}",
                    if block.checked { "x" } else { " " },
                    block.text
                ));
            }
            BlockKind::Numbered => out.push_str(&format!("1. {}", block.text)),
            BlockKind::Quote => out.push_str(&format!("> {}", block.text)),
            BlockKind::Code => {
                out.push_str("```\n");
                out.push_str(&block.text);
                out.push_str("\n```");
            }
            BlockKind::Divider => out.push_str("---"),
        }
        out.push_str("\n\n");
    }
    out.trim_end().to_string() + "\n"
}

pub fn parse(content: &str) -> Vec<Block> {
    fn push(blocks: &mut Vec<Block>, next_id: &mut u64, kind: BlockKind, text: String) {
        // Skip blank separator lines.
        if !blocks.is_empty() && kind == BlockKind::Paragraph && text.trim().is_empty() {
            return;
        }
        let mut b = Block::new(*next_id, kind);
        b.text = text;
        blocks.push(b);
        *next_id += 1;
    }

    let mut blocks = Vec::new();
    let mut next_id = 1u64;

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if trimmed == "---" {
            push(&mut blocks, &mut next_id, BlockKind::Divider, String::new());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            push(&mut blocks, &mut next_id, BlockKind::Heading3, rest.to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            push(&mut blocks, &mut next_id, BlockKind::Heading2, rest.to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            push(&mut blocks, &mut next_id, BlockKind::Heading1, rest.to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [x] ").or(trimmed.strip_prefix("- [X] ")) {
            let mut b = Block::new(next_id, BlockKind::Todo);
            b.text = rest.to_string();
            b.checked = true;
            blocks.push(b);
            next_id += 1;
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            let mut b = Block::new(next_id, BlockKind::Todo);
            b.text = rest.to_string();
            blocks.push(b);
            next_id += 1;
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ").or(trimmed.strip_prefix("* ")) {
            push(&mut blocks, &mut next_id, BlockKind::Bullet, rest.to_string());
            i += 1;
            continue;
        }
        if trimmed.starts_with("1. ") {
            // Consume consecutive "N. " lines as one numbered list? Keep 1:1 with lines.
            let rest = trimmed.trim_start_matches(|c: char| c.is_ascii_digit());
            let rest = rest.strip_prefix(". ").unwrap_or(rest);
            push(&mut blocks, &mut next_id, BlockKind::Numbered, rest.to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            push(&mut blocks, &mut next_id, BlockKind::Quote, rest.to_string());
            i += 1;
            continue;
        }
        if trimmed == "```" {
            // Code fence: gather until closing fence.
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && lines[i].trim() != "```" {
                code.push(lines[i]);
                i += 1;
            }
            i += 1; // skip closing fence (or EOF)
            push(&mut blocks, &mut next_id, BlockKind::Code, code.join("\n"));
            continue;
        }

        // Regular paragraph line.
        push(&mut blocks, &mut next_id, BlockKind::Paragraph, line.to_string());
        i += 1;
    }

    if blocks.is_empty() {
        blocks.push(Block::new(next_id, BlockKind::Paragraph));
    }
    blocks
}
