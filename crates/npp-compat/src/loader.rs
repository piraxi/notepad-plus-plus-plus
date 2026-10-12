//! Windows DLL lifecycle for trusted, ABI-conforming Notepad++ plugins.
//! This does not implement the host's NPPM/Scintilla messages or docking UI.

use crate::plugin::*;
use std::{io, marker::PhantomData, os::windows::ffi::OsStrExt, path::Path, rc::Rc};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*};

struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: This owns one successful LoadLibraryExW reference. The loading
        // contract requires all plugin threads/callbacks to end before drop.
        unsafe {
            FreeLibrary(self.0);
        }
    }
}

#[derive(Clone, Debug)]
pub struct PluginMenuItem {
    pub name: String,
    pub initially_checked: bool,
    pub shortcut: Option<ShortcutKey>,
    pub is_separator: bool,
}

pub struct LoadedPlugin {
    _library: Library,
    name: String,
    menu: Vec<PluginMenuItem>,
    items: *mut FuncItem,
    callbacks: Vec<Option<PluginCommand>>,
    message_proc: MessageProc,
    be_notified: BeNotified,
    // Host and plugin UI callbacks must remain on the owning thread.
    _thread: PhantomData<Rc<()>>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl LoadedPlugin {
    /// Load a DLL, validate required exports, and initialize its command array.
    ///
    /// # Safety
    /// Loading executes native code, including DLL initialization and teardown.
    /// The caller must trust this plugin and its dependencies to conform to the
    /// supplied Notepad++ ABI. Returned pointers must be valid: the name must be
    /// NUL-terminated, the command array must be writable and remain alive until
    /// unload, and shortcut pointers must refer to valid ShortcutKey objects.
    /// Host handles and message handling must meet this plugin's requirements.
    /// The caller must stop all plugin threads, windows and callbacks before
    /// dropping this object (including when initialization returns an error).
    pub unsafe fn load(path: &Path, data: NppData) -> io::Result<Self> {
        let path = std::fs::canonicalize(path)?;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in plugin path",
            ));
        }
        wide.push(0);
        // SAFETY: The caller authorizes native execution; path is absolute and
        // NUL-terminated. Search dependencies in the DLL directory/default dirs.
        let handle = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let library = Library(handle);
        macro_rules! export {
            ($name:literal, $ty:ty) => {{
                // SAFETY: Required symbol is looked up on a live owned module;
                // caller guarantees any returned export has its declared ABI.
                let address = unsafe { GetProcAddress(handle, concat!($name, "\0").as_ptr()) }
                    .ok_or_else(|| invalid(concat!("Missing plugin export: ", $name)))?;
                unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(address) }
            }};
        }
        let is_unicode = export!("isUnicode", IsUnicode);
        let get_name = export!("getName", GetName);
        let set_info = export!("setInfo", SetInfo);
        let get_funcs = export!("getFuncsArray", GetFuncsArray);
        let message_proc = export!("messageProc", MessageProc);
        let be_notified = export!("beNotified", BeNotified);
        // SAFETY: All six signatures have been resolved under the caller's ABI contract.
        unsafe {
            if is_unicode() == 0 {
                return Err(invalid("ANSI plugins are not supported"));
            }
            let name_ptr = get_name();
            if name_ptr.is_null() {
                return Err(invalid("Plugin returned a null name"));
            }
            let mut length = 0;
            while length < 4096 && *name_ptr.add(length) != 0 {
                length += 1;
            }
            if length == 4096 {
                return Err(invalid("Plugin name exceeds 4095 UTF-16 units"));
            }
            let name = String::from_utf16(std::slice::from_raw_parts(name_ptr, length))
                .map_err(|_| invalid("Plugin name is invalid UTF-16"))?;
            set_info(data);
            let mut count = 0;
            let items = get_funcs(&mut count);
            if items.is_null() || !(1..=65535).contains(&count) {
                return Err(invalid("Plugin returned an invalid command array"));
            }
            let mut menu = Vec::with_capacity(count as usize);
            let mut callbacks = Vec::with_capacity(count as usize);
            for item in std::slice::from_raw_parts(items, count as usize) {
                let length = item
                    .item_name
                    .iter()
                    .position(|&c| c == 0)
                    .ok_or_else(|| invalid("Plugin command name is not terminated"))?;
                menu.push(PluginMenuItem {
                    name: String::from_utf16(&item.item_name[..length])
                        .map_err(|_| invalid("Plugin command name is invalid UTF-16"))?,
                    initially_checked: item.initially_checked,
                    shortcut: item.shortcut.as_ref().copied(),
                    is_separator: item.function.is_none(),
                });
                callbacks.push(item.function);
            }
            Ok(Self {
                _library: library,
                name,
                menu,
                items,
                callbacks,
                message_proc,
                be_notified,
                _thread: PhantomData,
            })
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn same_module(&self, other: &Self) -> bool {
        self._library.0 == other._library.0
    }
    pub fn menu_items(&self) -> &[PluginMenuItem] {
        &self.menu
    }

    /// Write the host-assigned command ID into the plugin-owned array.
    pub fn assign_command_id(&mut self, index: usize, id: i32) -> io::Result<()> {
        if index >= self.menu.len() {
            return Err(invalid("Plugin command index out of bounds"));
        }
        // SAFETY: Load contract retains a writable array of this length.
        unsafe {
            (*self.items.add(index)).command_id = id;
        }
        Ok(())
    }

    /// # Safety
    /// The host must meet this command's message/UI requirements and prevent
    /// unloading the plugin while it or a reentrant callback is executing.
    pub unsafe fn invoke(&self, index: usize) -> io::Result<bool> {
        let callback = self
            .callbacks
            .get(index)
            .ok_or_else(|| invalid("Plugin command index out of bounds"))?;
        if let Some(callback) = callback {
            unsafe {
                callback();
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// # Safety
    /// Message parameters must be valid for the plugin's protocol and callbacks
    /// must not outlive this plugin or its host handles.
    pub unsafe fn message(&self, message: u32, wparam: usize, lparam: isize) -> isize {
        unsafe { (self.message_proc)(message, wparam, lparam) }
    }

    /// # Safety
    /// Notification fields must follow the event's upstream contract, with any
    /// pointed-to data valid for the callback. The plugin must not retain it.
    pub unsafe fn notify(&self, notification: &mut ScNotification) {
        unsafe {
            (self.be_notified)(notification);
        }
    }
}
