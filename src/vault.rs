//! Vault: a folder of markdown notes on disk.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub struct Vault {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct NoteMeta {
    pub path: PathBuf,
    /// Path relative to the vault root, used for display.
    pub rel: String,
    pub title: String,
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Recursively list all `.md` notes in the vault.
    pub fn list_notes(&self) -> Vec<NoteMeta> {
        let mut notes = Vec::new();
        self.walk(&self.root.clone(), &mut notes);
        notes.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
        notes
    }

    fn walk(&self, dir: &Path, notes: &mut Vec<NoteMeta>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue; // hidden files / folders
            }
            if path.is_dir() {
                self.walk(&path, notes);
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                let rel = path
                    .strip_prefix(&self.root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| name.to_string());
                notes.push(NoteMeta { path, rel, title });
            }
        }
    }

    pub fn read_note(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn write_note(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::write(path, content)?)
    }

    pub fn create_note(&self, title: &str, content: &str) -> Result<NoteMeta> {
        let mut slug: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        slug = slug.trim().replace(' ', "-");
        if slug.is_empty() {
            slug = "untitled".to_string();
        }

        let mut path = self.root.join(format!("{slug}.md"));
        let mut n = 1;
        while path.exists() {
            n += 1;
            path = self.root.join(format!("{slug}-{n}.md"));
        }
        std::fs::write(&path, content)?;
        let rel = path
            .strip_prefix(&self.root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let title = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        Ok(NoteMeta { path, rel, title })
    }

    pub fn delete_note(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.root) {
            return Err(anyhow!("Refusing to delete outside of the vault"));
        }
        Ok(std::fs::remove_file(path)?)
    }
}
