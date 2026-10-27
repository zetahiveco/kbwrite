//! Settings persistence (vault path and appearance).
//!
//! The on-disk shape stays snake_case so existing `~/.kbwrite/settings.json`
//! files keep working.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: PathBuf,
    #[serde(default = "default_true")]
    pub dark_mode: bool,
    #[serde(default)]
    pub ui_font: String,
    #[serde(default)]
    pub editor_font: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default)]
    pub custom_css: String,
    #[serde(default)]
    pub auto_format: bool,
    #[serde(default)]
    pub bookmarks: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_font_size() -> u32 {
    16
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vault_path: default_vault_path(),
            dark_mode: true,
            ui_font: String::new(),
            editor_font: String::new(),
            font_size: default_font_size(),
            custom_css: String::new(),
            auto_format: false,
            bookmarks: Vec::new(),
        }
    }
}

pub(crate) fn config_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".kbwrite")
}

fn config_file() -> PathBuf {
    config_dir().join("settings.json")
}

fn default_vault_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents").join("KbWriteVault")
}

impl Settings {
    pub fn load() -> Self {
        match std::fs::read_to_string(config_file()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir)?;
        std::fs::write(config_file(), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
