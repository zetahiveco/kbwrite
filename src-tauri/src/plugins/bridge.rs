//! Callbacks a loaded plugin uses to change the vault and register commands.

use crate::commands::ensure_inside;
use crate::vault::Vault;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::commands::VaultSlot;

pub struct Session {
    pub id: String,
    pub vault: Arc<Mutex<VaultSlot>>,
    pub app: AppHandle,
    pub commands: Mutex<Vec<(String, String)>>,
    last_error: Mutex<String>,
}

impl Session {
    pub fn new(id: String, vault: Arc<Mutex<VaultSlot>>, app: AppHandle) -> Self {
        Self {
            id,
            vault,
            app,
            commands: Mutex::new(Vec::new()),
            last_error: Mutex::new(String::new()),
        }
    }

    pub fn error_message(&self) -> String {
        self.last_error
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    fn fail(&self, message: impl Into<String>) -> *mut c_char {
        let message = message.into();
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = message;
        }
        std::ptr::null_mut()
    }

    fn ok_string(&self, text: &str) -> *mut c_char {
        if let Ok(mut slot) = self.last_error.lock() {
            slot.clear();
        }
        owned_c_string(text)
    }
}

fn session<'a>(user: *mut std::ffi::c_void) -> &'a Session {
    unsafe { &*(user as *const Session) }
}

fn arg(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("Missing text".into());
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map(|text| text.to_string())
        .map_err(|_| "Text was not valid UTF-8".into())
}

fn owned_c_string(text: &str) -> *mut c_char {
    CString::new(text.replace('\0', ""))
        .map(|value| value.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

pub unsafe extern "C" fn last_error(user: *mut std::ffi::c_void) -> *mut c_char {
    let session = session(user);
    let text = session
        .last_error
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();
    owned_c_string(&text)
}

pub unsafe extern "C" fn vault_path(user: *mut std::ffi::c_void) -> *mut c_char {
    let session = session(user);
    let Ok(slot) = session.vault.lock() else {
        return session.fail("Vault is busy");
    };
    match slot.vault.as_ref() {
        Some(vault) => session.ok_string(&vault.root.to_string_lossy()),
        None => session.fail("No vault open"),
    }
}

pub unsafe extern "C" fn list_notes(user: *mut std::ffi::c_void) -> *mut c_char {
    let session = session(user);
    let Ok(mut slot) = session.vault.lock() else {
        return session.fail("Vault is busy");
    };
    let Some(vault) = slot.vault.as_mut() else {
        return session.fail("No vault open");
    };
    let (notes, _) = vault.load();
    let text = notes
        .iter()
        .map(|note| note.rel.clone())
        .collect::<Vec<_>>()
        .join("\n");
    session.ok_string(&text)
}

pub unsafe extern "C" fn read_note(user: *mut std::ffi::c_void, path: *const c_char) -> *mut c_char {
    let session = session(user);
    let path = match arg(path) {
        Ok(path) => path,
        Err(error) => return session.fail(error),
    };
    let Ok(slot) = session.vault.lock() else {
        return session.fail("Vault is busy");
    };
    let Some(vault) = slot.vault.as_ref() else {
        return session.fail("No vault open");
    };
    match resolve_note(vault, &path).and_then(|full| vault.read_note(&full).map_err(|e| e.to_string())) {
        Ok(text) => session.ok_string(&text),
        Err(error) => session.fail(error),
    }
}

pub unsafe extern "C" fn write_note(
    user: *mut std::ffi::c_void,
    path: *const c_char,
    content: *const c_char,
) -> i32 {
    let session = session(user);
    let (path, content) = match (arg(path), arg(content)) {
        (Ok(path), Ok(content)) => (path, content),
        (Err(error), _) | (_, Err(error)) => {
            session.fail(error);
            return -1;
        }
    };
    let result = {
        let Ok(slot) = session.vault.lock() else {
            session.fail("Vault is busy");
            return -1;
        };
        let Some(vault) = slot.vault.as_ref() else {
            session.fail("No vault open");
            return -1;
        };
        resolve_note(vault, &path).and_then(|full| vault.write_note(&full, &content).map_err(|e| e.to_string()))
    };
    finish_write(session, result)
}

pub unsafe extern "C" fn create_note(
    user: *mut std::ffi::c_void,
    title: *const c_char,
    content: *const c_char,
) -> *mut c_char {
    let session = session(user);
    let (title, content) = match (arg(title), arg(content)) {
        (Ok(title), Ok(content)) => (title, content),
        (Err(error), _) | (_, Err(error)) => return session.fail(error),
    };
    let created = {
        let Ok(slot) = session.vault.lock() else {
            return session.fail("Vault is busy");
        };
        let Some(vault) = slot.vault.as_ref() else {
            return session.fail("No vault open");
        };
        vault.create_note(&title, &content).map_err(|e| e.to_string())
    };
    match created {
        Ok(note) => {
            let _ = session.app.emit("vault-changed", ());
            session.ok_string(&note.rel)
        }
        Err(error) => session.fail(error),
    }
}

pub unsafe extern "C" fn delete_note(user: *mut std::ffi::c_void, path: *const c_char) -> i32 {
    let session = session(user);
    let path = match arg(path) {
        Ok(path) => path,
        Err(error) => {
            session.fail(error);
            return -1;
        }
    };
    let result = {
        let Ok(slot) = session.vault.lock() else {
            session.fail("Vault is busy");
            return -1;
        };
        let Some(vault) = slot.vault.as_ref() else {
            session.fail("No vault open");
            return -1;
        };
        resolve_note(vault, &path).and_then(|full| vault.delete_path(&full).map_err(|e| e.to_string()))
    };
    finish_write(session, result)
}

pub unsafe extern "C" fn log(user: *mut std::ffi::c_void, message: *const c_char) {
    let session = session(user);
    let Ok(message) = arg(message) else {
        return;
    };
    let text = format!("{}: {message}", session.id);
    eprintln!("[kbwrite] plugin {text}");
    let _ = session.app.emit("plugin-notice", text);
}

pub unsafe extern "C" fn register_command(
    user: *mut std::ffi::c_void,
    id: *const c_char,
    name: *const c_char,
) -> i32 {
    let session = session(user);
    let (id, name) = match (arg(id), arg(name)) {
        (Ok(id), Ok(name)) => (id, name),
        (Err(error), _) | (_, Err(error)) => {
            session.fail(error);
            return -1;
        }
    };
    let id = id.trim().to_string();
    let name = name.trim().to_string();
    if id.is_empty() || name.is_empty() || id.contains('\n') || name.contains('\n') {
        session.fail("Command id and name are required");
        return -1;
    }
    if let Ok(mut commands) = session.commands.lock() {
        if let Some(existing) = commands.iter_mut().find(|(command_id, _)| command_id == &id) {
            existing.1 = name;
        } else {
            commands.push((id, name));
        }
    }
    0
}

fn finish_write(session: &Session, result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => {
            if let Ok(mut slot) = session.last_error.lock() {
                slot.clear();
            }
            let _ = session.app.emit("vault-changed", ());
            0
        }
        Err(error) => {
            session.fail(error);
            -1
        }
    }
}

fn resolve_note(vault: &Vault, input: &str) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter a note path".into());
    }
    let given = PathBuf::from(input);
    let path = if given.is_absolute() {
        given
    } else {
        let joined = vault.root.join(&given);
        if joined.extension().is_none() && !joined.exists() {
            let mut with_md = joined.clone();
            with_md.set_extension("md");
            with_md
        } else {
            joined
        }
    };
    ensure_inside(&vault.root, &path)?;
    Ok(path)
}

pub fn host_fns(session: &Session) -> kbwrite_plugin::HostFns {
    kbwrite_plugin::HostFns {
        abi: kbwrite_plugin::ABI_VERSION,
        user: session as *const Session as *mut std::ffi::c_void,
        free_string,
        last_error,
        vault_path,
        list_notes,
        read_note,
        write_note,
        create_note,
        delete_note,
        log,
        register_command,
    }
}
