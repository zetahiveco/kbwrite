//! Vault: a folder of markdown notes on disk.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub struct Vault {
    pub root: PathBuf,
    /// Errors encountered while listing notes (e.g. permission denied by sandbox/TCC).
    pub walk_errors: Vec<String>,
    /// True when the OS blocked access to the notes folder (macOS TCC privacy
    /// protection). Shown as an actionable "grant access" state in the UI.
    pub permission_denied: bool,
}

#[derive(Debug, Clone)]
pub struct NoteMeta {
    pub path: PathBuf,
    /// Path relative to the vault root, used for display.
    pub rel: String,
    pub title: String,
}

/// A node in the vault's file tree (folder or `.md` note).
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    /// Absolute path.
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let probe = root.join(".kbwrite-write-test");
        let mut permission_denied = false;
        match std::fs::write(&probe, b"ok").map(|_| std::fs::remove_file(&probe)) {
            Ok(_) => {}
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    permission_denied = true;
                }
                eprintln!(
                    "[kbwrite] WARNING: vault at {} is not writable: {e}",
                    root.display()
                );
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            walk_errors: Vec::new(),
            permission_denied,
        })
    }

    /// Walk the vault once and return notes plus the folder tree.
    pub fn load(&mut self) -> (Vec<NoteMeta>, Vec<TreeNode>) {
        self.walk_errors.clear();
        self.permission_denied = false;
        let root = self.root.clone();
        let mut tree = Vec::new();
        self.build_tree(&root, &mut tree);
        let mut notes = Vec::new();
        flatten(&tree, &self.root, &mut notes);
        notes.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
        eprintln!(
            "[kbwrite] vault {}: {} notes found{}",
            self.root.display(),
            notes.len(),
            if self.walk_errors.is_empty() {
                String::new()
            } else {
                format!(" (errors: {:?})", self.walk_errors)
            }
        );
        (notes, tree)
    }

    fn build_tree(&mut self, dir: &Path, out: &mut Vec<TreeNode>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    self.permission_denied = true;
                }
                self.walk_errors
                    .push(format!("cannot read {}: {e}", dir.display()));
                return;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                let mut children = Vec::new();
                self.build_tree(&path, &mut children);
                out.push(TreeNode {
                    name,
                    path,
                    is_dir: true,
                    children,
                });
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                out.push(TreeNode {
                    name,
                    path,
                    is_dir: false,
                    children: Vec::new(),
                });
            }
        }
        out.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
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
        let title = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Ok(NoteMeta { path, rel, title })
    }

    /// Create an empty file, failing if it already exists.
    pub fn create_file(&self, path: &Path, content: &str) -> Result<()> {
        if path.exists() {
            return Err(anyhow!("“{}” already exists", path.display()));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::write(path, content)?)
    }

    /// Create a folder (and any missing parents), failing if it already exists.
    pub fn create_folder(&self, path: &Path) -> Result<()> {
        if path.exists() {
            return Err(anyhow!("“{}” already exists", path.display()));
        }
        Ok(std::fs::create_dir_all(path)?)
    }

    /// Rename/move a file or folder inside the vault, failing if the target exists.
    pub fn rename_path(&self, from: &Path, to: &Path) -> Result<()> {
        if !from.starts_with(&self.root) || !to.starts_with(&self.root) {
            return Err(anyhow!("Refusing to operate outside of the vault"));
        }
        if to.exists() {
            return Err(anyhow!("“{}” already exists", to.display()));
        }
        Ok(std::fs::rename(from, to)?)
    }

    /// Delete a file or folder (recursively) inside the vault.
    pub fn delete_path(&self, path: &Path) -> Result<()> {
        if !path.starts_with(&self.root) {
            return Err(anyhow!("Refusing to delete outside of the vault"));
        }
        if path.is_dir() {
            Ok(std::fs::remove_dir_all(path)?)
        } else {
            Ok(std::fs::remove_file(path)?)
        }
    }
}

fn flatten(nodes: &[TreeNode], root: &Path, out: &mut Vec<NoteMeta>) {
    for n in nodes {
        if n.is_dir {
            flatten(&n.children, root, out);
        } else {
            let rel = n
                .path
                .strip_prefix(root)
                .unwrap_or(&n.path)
                .to_string_lossy()
                .to_string();
            let title = n
                .path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| n.name.clone());
            out.push(NoteMeta {
                path: n.path.clone(),
                rel,
                title,
            });
        }
    }
}
