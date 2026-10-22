//! Note graph: notes as nodes, `[[wikilinks]]` and local markdown links as edges.

use crate::vault::{NoteMeta, Vault};
use std::collections::{HashMap, HashSet};

pub struct GraphNode {
    pub path: String,
    pub title: String,
}

pub fn build(vault: &Vault, notes: &[NoteMeta]) -> (Vec<GraphNode>, Vec<[usize; 2]>) {
    let nodes: Vec<GraphNode> = notes
        .iter()
        .map(|n| GraphNode {
            path: n.path.to_string_lossy().to_string(),
            title: n.title.clone(),
        })
        .collect();

    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (i, note) in notes.iter().enumerate() {
        by_key.insert(key(&note.title), i);
        by_key.insert(key(note.rel.trim_end_matches(".md")), i);
        by_key.insert(key(&note.rel), i);
    }

    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        let Ok(content) = vault.read_note(&note.path) else {
            continue;
        };
        for link in extract_links(&content) {
            let Some(&j) = by_key.get(&key(&link)) else {
                continue;
            };
            if j == i {
                continue;
            }
            let e = if i < j { [i, j] } else { [j, i] };
            if seen.insert(e) {
                edges.push(e);
            }
        }
    }
    (nodes, edges)
}

fn key(s: &str) -> String {
    s.trim()
        .trim_end_matches(".md")
        .trim_end()
        .to_lowercase()
}

/// Extract link targets from markdown: `[[wikilinks]]` and `[text](relative.md)`.
pub fn extract_links(content: &str) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut links = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' && chars.get(i + 1) == Some(&'[') {
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

#[cfg(test)]
mod tests {
    use super::extract_links;

    #[test]
    fn wiki_and_markdown_links() {
        let links = extract_links(
            "See [[Welcome]] and [[Other|display]] and [[Note#heading]] and [a](./notes/Foo.md) and [web](https://example.com).",
        );
        assert_eq!(
            links,
            vec![
                "Welcome".to_string(),
                "Other".to_string(),
                "Note".to_string(),
                "notes/Foo.md".to_string(),
            ]
        );
    }
}
