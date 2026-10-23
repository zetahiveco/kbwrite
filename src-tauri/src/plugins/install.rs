//! Install a plugin from a local folder or a GitHub repository.
//!
//! The folder that contains `manifest.toml` is the plugin. If the compiled
//! library is missing and `Cargo.toml` is present, KbWrite runs
//! `cargo build --release` there, then copies the library next to the manifest
//! into `~/.kbwrite/plugins/<id>/`.

use super::manifest::{library_filename, library_path, Manifest};
use crate::settings::config_dir;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn plugins_dir() -> PathBuf {
    config_dir().join("plugins")
}

pub struct Prepared {
    pub manifest: Manifest,
    pub source: PathBuf,
    pub library: PathBuf,
    cleanup: Option<PathBuf>,
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Some(path) = self.cleanup.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub fn prepare_folder(source: &Path) -> Result<Prepared, String> {
    let source = source
        .canonicalize()
        .map_err(|e| format!("Cannot open {}: {e}", source.display()))?;
    if !source.is_dir() {
        return Err("Choose the plugin folder".into());
    }
    prepare_dir(source, None)
}

pub fn prepare_github(url: &str) -> Result<Prepared, String> {
    let spec = parse_github(url).ok_or_else(|| {
        "Enter a GitHub URL such as https://github.com/owner/repo".to_string()
    })?;
    let bytes = download_repo(&spec)?;
    let temp = std::env::temp_dir().join(format!(
        "kbwrite-plugin-{}-{}",
        std::process::id(),
        spec.repo
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let extracted = match extract_plugin_dir(&temp, &bytes, spec.subdir.as_deref()) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
    };
    match prepare_dir(extracted, Some(temp)) {
        Ok(prepared) => Ok(prepared),
        Err(error) => Err(error),
    }
}

fn prepare_dir(source: PathBuf, cleanup: Option<PathBuf>) -> Result<Prepared, String> {
    let manifest = match Manifest::read(&source) {
        Ok(manifest) => manifest,
        Err(error) => {
            if let Some(path) = &cleanup {
                let _ = fs::remove_dir_all(path);
            }
            return Err(error);
        }
    };
    let library = match ensure_library(&source, &manifest.entry) {
        Ok(library) => library,
        Err(error) => {
            if let Some(path) = &cleanup {
                let _ = fs::remove_dir_all(path);
            }
            return Err(error);
        }
    };
    Ok(Prepared {
        manifest,
        source,
        library,
        cleanup,
    })
}

pub fn commit(prepared: &Prepared) -> Result<(), String> {
    let dest = plugins_dir().join(&prepared.manifest.id);
    if dest.exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Could not replace {}: {e}", prepared.manifest.id))?;
    }
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    fs::copy(
        prepared.source.join("manifest.toml"),
        dest.join("manifest.toml"),
    )
    .map_err(|e| format!("Could not copy manifest.toml: {e}"))?;
    fs::copy(
        &prepared.library,
        dest.join(library_filename(&prepared.manifest.entry)),
    )
    .map_err(|e| format!("Could not copy the plugin library: {e}"))?;
    Ok(())
}

fn ensure_library(source: &Path, entry: &str) -> Result<PathBuf, String> {
    let ready = library_path(source, entry);
    if ready.is_file() {
        return Ok(ready);
    }
    let built = source.join("target").join("release").join(library_filename(entry));
    if built.is_file() {
        return Ok(built);
    }
    if !source.join("Cargo.toml").is_file() {
        return Err(format!(
            "Missing {} and there is no Cargo.toml to build",
            library_filename(entry)
        ));
    }
    build_release(source)?;
    if built.is_file() {
        Ok(built)
    } else {
        Err(format!(
            "cargo build finished but {} was not produced. Set entry in manifest.toml to the library name.",
            library_filename(entry)
        ))
    }
}

fn build_release(dir: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(dir.join("target"))
        .current_dir(dir)
        .status()
        .map_err(|e| {
            format!("Could not run cargo ({cargo}): {e}. Install Rust, or put the built library in the plugin folder.")
        })?;
    if status.success() {
        Ok(())
    } else {
        Err("cargo build --release failed".into())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct GitHubRepo {
    owner: String,
    repo: String,
    branch: String,
    subdir: Option<String>,
}

fn parse_github(input: &str) -> Option<GitHubRepo> {
    let input = input.trim().trim_end_matches('/').trim_end_matches(".git");
    if input.is_empty() {
        return None;
    }
    let path = if let Some(rest) = input.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = input.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = input.strip_prefix("github.com/") {
        rest
    } else if !input.contains("://") && input.matches('/').count() == 1 {
        input
    } else {
        return None;
    };
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() || owner.contains('.') {
        return None;
    }
    let mut branch = "main".to_string();
    let mut subdir = None;
    if parts.next() == Some("tree") {
        branch = parts.next()?.to_string();
        let rest = parts.collect::<Vec<_>>().join("/");
        if !rest.is_empty() {
            subdir = Some(rest);
        }
    }
    Some(GitHubRepo {
        owner,
        repo,
        branch,
        subdir,
    })
}

fn download_repo(spec: &GitHubRepo) -> Result<Vec<u8>, String> {
    let branches = if spec.branch == "main" {
        vec!["main".to_string(), "master".to_string()]
    } else {
        vec![spec.branch.clone()]
    };
    let mut last_error = String::new();
    for branch in branches {
        let url = format!(
            "https://codeload.github.com/{}/{}/zip/refs/heads/{branch}",
            spec.owner, spec.repo
        );
        match download(&url) {
            Ok(bytes) => return Ok(bytes),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let response = ureq::get(url)
        .set("User-Agent", concat!("kbwrite/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| format!("Could not download {url}: {e}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(50 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Could not read the download: {e}"))?;
    if bytes.len() >= 50 * 1024 * 1024 {
        return Err("The repository download is larger than 50 MB".into());
    }
    Ok(bytes)
}

fn extract_plugin_dir(root: &Path, bytes: &[u8], subdir: Option<&str>) -> Result<PathBuf, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("The download was not a zip archive: {e}"))?;
    let mut manifests = Vec::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|e| format!("Could not read the archive: {e}"))?;
        let name = file.name().replace('\\', "/");
        if name.ends_with("/manifest.toml") || name.ends_with("manifest.toml") {
            if !name.split('/').any(|part| part == ".." || part == ".git") {
                manifests.push(name);
            }
        }
    }
    if let Some(subdir) = subdir {
        let needle = format!("/{subdir}/manifest.toml");
        manifests.retain(|name| name.ends_with(&needle) || name.ends_with(&format!("{subdir}/manifest.toml")));
    }
    manifests.sort_by_key(|name| name.matches('/').count());
    let manifest_name = manifests
        .into_iter()
        .next()
        .ok_or_else(|| "The repository has no manifest.toml".to_string())?;
    let plugin_prefix = manifest_name
        .trim_end_matches("manifest.toml")
        .trim_end_matches('/');
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("Could not read the archive: {e}"))?;
        let name = file.name().replace('\\', "/");
        if name.split('/').any(|part| part == "..") {
            continue;
        }
        let relative = if plugin_prefix.is_empty() {
            name.as_str()
        } else if let Some(rest) = name.strip_prefix(plugin_prefix) {
            rest.trim_start_matches('/')
        } else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        let dest = root.join(relative);
        if file.is_dir() || name.ends_with('/') {
            fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_urls() {
        assert_eq!(
            parse_github("https://github.com/zetahiveco/kbwrite"),
            Some(GitHubRepo {
                owner: "zetahiveco".into(),
                repo: "kbwrite".into(),
                branch: "main".into(),
                subdir: None,
            })
        );
        assert_eq!(
            parse_github("https://github.com/owner/plugin.git"),
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "plugin".into(),
                branch: "main".into(),
                subdir: None,
            })
        );
        assert_eq!(
            parse_github("https://github.com/owner/plugin/tree/dev/plugins/hello"),
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "plugin".into(),
                branch: "dev".into(),
                subdir: Some("plugins/hello".into()),
            })
        );
        assert_eq!(
            parse_github("owner/plugin"),
            Some(GitHubRepo {
                owner: "owner".into(),
                repo: "plugin".into(),
                branch: "main".into(),
                subdir: None,
            })
        );
        assert!(parse_github("https://example.com/owner/plugin").is_none());
    }
}
