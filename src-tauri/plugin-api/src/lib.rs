//! The KbWrite plugin API.
//!
//! Plugins are Rust dynamic libraries. They export [`export_plugin!`] and talk
//! to the app through [`Api`]. The layout of [`HostFns`] is the stable ABI:
//! the app and the plugin each compile this crate, and only these `repr(C)`
//! types cross the library boundary.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

pub const ABI_VERSION: u32 = 1;

/// Function pointers the app fills in before calling a plugin.
#[repr(C)]
pub struct HostFns {
    pub abi: u32,
    pub user: *mut c_void,
    pub free_string: unsafe extern "C" fn(*mut c_char),
    pub last_error: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
    pub vault_path: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
    pub list_notes: unsafe extern "C" fn(*mut c_void) -> *mut c_char,
    pub read_note: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub write_note: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> i32,
    pub create_note: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char,
    pub delete_note: unsafe extern "C" fn(*mut c_void, *const c_char) -> i32,
    pub log: unsafe extern "C" fn(*mut c_void, *const c_char),
    pub register_command: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> i32,
}

/// What a plugin exports as `kbwrite_plugin_vtable`.
#[repr(C)]
pub struct PluginVTable {
    pub abi: u32,
    pub load: unsafe extern "C" fn(*const HostFns) -> *mut c_void,
    pub unload: unsafe extern "C" fn(*const HostFns, *mut c_void),
    pub command: unsafe extern "C" fn(*const HostFns, *mut c_void, *const c_char) -> i32,
}

/// Safe wrapper around [`HostFns`].
pub struct Api {
    host: *const HostFns,
}

impl Api {
    /// # Safety
    /// `host` must point at a live [`HostFns`] for this call, or be null.
    pub unsafe fn from_raw(host: *const HostFns) -> Self {
        Self { host }
    }

    fn fns(&self) -> Option<&HostFns> {
        if self.host.is_null() {
            None
        } else {
            Some(unsafe { &*self.host })
        }
    }

    fn take(&self, ptr: *mut c_char) -> Result<String, String> {
        let Some(fns) = self.fns() else {
            return Err("Plugin API is not available".into());
        };
        if ptr.is_null() {
            return Err(self.error_text());
        }
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
        unsafe { (fns.free_string)(ptr) };
        Ok(text)
    }

    fn error_text(&self) -> String {
        let Some(fns) = self.fns() else {
            return "Plugin API is not available".into();
        };
        let ptr = unsafe { (fns.last_error)(fns.user) };
        if ptr.is_null() {
            return "Plugin call failed".into();
        }
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
        unsafe { (fns.free_string)(ptr) };
        if text.is_empty() {
            "Plugin call failed".into()
        } else {
            text
        }
    }

    fn pass(text: &str) -> Result<CString, String> {
        CString::new(text).map_err(|_| "Text contained a null byte".into())
    }

    /// Absolute path of the open vault.
    pub fn vault_path(&self) -> Result<String, String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        self.take(unsafe { (fns.vault_path)(fns.user) })
    }

    /// Relative paths of every markdown note, such as `Plan.md`.
    pub fn notes(&self) -> Result<Vec<String>, String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let text = self.take(unsafe { (fns.list_notes)(fns.user) })?;
        Ok(text.lines().filter(|line| !line.is_empty()).map(str::to_string).collect())
    }

    /// Read a note. `path` is relative to the vault (`Plan.md` or `Plan`) or absolute inside it.
    pub fn read_note(&self, path: &str) -> Result<String, String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let path = Self::pass(path)?;
        self.take(unsafe { (fns.read_note)(fns.user, path.as_ptr()) })
    }

    /// Replace a note's contents, creating parent folders when needed.
    pub fn write_note(&self, path: &str, content: &str) -> Result<(), String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let path = Self::pass(path)?;
        let content = Self::pass(content)?;
        let code = unsafe { (fns.write_note)(fns.user, path.as_ptr(), content.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            Err(self.error_text())
        }
    }

    /// Create a new note in the vault root. Returns its relative path.
    pub fn create_note(&self, title: &str, content: &str) -> Result<String, String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let title = Self::pass(title)?;
        let content = Self::pass(content)?;
        self.take(unsafe { (fns.create_note)(fns.user, title.as_ptr(), content.as_ptr()) })
    }

    /// Delete a note or folder inside the vault.
    pub fn delete_path(&self, path: &str) -> Result<(), String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let path = Self::pass(path)?;
        let code = unsafe { (fns.delete_note)(fns.user, path.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            Err(self.error_text())
        }
    }

    /// Show a short message in KbWrite.
    pub fn log(&self, message: &str) {
        let Some(fns) = self.fns() else {
            return;
        };
        let Ok(message) = Self::pass(message) else {
            return;
        };
        unsafe { (fns.log)(fns.user, message.as_ptr()) };
    }

    /// Add a command the user can run from Settings. `id` is what [`Plugin::command`] receives.
    pub fn register_command(&self, id: &str, name: &str) -> Result<(), String> {
        let fns = self.fns().ok_or("Plugin API is not available")?;
        let id = Self::pass(id)?;
        let name = Self::pass(name)?;
        let code = unsafe { (fns.register_command)(fns.user, id.as_ptr(), name.as_ptr()) };
        if code == 0 {
            Ok(())
        } else {
            Err(self.error_text())
        }
    }
}

/// Implemented by a plugin. Use [`export_plugin!`] to publish it.
pub trait Plugin: Send {
    fn load(&mut self, api: &Api);
    fn command(&mut self, _api: &Api, _id: &str) {}
    fn unload(&mut self, _api: &Api) {}
}

/// Export a [`Plugin`] that implements [`Default`].
///
/// The dynamic library must be the one named by `entry` in `manifest.toml`.
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        #[no_mangle]
        pub extern "C" fn kbwrite_plugin_vtable() -> $crate::PluginVTable {
            $crate::PluginVTable {
                abi: $crate::ABI_VERSION,
                load: plugin_load,
                unload: plugin_unload,
                command: plugin_command,
            }
        }

        unsafe extern "C" fn plugin_load(host: *const $crate::HostFns) -> *mut std::ffi::c_void {
            let loaded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut plugin = <$ty>::default();
                let api = unsafe { $crate::Api::from_raw(host) };
                plugin.load(&api);
                Box::into_raw(Box::new(plugin)) as *mut std::ffi::c_void
            }));
            loaded.unwrap_or(std::ptr::null_mut())
        }

        unsafe extern "C" fn plugin_unload(host: *const $crate::HostFns, instance: *mut std::ffi::c_void) {
            if instance.is_null() {
                return;
            }
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut plugin = unsafe { Box::from_raw(instance as *mut $ty) };
                let api = unsafe { $crate::Api::from_raw(host) };
                plugin.unload(&api);
            }));
        }

        unsafe extern "C" fn plugin_command(
            host: *const $crate::HostFns,
            instance: *mut std::ffi::c_void,
            id: *const std::os::raw::c_char,
        ) -> i32 {
            if instance.is_null() {
                return -1;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let plugin = unsafe { &mut *(instance as *mut $ty) };
                let api = unsafe { $crate::Api::from_raw(host) };
                let name = if id.is_null() {
                    String::new()
                } else {
                    unsafe { std::ffi::CStr::from_ptr(id) }.to_string_lossy().into_owned()
                };
                plugin.command(&api, &name);
            }));
            if result.is_err() {
                -1
            } else {
                0
            }
        }
    };
}
