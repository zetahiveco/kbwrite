//! `manifest.toml` required in every plugin folder.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    /// Stored so the settings list can show which app version the plugin needs.
    #[allow(dead_code)]
    pub min_app_version: String,
    /// Library stem. `hello_plugin` becomes `libhello_plugin.dylib` on macOS.
    pub entry: String,
}

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    authors: AuthorField,
    #[serde(default)]
    min_app_version: String,
    #[serde(default)]
    entry: String,
}

#[derive(Default, Deserialize)]
#[serde(untagged)]
enum AuthorField {
    #[default]
    Empty,
    One(String),
    Many(Vec<String>),
}

impl Manifest {
    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join("manifest.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|_| format!("No manifest.toml in {}", dir.display()))?;
        let raw: RawManifest = toml::from_str(&text).map_err(|e| format!("manifest.toml: {e}"))?;
        let id = raw.id.trim().to_string();
        if !valid_id(&id) {
            return Err(
                "Plugin id must be 1–64 characters of letters, numbers, hyphens, or underscores"
                    .into(),
            );
        }
        let authors = match raw.authors {
            AuthorField::Many(list) if !list.is_empty() => list.join(", "),
            AuthorField::One(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => raw.author.trim().to_string(),
        };
        let entry = {
            let entry = raw.entry.trim();
            if entry.is_empty() {
                id.replace('-', "_")
            } else {
                entry.to_string()
            }
        };
        let version = raw.version.trim().to_string();
        if version.is_empty() {
            return Err("manifest.toml needs a version".into());
        }
        let name = raw.name.trim().to_string();
        if name.is_empty() {
            return Err("manifest.toml needs a name".into());
        }
        if !version_allows(env!("CARGO_PKG_VERSION"), &raw.min_app_version) {
            return Err(format!(
                "{} needs KbWrite {}, this is {}",
                name,
                raw.min_app_version.trim(),
                env!("CARGO_PKG_VERSION")
            ));
        }
        Ok(Self {
            id,
            name,
            version,
            description: raw.description.trim().to_string(),
            authors,
            min_app_version: raw.min_app_version.trim().to_string(),
            entry,
        })
    }
}

pub fn library_filename(entry: &str) -> String {
    let path = Path::new(entry);
    if path.extension().is_some() {
        return entry.to_string();
    }
    if cfg!(windows) {
        format!("{entry}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{entry}.dylib")
    } else {
        format!("lib{entry}.so")
    }
}

pub fn library_path(dir: &Path, entry: &str) -> PathBuf {
    dir.join(library_filename(entry))
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    id.len() <= 64 && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// True when the running app is at least `required` (`major.minor.patch`).
fn version_allows(app: &str, required: &str) -> bool {
    let required = required.trim();
    if required.is_empty() {
        return true;
    }
    parse_version(app) >= parse_version(required)
}

fn parse_version(text: &str) -> (u64, u64, u64) {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(version_allows("0.5.0", "0.5.0"));
        assert!(version_allows("0.6.0", "0.5.1"));
        assert!(!version_allows("0.5.0", "0.6.0"));
        assert!(version_allows("0.5.0", ""));
    }

    #[test]
    fn library_name_uses_the_platform() {
        let name = library_filename("hello_plugin");
        if cfg!(target_os = "macos") {
            assert_eq!(name, "libhello_plugin.dylib");
        } else if cfg!(windows) {
            assert_eq!(name, "hello_plugin.dll");
        } else {
            assert_eq!(name, "libhello_plugin.so");
        }
        assert_eq!(library_filename("already.so"), "already.so");
    }
}
