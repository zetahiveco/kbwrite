//! Load installed plugins and call into them.

use super::bridge::{host_fns, Session};
use super::install::plugins_dir;
use super::manifest::Manifest;
use crate::commands::VaultSlot;
use crate::settings::config_dir;
use kbwrite_plugin::{PluginVTable, ABI_VERSION};
use libloading::Library;
use serde::{Deserialize, Serialize};
use std::ffi::{c_void, CString};
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

pub struct PluginHost {
    app: Option<AppHandle>,
    vault: Option<Arc<Mutex<VaultSlot>>>,
    loaded: Vec<LoadedPlugin>,
    catalog: Vec<PluginDto>,
}

struct LoadedPlugin {
    id: String,
    _library: Library,
    vtable: PluginVTable,
    instance: *mut c_void,
    session: Box<Session>,
    fns: Box<kbwrite_plugin::HostFns>,
}

unsafe impl Send for LoadedPlugin {}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        if self.instance.is_null() {
            return;
        }
        let instance = self.instance;
        self.instance = std::ptr::null_mut();
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            (self.vtable.unload)(&*self.fns, instance);
        }));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandDto {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub enabled: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub commands: Vec<PluginCommandDto>,
}

#[derive(Default, Serialize, Deserialize)]
struct PluginState {
    #[serde(default)]
    disabled: Vec<String>,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self {
            app: None,
            vault: None,
            loaded: Vec::new(),
            catalog: Vec::new(),
        }
    }
}

impl PluginHost {
    pub fn attach(&mut self, app: AppHandle, vault: Arc<Mutex<VaultSlot>>) {
        self.app = Some(app);
        self.vault = Some(vault);
    }

    pub fn list(&self) -> Vec<PluginDto> {
        self.catalog.clone()
    }

    pub fn reload(&mut self) {
        self.loaded.clear();
        let disabled = read_state().disabled;
        let mut catalog = Vec::new();
        let mut dirs = Vec::new();
        if let Ok(entries) = fs::read_dir(plugins_dir()) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
        dirs.sort();
        for path in dirs {
            let manifest = match Manifest::read(&path) {
                Ok(manifest) => manifest,
                Err(error) => {
                    catalog.push(broken_plugin(&path, error));
                    continue;
                }
            };
            let enabled = !disabled.iter().any(|id| id == &manifest.id);
            if !enabled {
                catalog.push(dto(manifest, false, false, None, Vec::new()));
                continue;
            }
            match self.load_dir(&path, &manifest) {
                Ok(commands) => catalog.push(dto(manifest, true, true, None, commands)),
                Err(error) => catalog.push(dto(manifest, true, false, Some(error), Vec::new())),
            }
        }
        self.catalog = catalog;
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        if !self.catalog.iter().any(|plugin| plugin.id == id) {
            return Err(format!("No plugin named {id}"));
        }
        let mut state = read_state();
        if enabled {
            state.disabled.retain(|item| item != id);
        } else if !state.disabled.iter().any(|item| item == id) {
            state.disabled.push(id.to_string());
        }
        write_state(&state)?;
        self.reload();
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        if !self.catalog.iter().any(|plugin| plugin.id == id) {
            return Err(format!("No plugin named {id}"));
        }
        self.loaded.retain(|plugin| plugin.id != id);
        let path = plugins_dir().join(id);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| format!("Could not remove {id}: {e}"))?;
        }
        let mut state = read_state();
        state.disabled.retain(|item| item != id);
        write_state(&state)?;
        self.reload();
        Ok(())
    }

    pub fn unload_all(&mut self) {
        self.loaded.clear();
    }

    pub fn run(&mut self, plugin_id: &str, command_id: &str) -> Result<(), String> {
        let plugin = self
            .loaded
            .iter_mut()
            .find(|plugin| plugin.id == plugin_id)
            .ok_or_else(|| format!("{plugin_id} is not loaded"))?;
        let command = CString::new(command_id).map_err(|_| "Command id is invalid".to_string())?;
        let code = unsafe { (plugin.vtable.command)(&*plugin.fns, plugin.instance, command.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            let message = plugin.session.error_message();
            if message.is_empty() {
                Err(format!("Command {command_id} failed"))
            } else {
                Err(message)
            }
        }
    }

    fn load_dir(&mut self, dir: &Path, manifest: &Manifest) -> Result<Vec<PluginCommandDto>, String> {
        let app = self.app.clone().ok_or("KbWrite is still starting")?;
        let vault = self.vault.clone().ok_or("KbWrite is still starting")?;
        let library_path = super::manifest::library_path(dir, &manifest.entry);
        if !library_path.is_file() {
            return Err(format!("Missing {}", library_path.display()));
        }
        let library = unsafe { Library::new(&library_path) }
            .map_err(|e| format!("Could not open {}: {e}", library_path.display()))?;
        let vtable = {
            let symbol = unsafe {
                library.get::<unsafe extern "C" fn() -> PluginVTable>(b"kbwrite_plugin_vtable\0")
            }
            .map_err(|_| "The library does not export kbwrite_plugin_vtable".to_string())?;
            unsafe { symbol() }
        };
        if vtable.abi != ABI_VERSION {
            return Err(format!(
                "Plugin ABI is {}, KbWrite expects {ABI_VERSION}",
                vtable.abi
            ));
        }
        let session = Box::new(Session::new(manifest.id.clone(), vault, app));
        let fns = Box::new(host_fns(&session));
        let instance = unsafe { (vtable.load)(&*fns) };
        if instance.is_null() {
            return Err("The plugin failed while loading".into());
        }
        let commands = session
            .commands
            .lock()
            .map(|commands| {
                commands
                    .iter()
                    .map(|(id, name)| PluginCommandDto {
                        id: id.clone(),
                        name: name.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.loaded.push(LoadedPlugin {
            id: manifest.id.clone(),
            _library: library,
            vtable,
            instance,
            session,
            fns,
        });
        Ok(commands)
    }
}

fn dto(
    manifest: Manifest,
    enabled: bool,
    loaded: bool,
    error: Option<String>,
    commands: Vec<PluginCommandDto>,
) -> PluginDto {
    PluginDto {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        description: manifest.description,
        authors: manifest.authors,
        enabled,
        loaded,
        error,
        commands,
    }
}

fn broken_plugin(path: &Path, error: String) -> PluginDto {
    let id = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "plugin".into());
    PluginDto {
        name: id.clone(),
        id,
        version: String::new(),
        description: String::new(),
        authors: String::new(),
        enabled: false,
        loaded: false,
        error: Some(error),
        commands: Vec::new(),
    }
}

fn state_path() -> std::path::PathBuf {
    config_dir().join("plugins.json")
}

fn read_state() -> PluginState {
    let Ok(text) = fs::read_to_string(state_path()) else {
        return PluginState::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_state(state: &PluginState) -> Result<(), String> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, serde_json::to_string_pretty(state).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
