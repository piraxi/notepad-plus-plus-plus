use npp_compat::registry::PluginRegistry;
use std::{io, ptr::null_mut};
use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};

pub struct PluginAccelerators {
    table: HACCEL,
    bindings: Vec<ACCEL>,
}
impl Default for PluginAccelerators {
    fn default() -> Self {
        Self {
            table: null_mut(),
            bindings: Vec::new(),
        }
    }
}
impl PluginAccelerators {
    pub fn build(registry: &PluginRegistry) -> io::Result<Self> {
        let mut bindings = Vec::new();
        for plugin in registry.plugins() {
            for (command, item) in plugin.menu_items() {
                if let (Some(command), Some(shortcut)) = (command, item.shortcut) {
                    if shortcut.key == 0 {
                        continue;
                    }
                    bindings.push(ACCEL {
                        fVirt: FVIRTKEY
                            | if shortcut.is_ctrl { FCONTROL } else { 0 }
                            | if shortcut.is_alt { FALT } else { 0 }
                            | if shortcut.is_shift { FSHIFT } else { 0 },
                        key: u16::from(shortcut.key),
                        cmd: command,
                    });
                }
            }
        }
        Self::from_bindings(bindings)
    }

    fn from_bindings(bindings: Vec<ACCEL>) -> io::Result<Self> {
        if bindings.is_empty() {
            return Ok(Self::default());
        }
        // SAFETY: Windows copies the complete initialized accelerator array.
        let table = unsafe { CreateAcceleratorTableW(bindings.as_ptr(), bindings.len() as i32) };
        if table.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self { table, bindings })
        }
    }

    pub fn translate(&mut self, window: HWND, message: &MSG) -> bool {
        if self
            .bindings
            .iter()
            .any(|binding| !crate::plugin_host::shortcut_enabled(window, binding.cmd))
        {
            let bindings = self
                .bindings
                .iter()
                .filter(|binding| crate::plugin_host::shortcut_enabled(window, binding.cmd))
                .copied()
                .collect();
            match Self::from_bindings(bindings) {
                Ok(replacement) => *self = replacement,
                // Do not dispatch a removed binding when allocation fails.
                Err(_) => return false,
            }
        }
        // SAFETY: Called on the host UI thread. The owned table remains live;
        // translated WM_COMMAND messages queue dispatch outside the window proc.
        !self.table.is_null() && unsafe { TranslateAcceleratorW(window, self.table, message) != 0 }
    }
}
impl Drop for PluginAccelerators {
    fn drop(&mut self) {
        // SAFETY: This wrapper uniquely owns the accelerator table.
        unsafe {
            if !self.table.is_null() {
                DestroyAcceleratorTable(self.table);
            }
        }
    }
}
