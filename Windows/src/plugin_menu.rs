//! Build a complete replacement popup before changing the window's live menu.
use npp_compat::registry::PluginRegistry;
use std::io;
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};

struct Popup(HMENU);
impl Popup {
    fn new() -> io::Result<Self> {
        // SAFETY: Menu creation requires no caller-owned pointers.
        let handle = unsafe { CreatePopupMenu() };
        if handle.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }
    fn append(&self, flags: u32, id: usize, label: &str) -> io::Result<()> {
        let label: Vec<_> = label.encode_utf16().chain(Some(0)).collect();
        // SAFETY: This menu is owned; Windows copies the terminated label.
        if unsafe { AppendMenuW(self.0, flags, id, label.as_ptr()) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn release(self) -> HMENU {
        let handle = self.0;
        std::mem::forget(self);
        handle
    }
}
impl Drop for Popup {
    fn drop(&mut self) {
        // SAFETY: Only unattached menus are retained by this owner.
        unsafe {
            DestroyMenu(self.0);
        }
    }
}

/// # Safety
/// Window must be live on this UI thread. If non-null, `previous` must be its
/// final top-level submenu and must not be retained after successful replacement.
pub unsafe fn install(
    window: HWND,
    previous: HMENU,
    registry: &PluginRegistry,
) -> io::Result<HMENU> {
    let popup = Popup::new()?;
    if registry.plugins().is_empty() {
        popup.append(MF_STRING | MF_GRAYED, 0, "No plugins loaded")?;
    }
    for plugin in registry.plugins() {
        let submenu = Popup::new()?;
        for (command, item) in plugin.menu_items() {
            if let Some(command) = command {
                // SAFETY: Caller supplies the previous live popup, if any.
                // Preserve runtime check changes when rebuilding existing items.
                let previous_state = if previous.is_null() {
                    u32::MAX
                } else {
                    unsafe { GetMenuState(previous, command as u32, MF_BYCOMMAND) }
                };
                let checked = if previous_state == u32::MAX {
                    item.initially_checked
                } else {
                    previous_state & MF_CHECKED != 0
                };
                submenu.append(
                    MF_STRING | if checked { MF_CHECKED } else { 0 },
                    command as usize,
                    &item.name,
                )?;
            } else {
                submenu.append(MF_SEPARATOR, 0, "")?;
            }
        }
        popup.append(MF_POPUP, submenu.0 as usize, plugin.name())?;
        submenu.release(); // Parent now owns it, including on later failure.
    }
    let title: Vec<_> = "&Plugins".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Caller supplies a live owning window and (if present) its last popup.
    unsafe {
        let menu = crate::plugin_host::main_menu(window);
        if menu.is_null() {
            return Err(io::Error::other("Window has no menu"));
        }
        if previous.is_null() {
            if AppendMenuW(menu, MF_POPUP, popup.0 as usize, title.as_ptr()) == 0 {
                return Err(io::Error::last_os_error());
            }
        } else {
            let position = GetMenuItemCount(menu) - 1;
            if position < 0 || GetSubMenu(menu, position) != previous {
                return Err(io::Error::other("Plugin menu position changed"));
            }
            if ModifyMenuW(
                menu,
                position as u32,
                MF_BYPOSITION | MF_POPUP,
                popup.0 as usize,
                title.as_ptr(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            // ModifyMenuW destroys the replaced submenu itself:
            // https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-modifymenuw
        }
        let handle = popup.release();
        DrawMenuBar(window);
        Ok(handle)
    }
}
