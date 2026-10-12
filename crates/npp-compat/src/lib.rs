//! Source-derived command IDs, Windows plugin ABI declarations and DLL loading.
//! These components do not yet provide application host-message compatibility.
#[cfg(windows)]
pub mod loader;
#[cfg(windows)]
pub mod registry;
pub mod commands {
    include!(concat!(env!("OUT_DIR"), "/commands.rs"));
}
pub mod messages;

#[cfg(windows)]
pub mod plugin {
    use std::ffi::c_void;

    pub type WindowHandle = *mut c_void;
    pub type PluginCommand = unsafe extern "C" fn();

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct NppData {
        pub npp_handle: WindowHandle,
        pub scintilla_main_handle: WindowHandle,
        pub scintilla_second_handle: WindowHandle,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ShortcutKey {
        pub is_ctrl: bool,
        pub is_alt: bool,
        pub is_shift: bool,
        pub key: u8,
    }

    #[repr(C)]
    pub struct FuncItem {
        pub item_name: [u16; 64],
        pub function: Option<PluginCommand>,
        pub command_id: i32,
        pub initially_checked: bool,
        pub shortcut: *mut ShortcutKey,
    }

    pub type GetName = unsafe extern "C" fn() -> *const u16;
    /// Notification layout from the supplied Scintilla.h (Sci_Position is isize).
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct NotifyHeader {
        pub hwnd_from: WindowHandle,
        pub id_from: usize,
        pub code: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct ScNotification {
        pub header: NotifyHeader,
        pub position: isize,
        pub ch: i32,
        pub modifiers: i32,
        pub modification_type: i32,
        pub text: *const std::ffi::c_char,
        pub length: isize,
        pub lines_added: isize,
        pub message: i32,
        pub wparam: usize,
        pub lparam: isize,
        pub line: isize,
        pub fold_level_now: i32,
        pub fold_level_prev: i32,
        pub margin: i32,
        pub list_type: i32,
        pub x: i32,
        pub y: i32,
        pub token: i32,
        pub annotation_lines_added: isize,
        pub updated: i32,
        pub list_completion_method: i32,
        pub character_source: i32,
    }

    pub type BeNotified = unsafe extern "C" fn(*mut ScNotification);
    pub type SetInfo = unsafe extern "C" fn(NppData);
    pub type GetFuncsArray = unsafe extern "C" fn(*mut i32) -> *mut FuncItem;
    pub type MessageProc = unsafe extern "C" fn(u32, usize, isize) -> isize;
    pub type IsUnicode = unsafe extern "C" fn() -> i32;
}
