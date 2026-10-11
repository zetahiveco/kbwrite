//! Settings persistence (vault path + OpenAI-compatible inline AI config).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: PathBuf,
    /// Base URL of an OpenAI-compatible API, e.g. https://api.openai.com/v1
    pub openai_base_url: String,
    pub openai_model: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default = "default_true")]
    pub dark_mode: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vault_path: default_vault_path(),
            openai_base_url: "https://api.openai.com/v1".to_string(),
            openai_model: "gpt-4o-mini".to_string(),
            openai_api_key: String::new(),
            dark_mode: true,
        }
    }
}

fn config_dir() -> PathBuf {
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
    PathBuf::from(home)
        .join("Documents")
        .join("KbWriteVault")
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
