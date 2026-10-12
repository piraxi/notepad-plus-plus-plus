//! Plugin command ownership and routing, independent of native menu widgets.
use crate::{
    loader::{LoadedPlugin, PluginMenuItem},
    plugin::ScNotification,
};
use std::{fmt, ops::RangeInclusive};

// Supplied PowerEditor/src/resource.h reserves this range for plugin commands.
pub const FIRST_PLUGIN_COMMAND: u16 = 22000;
pub const LAST_PLUGIN_COMMAND: u16 = 22999;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationError {
    DuplicateModule,
    CommandRangeFull,
}

/// Failure retains the library so its caller can perform any required shutdown
/// before dropping it. Registration never invokes plugin callbacks.
pub struct RejectedPlugin {
    pub reason: RegistrationError,
    pub plugin: Box<LoadedPlugin>,
}
impl fmt::Debug for RejectedPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectedPlugin")
            .field("reason", &self.reason)
            .field("name", &self.plugin.name())
            .finish()
    }
}

pub struct RegisteredPlugin {
    library: LoadedPlugin,
    command_ids: Vec<Option<u16>>,
}
impl RegisteredPlugin {
    pub fn name(&self) -> &str {
        self.library.name()
    }
    /// Separators have no command ID and consume no slot in the reserved range.
    pub fn menu_items(&self) -> impl Iterator<Item = (Option<u16>, &PluginMenuItem)> {
        self.command_ids
            .iter()
            .copied()
            .zip(self.library.menu_items())
    }
}

pub struct PluginRegistry {
    first: u16,
    capacity: usize,
    commands: Vec<(usize, usize)>,
    plugins: Vec<RegisteredPlugin>,
}
impl Default for PluginRegistry {
    fn default() -> Self {
        Self::with_command_range(FIRST_PLUGIN_COMMAND..=LAST_PLUGIN_COMMAND)
    }
}
impl PluginRegistry {
    /// The host must reserve this range exclusively for plugin menu commands.
    /// IDs remain stable until the registry is dropped; individual unloading is
    /// deliberately not exposed while native menus may retain IDs or callbacks.
    pub fn with_command_range(range: RangeInclusive<u16>) -> Self {
        let capacity = if range.is_empty() {
            0
        } else {
            usize::from(*range.end()) - usize::from(*range.start()) + 1
        };
        Self {
            first: *range.start(),
            capacity,
            commands: Vec::new(),
            plugins: Vec::new(),
        }
    }

    pub fn plugins(&self) -> &[RegisteredPlugin] {
        &self.plugins
    }

    pub fn register(&mut self, mut plugin: LoadedPlugin) -> Result<usize, RejectedPlugin> {
        let reject = if self
            .plugins
            .iter()
            .any(|entry| entry.library.same_module(&plugin))
        {
            Some(RegistrationError::DuplicateModule)
        } else if plugin
            .menu_items()
            .iter()
            .filter(|item| !item.is_separator)
            .count()
            > self.capacity - self.commands.len()
        {
            Some(RegistrationError::CommandRangeFull)
        } else {
            None
        };
        if let Some(reason) = reject {
            return Err(RejectedPlugin {
                reason,
                plugin: Box::new(plugin),
            });
        }
        // Preflight above ensures no registry or plugin metadata is changed on rejection.
        let index = self.plugins.len();
        let mut ids = Vec::with_capacity(plugin.menu_items().len());
        for item in 0..plugin.menu_items().len() {
            if plugin.menu_items()[item].is_separator {
                ids.push(None);
                continue;
            }
            let id = (usize::from(self.first) + self.commands.len()) as u16;
            plugin
                .assign_command_id(item, i32::from(id))
                .expect("validated command array index");
            self.commands.push((index, item));
            ids.push(Some(id));
        }
        self.plugins.push(RegisteredPlugin {
            library: plugin,
            command_ids: ids,
        });
        Ok(index)
    }

    /// # Safety
    /// The host must satisfy the target plugin's command and reentrancy contracts.
    /// Do not unload the registry or host windows while callbacks are executing.
    pub unsafe fn dispatch(&self, command_id: u16) -> bool {
        let Some(offset) = command_id.checked_sub(self.first) else {
            return false;
        };
        let Some(&(plugin, item)) = self.commands.get(usize::from(offset)) else {
            return false;
        };
        // SAFETY: Registration retains this DLL and validates the command index;
        // caller supplies the compatible host environment.
        unsafe {
            self.plugins[plugin]
                .library
                .invoke(item)
                .expect("registered command index")
        }
    }

    /// # Safety
    /// Every plugin must accept this event, and any referenced notification data
    /// must remain valid for the entire synchronous broadcast. Plugins may mutate
    /// the notification as in upstream; the host must handle resulting changes.
    pub unsafe fn notify_all(&self, notification: &mut ScNotification) {
        for plugin in &self.plugins {
            unsafe {
                plugin.library.notify(notification);
            }
        }
    }

    /// # Safety
    /// Parameters must satisfy the addressed plugin's message protocol. Plugins
    /// and host handles must remain alive throughout reentrant callbacks.
    pub unsafe fn message(
        &self,
        plugin: usize,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> Option<isize> {
        self.plugins
            .get(plugin)
            .map(|entry| unsafe { entry.library.message(message, wparam, lparam) })
    }
}
