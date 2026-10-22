//! Tauri commands. The webview never touches the filesystem itself.

use crate::settings::Settings;
use crate::vault::{NoteMeta, TreeNode, Vault};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

const WELCOME: &str = include_str!("welcome.md");

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub vault: Mutex<VaultSlot>,
}

pub struct VaultSlot {
    pub vault: Option<Vault>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub vault_path: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_api_key: String,
    pub dark_mode: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub path: String,
    pub rel: String,
    pub title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNodeDto {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<TreeNodeDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: SettingsDto,
    pub notes: Vec<NoteDto>,
    pub tree: Vec<TreeNodeDto>,
    pub walk_errors: Vec<String>,
    pub permission_denied: bool,
    pub vault_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mutation {
    pub path: String,
    pub snapshot: Snapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeDto {
    pub path: String,
    pub title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<[usize; 2]>,
}

pub fn open_vault(path: &Path) -> (Option<Vault>, Option<String>) {
    match Vault::open(path) {
        Ok(mut vault) => {
            let (notes, _) = vault.load();
            if notes.is_empty() {
                if let Err(e) = vault.create_note("Welcome", WELCOME) {
                    return (
                        Some(vault),
                        Some(format!("Could not create the Welcome note: {e:#}")),
                    );
                }
            }
            (Some(vault), None)
        }
        Err(e) => (None, Some(format!("Vault error: {e:#}"))),
    }
}

fn settings_dto(settings: &Settings) -> SettingsDto {
    SettingsDto {
        vault_path: settings.vault_path.to_string_lossy().to_string(),
        openai_base_url: settings.openai_base_url.clone(),
        openai_model: settings.openai_model.clone(),
        openai_api_key: settings.openai_api_key.clone(),
        dark_mode: settings.dark_mode,
    }
}

fn note_dto(note: &NoteMeta) -> NoteDto {
    NoteDto {
        path: note.path.to_string_lossy().to_string(),
        rel: note.rel.clone(),
        title: note.title.clone(),
    }
}

fn tree_dto(node: &TreeNode) -> TreeNodeDto {
    TreeNodeDto {
        name: node.name.clone(),
        path: node.path.to_string_lossy().to_string(),
        is_dir: node.is_dir,
        children: node.children.iter().map(tree_dto).collect(),
    }
}

fn snapshot(settings: &Settings, slot: &mut VaultSlot) -> Snapshot {
    let (notes, tree, walk_errors, permission_denied) = if let Some(vault) = slot.vault.as_mut() {
        let (notes, tree) = vault.load();
        (
            notes.iter().map(note_dto).collect(),
            tree.iter().map(tree_dto).collect(),
            vault.walk_errors.clone(),
            vault.permission_denied,
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new(), false)
    };
    Snapshot {
        settings: settings_dto(settings),
        notes,
        tree,
        walk_errors,
        permission_denied,
        vault_error: slot.error.clone(),
    }
}

fn lock_settings(state: &AppState) -> Result<std::sync::MutexGuard<'_, Settings>, String> {
    state.settings.lock().map_err(|e| e.to_string())
}

fn lock_vault(state: &AppState) -> Result<std::sync::MutexGuard<'_, VaultSlot>, String> {
    state.vault.lock().map_err(|e| e.to_string())
}

fn vault_mut<'a>(
    slot: &'a mut VaultSlot,
) -> Result<&'a mut Vault, String> {
    slot.vault.as_mut().ok_or_else(|| {
        slot.error
            .clone()
            .unwrap_or_else(|| "No vault open".to_string())
    })
}

/// Canonicalize `path` (or its parent, if it does not exist yet) and refuse
/// anything that escapes the vault root, including through symlinks.
fn ensure_inside(root: &Path, path: &Path) -> Result<(), String> {
    let root = std::fs::canonicalize(root).map_err(|e| format!("vault path: {e}"))?;
    let check = if path.exists() {
        std::fs::canonicalize(path).map_err(|e| e.to_string())?
    } else {
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let parent = std::fs::canonicalize(parent)
            .map_err(|e| format!("cannot use {}: {e}", parent.display()))?;
        parent.join(path.file_name().unwrap_or_default())
    };
    if check.starts_with(&root) {
        Ok(())
    } else {
        Err("Refusing to operate outside of the vault".into())
    }
}

fn clean_name(name: &str, is_dir: bool) -> Result<String, String> {
    let name = name.trim().trim_matches(|c| c == '/' || c == '\\');
    let name = name
        .split(['/', '\\'])
        .next_back()
        .unwrap_or("")
        .trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err("Enter a name".into());
    }
    if name.starts_with('.') {
        return Err("Names cannot start with a dot".into());
    }
    let mut name = name.to_string();
    if !is_dir && !name.to_lowercase().ends_with(".md") {
        name.push_str(".md");
    }
    Ok(name)
}

fn child_path(root: &Path, dir: &Path, name: &str, is_dir: bool) -> Result<PathBuf, String> {
    let name = clean_name(name, is_dir)?;
    ensure_inside(root, dir)?;
    let path = dir.join(&name);
    ensure_inside(root, &path)?;
    Ok(path)
}

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let settings = lock_settings(&state)?;
    let mut slot = lock_vault(&state)?;
    Ok(snapshot(&settings, &mut slot))
}

#[tauri::command]
pub fn reload_vault(state: State<'_, AppState>, path: String) -> Result<Snapshot, String> {
    let mut settings = lock_settings(&state)?;
    let path = path.trim();
    if path.is_empty() {
        return Err("Vault path is empty".into());
    }
    settings.vault_path = PathBuf::from(path);
    settings.save().map_err(|e| format!("{e:#}"))?;
    let (vault, error) = open_vault(&settings.vault_path);
    let mut slot = lock_vault(&state)?;
    *slot = VaultSlot { vault, error };
    Ok(snapshot(&settings, &mut slot))
}

#[tauri::command]
pub fn read_note(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let settings = lock_settings(&state)?;
    let slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let vault = slot.vault.as_ref().ok_or_else(|| {
        slot.error
            .clone()
            .unwrap_or_else(|| "No vault open".to_string())
    })?;
    let path = PathBuf::from(path);
    ensure_inside(&root, &path)?;
    vault.read_note(&path).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn write_note(state: State<'_, AppState>, path: String, content: String) -> Result<(), String> {
    let settings = lock_settings(&state)?;
    let slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let vault = slot.vault.as_ref().ok_or_else(|| {
        slot.error
            .clone()
            .unwrap_or_else(|| "No vault open".to_string())
    })?;
    let path = PathBuf::from(path);
    ensure_inside(&root, &path)?;
    if path.extension().map(|e| e != "md").unwrap_or(true) {
        return Err("Only markdown notes can be saved".into());
    }
    vault.write_note(&path, &content).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn create_file(state: State<'_, AppState>, dir: String, name: String) -> Result<Mutation, String> {
    let settings = lock_settings(&state)?;
    let mut slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let path = child_path(&root, Path::new(&dir), &name, false)?;
    let path_string = path.to_string_lossy().to_string();
    vault_mut(&mut slot)?
        .create_file(&path, "")
        .map_err(|e| format!("{e:#}"))?;
    Ok(Mutation {
        path: path_string,
        snapshot: snapshot(&settings, &mut slot),
    })
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    dir: String,
    name: String,
) -> Result<Mutation, String> {
    let settings = lock_settings(&state)?;
    let mut slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let path = child_path(&root, Path::new(&dir), &name, true)?;
    let path_string = path.to_string_lossy().to_string();
    vault_mut(&mut slot)?
        .create_folder(&path)
        .map_err(|e| format!("{e:#}"))?;
    Ok(Mutation {
        path: path_string,
        snapshot: snapshot(&settings, &mut slot),
    })
}

#[tauri::command]
pub fn rename_path(state: State<'_, AppState>, from: String, name: String) -> Result<Mutation, String> {
    let settings = lock_settings(&state)?;
    let mut slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let from = PathBuf::from(from);
    ensure_inside(&root, &from)?;
    let is_dir = from.is_dir();
    let parent = from
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.clone());
    let to = child_path(&root, &parent, &name, is_dir)?;
    let path_string = to.to_string_lossy().to_string();
    if to != from {
        vault_mut(&mut slot)?
            .rename_path(&from, &to)
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(Mutation {
        path: path_string,
        snapshot: snapshot(&settings, &mut slot),
    })
}

#[tauri::command]
pub fn delete_path(state: State<'_, AppState>, path: String) -> Result<Snapshot, String> {
    let settings = lock_settings(&state)?;
    let mut slot = lock_vault(&state)?;
    let root = settings.vault_path.clone();
    let path = PathBuf::from(path);
    ensure_inside(&root, &path)?;
    {
        let vault = vault_mut(&mut slot)?;
        vault.delete_path(&path).map_err(|e| format!("{e:#}"))?;
        let (notes, _) = vault.load();
        if notes.is_empty() {
            vault
                .create_note("Welcome", WELCOME)
                .map_err(|e| format!("{e:#}"))?;
        }
    }
    Ok(snapshot(&settings, &mut slot))
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: SettingsDto) -> Result<Snapshot, String> {
    let mut current = lock_settings(&state)?;
    let next_path = PathBuf::from(settings.vault_path.trim());
    if next_path.as_os_str().is_empty() {
        return Err("Vault path is empty".into());
    }
    let path_changed = next_path != current.vault_path;
    current.vault_path = next_path;
    current.openai_base_url = settings.openai_base_url.trim().to_string();
    current.openai_model = settings.openai_model.trim().to_string();
    current.openai_api_key = settings.openai_api_key.trim().to_string();
    current.dark_mode = settings.dark_mode;
    if current.openai_base_url.is_empty() {
        current.openai_base_url = "https://api.openai.com/v1".into();
    }
    if current.openai_model.is_empty() {
        current.openai_model = "gpt-4o-mini".into();
    }
    current.save().map_err(|e| format!("{e:#}"))?;

    let mut slot = lock_vault(&state)?;
    if path_changed {
        let (vault, error) = open_vault(&current.vault_path);
        *slot = VaultSlot { vault, error };
    }
    Ok(snapshot(&current, &mut slot))
}

#[tauri::command]
pub async fn ai_complete(
    state: State<'_, AppState>,
    instruction: String,
    text: String,
) -> Result<String, String> {
    let settings = lock_settings(&state)?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::ai::complete(&settings, &instruction, &text).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn graph(state: State<'_, AppState>) -> Result<GraphDto, String> {
    let mut slot = lock_vault(&state)?;
    let vault = vault_mut(&mut slot)?;
    let (notes, _) = vault.load();
    let (nodes, edges) = crate::graph::build(vault, &notes);
    Ok(GraphDto {
        nodes: nodes
            .into_iter()
            .map(|n| GraphNodeDto {
                path: n.path,
                title: n.title,
            })
            .collect(),
        edges,
    })
}

#[tauri::command]
pub fn open_privacy_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_DocumentsFolder")
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("System privacy settings are only available on macOS.".into())
    }
}
