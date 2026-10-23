//! Rust plugins. Each installed plugin is a folder under `~/.kbwrite/plugins`
//! with a `manifest.toml` and a dynamic library built against `kbwrite-plugin`.

mod bridge;
mod host;
mod install;
mod manifest;

pub use host::PluginHost;
use tauri::{AppHandle, Manager, State};

use crate::commands::AppState;

pub fn attach(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(mut plugins) = state.plugins.lock() else {
        return;
    };
    plugins.attach(app.clone(), state.vault.clone());
    plugins.reload();
}

#[tauri::command]
pub fn list_plugins(state: State<'_, AppState>) -> Result<Vec<host::PluginDto>, String> {
    let plugins = state.plugins.lock().map_err(|e| e.to_string())?;
    Ok(plugins.list())
}

#[tauri::command]
pub async fn install_plugin_folder(app: AppHandle, path: String) -> Result<Vec<host::PluginDto>, String> {
    let prepared = tauri::async_runtime::spawn_blocking(move || install::prepare_folder(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())??;
    finish_install(&app, &prepared)
}

#[tauri::command]
pub async fn install_plugin_github(app: AppHandle, url: String) -> Result<Vec<host::PluginDto>, String> {
    let url = url.trim().to_string();
    let prepared = tauri::async_runtime::spawn_blocking(move || install::prepare_github(&url))
        .await
        .map_err(|e| e.to_string())??;
    finish_install(&app, &prepared)
}

fn finish_install(app: &AppHandle, prepared: &install::Prepared) -> Result<Vec<host::PluginDto>, String> {
    let state = app.state::<AppState>();
    let mut plugins = state.plugins.lock().map_err(|e| e.to_string())?;
    plugins.unload_all();
    let committed = install::commit(prepared);
    plugins.reload();
    committed?;
    Ok(plugins.list())
}

#[tauri::command]
pub fn set_plugin_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<Vec<host::PluginDto>, String> {
    let mut plugins = state.plugins.lock().map_err(|e| e.to_string())?;
    plugins.set_enabled(&id, enabled)?;
    Ok(plugins.list())
}

#[tauri::command]
pub fn remove_plugin(state: State<'_, AppState>, id: String) -> Result<Vec<host::PluginDto>, String> {
    let mut plugins = state.plugins.lock().map_err(|e| e.to_string())?;
    plugins.remove(&id)?;
    Ok(plugins.list())
}

#[tauri::command]
pub fn run_plugin_command(
    state: State<'_, AppState>,
    plugin_id: String,
    command_id: String,
) -> Result<(), String> {
    let mut plugins = state.plugins.lock().map_err(|e| e.to_string())?;
    plugins.run(&plugin_id, &command_id)
}

pub fn empty_host() -> PluginHost {
    PluginHost::default()
}
