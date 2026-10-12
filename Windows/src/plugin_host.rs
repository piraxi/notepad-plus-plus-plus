//! Synchronous read-only plugin queries use per-window snapshots. The window
//! procedure never borrows App while a plugin callback is executing.
use npp_compat::messages::*;
use npp_editor_core::{BufferId, Encoding, LineEnding, Workspace};
use std::{
    cell::RefCell,
    collections::HashMap,
    os::windows::ffi::{OsStrExt, OsStringExt},
};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Shell::{
    PathFindExtensionW, PathFindFileNameW, PathRemoveExtensionW, PathRemoveFileSpecW,
};

#[derive(Default)]
struct Snapshot {
    main_menu: usize,
    menu_hidden: bool,
    system_menu_entering: bool,
    removed_shortcuts: std::collections::HashSet<u32>,
    shortcuts: HashMap<u32, npp_compat::plugin::ShortcutKey>,
    plugin_menu: usize,
    dialogs: Vec<usize>,
    // Tokens stay allocated until the host window closes, so closed-buffer IDs
    // cannot accidentally identify newly opened buffers through address reuse.
    tokens: HashMap<BufferId, Box<BufferId>>,
    buffers: Vec<BufferInfo>,
    active: Option<usize>,
    line: usize,
    column: usize,
    line_text: Vec<u16>,
    session: Option<npp_editor_core::Session>,
}
struct BufferInfo {
    token: usize,
    encoding: isize,
    format: isize,
    path: Vec<u16>,
}
thread_local! {
    static HOSTS: RefCell<HashMap<usize, Snapshot>> = RefCell::new(HashMap::new());
}

pub fn publish(
    window: HWND,
    workspace: &Workspace,
    tab_width: std::num::NonZeroUsize,
    scroll_lines: &HashMap<BufferId, usize>,
) {
    HOSTS.with_borrow_mut(|hosts| {
        let snapshot = hosts.entry(window as usize).or_default();
        snapshot.buffers.clear();
        snapshot.active = None;
        snapshot.line = 0;
        snapshot.column = 0;
        snapshot.line_text.clear();
        snapshot.session = workspace.capture_session().ok();
        if let Some(session) = &mut snapshot.session {
            for file in &mut session.main_view.files {
                if let Some((id, _)) = workspace.buffers().find(|(_, document)| {
                    document.path().and_then(std::path::Path::to_str) == file.filename()
                }) {
                    if let Some(line) = scroll_lines.get(&id) {
                        file.attributes
                            .insert("firstVisibleLine".into(), line.to_string());
                    }
                }
            }
        }
        for (id, document) in workspace.buffers() {
            let identity = snapshot.tokens.entry(id).or_insert_with(|| Box::new(id));
            let token = &**identity as *const BufferId as usize;
            if workspace.active() == Some(id) {
                snapshot.active = Some(snapshot.buffers.len());
                (snapshot.line, snapshot.column) = document
                    .line_column(document.selection().caret, tab_width)
                    .expect("valid document caret");
                let line =
                    &document.text()[document.line_range(snapshot.line).expect("valid line")];
                // The supplied getLine helper returns an empty buffer when the
                // UTF-8 line length reaches CURRENTWORD_MAXLENGTH (2048 bytes).
                if line.len() < 2048 {
                    snapshot.line_text =
                        line.encode_utf16().take_while(|&unit| unit != 0).collect();
                }
            }
            snapshot.buffers.push(BufferInfo {
                token,
                path: document
                    .path()
                    .map(|path| path.as_os_str().encode_wide().collect())
                    .unwrap_or_else(|| {
                        workspace
                            .untitled_name(id)
                            .expect("untitled buffer name")
                            .encode_utf16()
                            .collect()
                    }),
                encoding: match document.encoding() {
                    Encoding::Utf8 => 4,
                    Encoding::Utf8Bom => 1,
                    Encoding::Utf16Be => 2,
                    Encoding::Utf16Le => 3,
                    Encoding::Utf16BeNoBom => 6,
                    Encoding::Utf16LeNoBom => 7,
                    Encoding::Windows1252 | Encoding::Windows1251 | Encoding::ShiftJis => 0,
                },
                format: match document.line_ending() {
                    LineEnding::CrLf => 0,
                    LineEnding::Cr => 1,
                    LineEnding::Lf => 2,
                },
            });
        }
    });
}

pub fn remove(window: HWND) {
    HOSTS.with_borrow_mut(|hosts| {
        hosts.remove(&(window as usize));
    });
}

pub fn publish_main_menu(window: HWND, menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU) {
    HOSTS.with_borrow_mut(|hosts| {
        hosts.entry(window as usize).or_default().main_menu = menu as usize
    });
}

pub fn main_menu(window: HWND) -> windows_sys::Win32::UI::WindowsAndMessaging::HMENU {
    let saved = HOSTS.with_borrow(|hosts| {
        hosts
            .get(&(window as usize))
            .map_or(0, |host| host.main_menu)
    });
    if saved != 0 {
        saved as _
    } else {
        // SAFETY: Read the menu attached to the caller's live host window.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetMenu(window) }
    }
}

pub fn set_menu_hidden(window: HWND, hidden: bool) {
    HOSTS.with_borrow_mut(|hosts| {
        hosts.entry(window as usize).or_default().menu_hidden = hidden;
    });
}

pub fn entering_system_menu(window: HWND) {
    HOSTS.with_borrow_mut(|hosts| {
        hosts
            .entry(window as usize)
            .or_default()
            .system_menu_entering = true;
    });
}

/// Return a temporary attachment change without holding a borrow across Win32 calls.
pub fn menu_loop(window: HWND, entering: bool, popup: bool) -> Option<bool> {
    HOSTS.with_borrow_mut(|hosts| {
        let host = hosts.get_mut(&(window as usize))?;
        let change = (host.menu_hidden && !popup && !host.system_menu_entering).then_some(entering);
        if !entering {
            host.system_menu_entering = false;
        }
        change
    })
}

pub fn publish_plugin_menu(
    window: HWND,
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    registry: &npp_compat::registry::PluginRegistry,
) {
    HOSTS.with_borrow_mut(|hosts| {
        let host = hosts.entry(window as usize).or_default();
        host.plugin_menu = menu as usize;
        host.shortcuts.clear();
        for plugin in registry.plugins() {
            for (command, item) in plugin.menu_items() {
                if let (Some(command), Some(shortcut)) = (command, item.shortcut) {
                    if shortcut.key != 0 && !host.removed_shortcuts.contains(&u32::from(command)) {
                        host.shortcuts.insert(u32::from(command), shortcut);
                    }
                }
            }
        }
    });
}

pub fn menu_message(window: HWND, message: u32, wparam: usize, lparam: isize) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match message {
        NPPM_REMOVESHORTCUTBYCMDID => {
            let command = wparam as u32;
            if command == 0 {
                return Some(0);
            }
            HOSTS.with_borrow_mut(|hosts| {
                if let Some(host) = hosts.get_mut(&(window as usize)) {
                    if host.shortcuts.remove(&command).is_some() {
                        host.removed_shortcuts.insert(command);
                    }
                }
            });
            Some(1)
        }
        NPPM_GETMENUHANDLE => Some(match wparam {
            NPPPLUGINMENU => HOSTS.with_borrow(|hosts| {
                hosts
                    .get(&(window as usize))
                    .map_or(0, |host| host.plugin_menu as isize)
            }),
            // SAFETY: Called on the window thread with the live host window.
            NPPMAINMENU => main_menu(window) as isize,
            _ => 0,
        }),
        NPPM_SETMENUITEMCHECK => {
            // SAFETY: Native menu operation uses only scalar IDs and flags.
            // The source returns TRUE even when the command is absent.
            unsafe {
                CheckMenuItem(
                    main_menu(window),
                    wparam as u32,
                    MF_BYCOMMAND
                        | if lparam as i32 != 0 {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                );
            }
            Some(1)
        }
        _ => None,
    }
}

pub fn shortcut_enabled(window: HWND, command: u16) -> bool {
    HOSTS.with_borrow(|hosts| {
        hosts
            .get(&(window as usize))
            .is_some_and(|host| host.shortcuts.contains_key(&u32::from(command)))
    })
}

/// Register handles without taking ownership; plugins destroy their own dialogs.
pub fn modeless_dialog(window: HWND, message: u32, action: usize, dialog: isize) -> Option<isize> {
    if message != NPPM_MODELESSDIALOG {
        return None;
    }
    Some(HOSTS.with_borrow_mut(|hosts| {
        let dialogs = &mut hosts.entry(window as usize).or_default().dialogs;
        match action {
            MODELESSDIALOGADD => {
                if dialogs.contains(&(dialog as usize)) {
                    0
                } else {
                    dialogs.push(dialog as usize);
                    dialog
                }
            }
            MODELESSDIALOGREMOVE => {
                if let Some(index) = dialogs.iter().position(|&handle| handle == dialog as usize) {
                    dialogs.remove(index);
                    0
                } else {
                    dialog
                }
            }
            _ => 1,
        }
    }))
}

pub fn translate_dialogs(
    window: HWND,
    message: &windows_sys::Win32::UI::WindowsAndMessaging::MSG,
) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsDialogMessageW, IsWindow};
    let dialogs = HOSTS.with_borrow(|hosts| {
        hosts
            .get(&(window as usize))
            .map(|host| host.dialogs.clone())
            .unwrap_or_default()
    });
    for dialog in dialogs {
        let registered = HOSTS.with_borrow(|hosts| {
            hosts
                .get(&(window as usize))
                .is_some_and(|host| host.dialogs.contains(&dialog))
        });
        if !registered {
            continue;
        }
        // SAFETY: The handle is validated before synchronous Windows dispatch.
        // No TLS borrow is held: a plugin may unregister or query the host from
        // a dialog callback. The host never destroys plugin-owned windows.
        if unsafe {
            IsWindow(dialog as HWND) != 0 && IsDialogMessageW(dialog as HWND, message) != 0
        } {
            return true;
        }
    }
    false
}

pub fn query(window: HWND, message: u32, wparam: usize, lparam: isize) -> Option<isize> {
    HOSTS.with_borrow(|hosts| {
        let snapshot = hosts.get(&(window as usize))?;
        let buffer = || {
            snapshot
                .buffers
                .iter()
                .find(|buffer| buffer.token == wparam)
        };
        Some(match message {
            // The current shell has one editor view, regardless of its active tab.
            NPPM_GETCURRENTVIEW => MAIN_VIEW as isize,
            NPPM_GETCURRENTLINE => snapshot.line as isize,
            NPPM_GETCURRENTCOLUMN => snapshot.column as isize,
            NPPM_GETNBOPENFILES => {
                if matches!(lparam, 0 | 1) {
                    snapshot.buffers.len() as isize
                } else {
                    0
                }
            }
            NPPM_GETCURRENTDOCINDEX => {
                if lparam == 0 {
                    snapshot.active.map_or(-1, |index| index as isize)
                } else {
                    -1
                }
            }
            NPPM_GETCURRENTBUFFERID => snapshot
                .active
                .map_or(0, |index| snapshot.buffers[index].token as isize),
            NPPM_GETBUFFERIDFROMPOS => {
                if lparam == 0 {
                    snapshot
                        .buffers
                        .get(wparam)
                        .map_or(0, |buffer| buffer.token as isize)
                } else {
                    0
                }
            }
            NPPM_GETPOSFROMBUFFERID => snapshot
                .buffers
                .iter()
                .position(|buffer| buffer.token == wparam)
                .map_or(-1, |index| index as isize),
            NPPM_GETBUFFERENCODING => buffer().map_or(-1, |buffer| buffer.encoding),
            NPPM_GETBUFFERFORMAT => buffer().map_or(-1, |buffer| buffer.format),
            _ => return None,
        })
    })
}

/// # Safety
/// For recognized text queries, a non-null lParam must point to writable UTF-16
/// storage as required by the upstream message. GETFULLPATHFROMBUFFERID and
/// legacy current-path calls with wParam=0 carry no capacity: the caller must
/// allocate the full length plus a terminator. No pointer is retained.
/// For deprecated GETOPENFILENAMES messages, wParam instead points to an array
/// of lParam writable UTF-16 buffer pointers; each destination must accommodate
/// its full path plus NUL. The legacy API carries no per-string capacity.
/// GETSHORTCUTBYCMDID instead requires lParam to point to one writable ShortcutKey.
/// GETCURRENTSCINTILLA instead requires lParam to point to one writable i32.
/// Session queries require lParam to point to a NUL-terminated UTF-16 filename.
/// GETNBSESSIONFILES optionally writes a 32-bit BOOL through wParam.
/// GETSESSIONFILES requires wParam to point to an array with one writable UTF-16
/// destination per session entry, each large enough for the filename plus NUL.
/// SAVESESSION requires a readable source-compatible sessionInfo structure,
/// its filename and count file pointers. SAVECURRENTSESSION takes a filename.
pub unsafe fn query_text(
    window: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Option<isize> {
    if matches!(message, NPPM_SAVESESSION | NPPM_SAVECURRENTSESSION) {
        #[repr(C)]
        struct SessionInfo {
            filename: *const u16,
            count: i32,
            files: *const *const u16,
        }
        if lparam == 0 {
            return Some(0);
        }
        // SAFETY: Plugin messages provide the source-compatible struct or a
        // terminated filename, valid for the duration of this synchronous call.
        let (filename, count, files) = unsafe {
            if message == NPPM_SAVESESSION {
                let info = &*(lparam as *const SessionInfo);
                if info.count < 0 {
                    return Some(0);
                }
                (info.filename, info.count as usize, info.files)
            } else {
                (lparam as *const u16, 0, std::ptr::null())
            }
        };
        // SAFETY: Readable terminated input is required by the plugin contract.
        let Some(path) = (unsafe { session_path(filename) }) else {
            return Some(0);
        };
        let mut session = if count != 0 && !files.is_null() {
            let mut session = npp_editor_core::Session::default();
            for index in 0..count {
                // SAFETY: The plugin supplies count readable filename pointers.
                let file = unsafe { session_path(*files.add(index)) };
                if let Some(file) = file.filter(|file| file.is_file()) {
                    let Some(name) = file.to_str() else {
                        return Some(0);
                    };
                    session
                        .main_view
                        .files
                        .push(npp_editor_core::SessionFile::new(name.into()));
                }
            }
            session
        } else {
            let Some(session) = HOSTS.with_borrow(|hosts| {
                hosts
                    .get(&(window as usize))
                    .and_then(|host| host.session.clone())
            }) else {
                return Some(0);
            };
            session
        };
        session.main_view.files.retain(|file| {
            file.filename()
                .is_some_and(|name| std::path::Path::new(name).is_file())
        });
        let result = session.to_xml().and_then(|bytes| {
            crate::file_io::normalized_save_path(&path)
                .and_then(|path| crate::file_io::atomic_write(&path, &bytes))
        });
        return Some(if result.is_ok() { filename as isize } else { 0 });
    }
    if matches!(message, NPPM_GETNBSESSIONFILES | NPPM_GETSESSIONFILES) {
        if message == NPPM_GETNBSESSIONFILES && wparam != 0 {
            // SAFETY: Optional writable BOOL follows the plugin API contract.
            unsafe { (wparam as *mut i32).write(0) };
        }
        if lparam == 0 {
            return Some(0);
        }
        // SAFETY: The plugin owns a readable NUL-terminated filename for this
        // synchronous call. No pointer or borrowed slice is retained.
        let filename = unsafe {
            let pointer = lparam as *const u16;
            let mut length = 0;
            while *pointer.add(length) != 0 {
                length += 1;
            }
            if length == 0 {
                return Some(0);
            }
            std::ffi::OsString::from_wide(std::slice::from_raw_parts(pointer, length))
        };
        let session = std::fs::read(std::path::Path::new(&filename))
            .and_then(|bytes| npp_editor_core::Session::from_xml(&bytes));
        let Ok(session) = session else {
            return Some(0);
        };
        let files = session
            .main_view
            .files
            .iter()
            .chain(&session.sub_view.files);
        if message == NPPM_GETNBSESSIONFILES {
            if wparam != 0 {
                // SAFETY: Optional writable BOOL follows the contract above.
                unsafe { (wparam as *mut i32).write(1) };
            }
            return Some(files.count() as isize);
        }
        if wparam == 0 {
            return Some(0);
        }
        // Validate null destinations before writing any path; the legacy API
        // supplies no capacities, so allocation remains the caller's duty.
        for index in 0..files.clone().count() {
            // SAFETY: Caller supplies the complete array of destination pointers.
            if unsafe { *(wparam as *const *mut u16).add(index) }.is_null() {
                return Some(0);
            }
        }
        for (index, file) in files.enumerate() {
            let path: Vec<u16> = file
                .filename()
                .unwrap_or_default()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            // SAFETY: Caller guarantees capacity for the full filename and NUL.
            unsafe {
                let destination = *(wparam as *const *mut u16).add(index);
                std::ptr::copy_nonoverlapping(path.as_ptr(), destination, path.len());
            }
        }
        return Some(1);
    }
    if message == NPPM_GETCURRENTSCINTILLA {
        if lparam == 0 {
            return Some(0);
        }
        let view = query(window, NPPM_GETCURRENTVIEW, 0, 0)? as i32;
        // SAFETY: The caller supplies one writable 32-bit int, even on Win64.
        unsafe { (lparam as *mut i32).write(view) };
        return Some(1);
    }
    if message == NPPM_GETSHORTCUTBYCMDID {
        if wparam as u32 == 0 || lparam == 0 {
            return Some(0);
        }
        let shortcut = HOSTS.with_borrow(|hosts| {
            hosts
                .get(&(window as usize))
                .and_then(|host| host.shortcuts.get(&(wparam as u32)))
                .copied()
        });
        if let Some(shortcut) = shortcut {
            // SAFETY: The caller supplies one writable ABI-compatible struct.
            unsafe {
                (lparam as *mut npp_compat::plugin::ShortcutKey).write(shortcut);
            }
            return Some(1);
        }
        return Some(0);
    }
    if matches!(
        message,
        NPPM_GETOPENFILENAMES_DEPRECATED
            | NPPM_GETOPENFILENAMESPRIMARY_DEPRECATED
            | NPPM_GETOPENFILENAMESSECOND_DEPRECATED
    ) {
        if wparam == 0 || message == NPPM_GETOPENFILENAMESSECOND_DEPRECATED {
            return Some(0);
        }
        let paths = HOSTS.with_borrow(|hosts| {
            hosts.get(&(window as usize)).map(|snapshot| {
                snapshot
                    .buffers
                    .iter()
                    .take(lparam as usize)
                    .map(|buffer| buffer.path.clone())
                    .collect::<Vec<_>>()
            })
        });
        let Some(paths) = paths else {
            return Some(0);
        };
        for (index, path) in paths.iter().enumerate() {
            // SAFETY: The plugin supplies an array and full-sized destinations
            // under the legacy contract above. Snapshot data is owned and no
            // App/TLS borrow is held while writing into the plugin's buffers.
            unsafe {
                let destination = *(wparam as *const *mut u16).add(index);
                if destination.is_null() {
                    return Some(index as isize);
                }
                std::ptr::copy_nonoverlapping(path.as_ptr(), destination, path.len());
                *destination.add(path.len()) = 0;
            }
        }
        return Some(paths.len() as isize);
    }
    if !matches!(
        message,
        NPPM_GETFULLPATHFROMBUFFERID
            | NPPM_GETFULLCURRENTPATH
            | NPPM_GETCURRENTDIRECTORY
            | NPPM_GETFILENAME
            | NPPM_GETNAMEPART
            | NPPM_GETEXTPART
            | NPPM_GETCURRENTLINESTR
            | NPPM_GETNPPDIRECTORY
            | NPPM_GETNPPFULLFILEPATH
    ) {
        return None;
    }
    let path = HOSTS.with_borrow(|hosts| {
        if matches!(message, NPPM_GETNPPDIRECTORY | NPPM_GETNPPFULLFILEPATH) {
            // Match the source's MAX_PATH module-name buffer. This query is
            // independent of the selected document and does not call plugins.
            let mut module = vec![0u16; 260];
            // SAFETY: A null module denotes this process; the buffer contains
            // 260 writable UTF-16 units and remains live throughout the call.
            let length = unsafe {
                windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
                    std::ptr::null_mut(),
                    module.as_mut_ptr(),
                    module.len() as u32,
                )
            };
            if length == 0 {
                return None;
            }
            module.truncate(module.iter().position(|&unit| unit == 0).unwrap_or(259));
            return Some(module);
        }
        let snapshot = hosts.get(&(window as usize))?;
        let buffer = if message == NPPM_GETFULLPATHFROMBUFFERID {
            snapshot
                .buffers
                .iter()
                .find(|buffer| buffer.token == wparam)
        } else if message == NPPM_GETCURRENTLINESTR {
            return Some(snapshot.line_text.clone());
        } else {
            snapshot
                .active
                .and_then(|index| snapshot.buffers.get(index))
        }?;
        Some(buffer.path.clone())
    });
    // The supplied NppBigSwitch.cpp returns FALSE for unknown buffer IDs,
    // although the public header describes -1 for that case.
    let Some(mut path) = path else {
        return Some(0);
    };
    path.push(0);
    // SAFETY: These path helpers operate inside the owned terminated buffer.
    unsafe {
        let mut start = path.as_mut_ptr();
        match message {
            NPPM_GETCURRENTDIRECTORY | NPPM_GETNPPDIRECTORY => {
                PathRemoveFileSpecW(start);
            }
            NPPM_GETFILENAME => {
                start = PathFindFileNameW(start);
            }
            NPPM_GETNAMEPART => {
                start = PathFindFileNameW(start);
                PathRemoveExtensionW(start);
            }
            NPPM_GETEXTPART => {
                start = PathFindExtensionW(start);
            }
            _ => (),
        }
        let offset = start.offset_from(path.as_ptr()) as usize;
        let length = path[offset..]
            .iter()
            .position(|&unit| unit == 0)
            .expect("terminated path buffer");
        if message == NPPM_GETFULLPATHFROMBUFFERID {
            if lparam != 0 {
                std::ptr::copy_nonoverlapping(start, lparam as *mut u16, length + 1);
            }
            return Some(length as isize);
        }
        if lparam == 0 || (wparam != 0 && wparam <= length) {
            return Some(0);
        }
        std::ptr::copy_nonoverlapping(start, lparam as *mut u16, length + 1);
        Some(1)
    }
}

/// Read a filename owned by a trusted native plugin; no pointer is retained.
/// # Safety
/// A non-null pointer must address readable NUL-terminated UTF-16 storage.
pub(crate) unsafe fn session_path(pointer: *const u16) -> Option<std::path::PathBuf> {
    if pointer.is_null() {
        return None;
    }
    // SAFETY: The caller provides a readable terminated string.
    unsafe {
        let mut length = 0;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        if length == 0 {
            return None;
        }
        Some(std::ffi::OsString::from_wide(std::slice::from_raw_parts(pointer, length)).into())
    }
}
