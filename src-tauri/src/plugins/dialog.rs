//! Dialogs and block specs a plugin asks the app to show.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogField {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogSpec {
    pub title: String,
    pub message: String,
    pub fields: Vec<DialogField>,
    pub confirm: String,
    pub cancel: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockProp {
    pub id: String,
    pub label: String,
    pub default: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSpec {
    #[serde(rename = "type")]
    pub type_name: String,
    pub title: String,
    pub description: String,
    pub group: String,
    pub props: Vec<BlockProp>,
}

pub struct DialogHub {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<String>>>,
}

impl Default for DialogHub {
    fn default() -> Self {
        Self {
            next: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
        }
    }
}

impl DialogHub {
    pub fn open(&self, response_to: impl FnOnce(u64)) -> Result<String, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| "Dialogs are busy".to_string())?
            .insert(id, tx);
        response_to(id);
        match rx.recv_timeout(Duration::from_secs(30 * 60)) {
            Ok(body) => Ok(body),
            Err(_) => {
                self.finish(id);
                Err("The dialog was closed before it was answered".into())
            }
        }
    }

    pub fn resolve(&self, id: u64, body: String) -> Result<(), String> {
        if body.len() > 64 * 1024 {
            return Err("Dialog response is too large".into());
        }
        let sender = self
            .pending
            .lock()
            .map_err(|_| "Dialogs are busy".to_string())?
            .remove(&id);
        let Some(sender) = sender else {
            return Err("That dialog is no longer open".into());
        };
        sender
            .send(body)
            .map_err(|_| "The plugin stopped waiting for this dialog".to_string())
    }

    fn finish(&self, id: u64) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
    }
}

pub fn parse_dialog(text: &str) -> Result<DialogSpec, String> {
    if text.len() > 32 * 1024 {
        return Err("Dialog spec is too large".into());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| format!("Dialog JSON: {e}"))?;
    let title = clean_line(value.get("title").and_then(Value::as_str).unwrap_or(""), 120);
    if title.is_empty() {
        return Err("Dialog needs a title".into());
    }
    let message = clean_text(value.get("message").and_then(Value::as_str).unwrap_or(""), 2000);
    let confirm = label_or(value.get("confirm"), "OK");
    let cancel = label_or(value.get("cancel"), "Cancel");
    let raw_fields = value.get("fields").and_then(Value::as_array);
    let mut fields = Vec::new();
    if let Some(raw_fields) = raw_fields {
        if raw_fields.len() > 12 {
            return Err("A dialog can have at most 12 fields".into());
        }
        for field in raw_fields {
            let id = field.get("id").and_then(Value::as_str).unwrap_or("").trim();
            if !ident(id, 32) {
                return Err("Each dialog field needs an id of letters, numbers, or underscores".into());
            }
            if fields.iter().any(|existing: &DialogField| existing.id == id) {
                return Err(format!("Dialog field {id} is listed twice"));
            }
            let label = clean_line(field.get("label").and_then(Value::as_str).unwrap_or(id), 80);
            let kind = field.get("kind").and_then(Value::as_str).unwrap_or("text").trim();
            if kind != "text" && kind != "password" && kind != "checkbox" {
                return Err("Dialog field kind must be text, password, or checkbox".into());
            }
            let value = match kind {
                "checkbox" => {
                    let raw = field.get("value").and_then(Value::as_str).unwrap_or("false");
                    if raw == "true" { "true" } else { "false" }.to_string()
                }
                _ => clean_text(field.get("value").and_then(Value::as_str).unwrap_or(""), 4000),
            };
            fields.push(DialogField {
                id: id.to_string(),
                label,
                kind: kind.to_string(),
                value,
            });
        }
    }
    Ok(DialogSpec {
        title,
        message,
        fields,
        confirm,
        cancel,
    })
}

pub fn parse_block(plugin_id: &str, text: &str) -> Result<BlockSpec, String> {
    if text.len() > 16 * 1024 {
        return Err("Block spec is too large".into());
    }
    let value: Value = serde_json::from_str(text).map_err(|e| format!("Block JSON: {e}"))?;
    let id = value.get("id").and_then(Value::as_str).unwrap_or("").trim();
    if !ident(id, 32) {
        return Err("Block id must be letters, numbers, or underscores".into());
    }
    let title = clean_line(value.get("title").and_then(Value::as_str).unwrap_or(""), 80);
    if title.is_empty() {
        return Err("Block needs a title".into());
    }
    let description = clean_line(
        value.get("description").and_then(Value::as_str).unwrap_or(""),
        200,
    );
    let group = {
        let group = clean_line(value.get("group").and_then(Value::as_str).unwrap_or(""), 40);
        if group.is_empty() { "Plugins".into() } else { group }
    };
    let mut props = Vec::new();
    if let Some(raw) = value.get("props").and_then(Value::as_array) {
        if raw.len() > 8 {
            return Err("A block can have at most 8 fields".into());
        }
        for prop in raw {
            let prop_id = prop.get("id").and_then(Value::as_str).unwrap_or("").trim();
            if !ident(prop_id, 32) {
                return Err("Each block field needs an id of letters, numbers, or underscores".into());
            }
            if props.iter().any(|existing: &BlockProp| existing.id == prop_id) {
                return Err(format!("Block field {prop_id} is listed twice"));
            }
            let label = clean_line(prop.get("label").and_then(Value::as_str).unwrap_or(prop_id), 80);
            let default = clean_text(prop.get("default").and_then(Value::as_str).unwrap_or(""), 4000);
            props.push(BlockProp {
                id: prop_id.to_string(),
                label,
                default,
            });
        }
    }
    Ok(BlockSpec {
        type_name: format!("plugin_{plugin_id}_{id}"),
        title,
        description,
        group,
        props,
    })
}

fn label_or(value: Option<&Value>, fallback: &str) -> String {
    let text = clean_line(value.and_then(Value::as_str).unwrap_or(""), 40);
    if text.is_empty() { fallback.into() } else { text }
}

fn clean_line(text: &str, max: usize) -> String {
    let text = text.trim().replace(['\n', '\r'], " ");
    clip(&text, max)
}

fn clean_text(text: &str, max: usize) -> String {
    clip(text.trim(), max)
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
}

fn ident(value: &str, max: usize) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    value.len() <= max && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_requires_a_title_and_keeps_fields() {
        let spec = parse_dialog(
            r#"{"title":"Sync","fields":[{"id":"repo","label":"Repository","kind":"text","value":"owner/notes"},{"id":"token","label":"Token","kind":"password"},{"id":"push","label":"Push","kind":"checkbox","value":"true"}]}"#,
        )
        .unwrap();
        assert_eq!(spec.title, "Sync");
        assert_eq!(spec.fields.len(), 3);
        assert_eq!(spec.fields[2].value, "true");
        assert_eq!(spec.confirm, "OK");
    }

    #[test]
    fn block_type_is_namespaced() {
        let spec = parse_block(
            "flashcards",
            r#"{"id":"card","title":"Flashcard","props":[{"id":"front","label":"Front"},{"id":"back","label":"Back","default":""}]}"#,
        )
        .unwrap();
        assert_eq!(spec.type_name, "plugin_flashcards_card");
        assert_eq!(spec.group, "Plugins");
        assert_eq!(spec.props.len(), 2);
    }
}
