use npp_editor_core::{SearchDirection, SearchMode, SearchOptions};
use std::{
    cell::UnsafeCell,
    io,
    mem::size_of,
    ptr::null_mut,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::{
            BST_CHECKED, Dialogs::*, EM_SETSEL, IsDlgButtonChecked, chx1, chx2, edt1, edt2, psh1,
            psh2, rad1,
        },
        Input::KeyboardAndMouse::EnableWindow,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const SEARCH_TABS: usize = 620;
const MODE_NORMAL: usize = 621;
const MODE_EXTENDED: usize = 622;
const MODE_REGEX: usize = 623;
const WRAP: usize = 624;
const COUNT: usize = 625;
unsafe extern "system" fn layout_child(hwnd: HWND, replace: LPARAM) -> i32 {
    unsafe {
        use windows_sys::Win32::UI::Controls::*;
        let id = GetDlgCtrlID(hwnd) as u32;
        let position = if id == edt1 {
            Some((110, 52, 390, 25))
        } else if id == edt2 {
            Some((110, 90, 390, 25))
        } else if id == IDOK as u32 {
            Some((535, 52, 110, 28))
        } else if id == IDCANCEL as u32 {
            Some((535, if replace != 0 { 166 } else { 90 }, 110, 28))
        } else if id == psh1 {
            Some((535, 90, 110, 28))
        } else if id == psh2 {
            Some((535, 128, 110, 28))
        } else if id == chx1 {
            Some((12, 138, 235, 25))
        } else if id == chx2 {
            Some((12, 170, 235, 25))
        } else if id == grp1 {
            Some((280, 138, 220, 65))
        } else if id == rad1 {
            Some((295, 165, 85, 25))
        } else if id == rad2 {
            Some((390, 165, 95, 25))
        } else {
            None
        };
        if let Some((x, y, width, height)) = position {
            MoveWindow(hwnd, x, y, width, height, 1);
        } else {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
    1
}

static MESSAGE: OnceLock<u32> = OnceLock::new();
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

unsafe extern "system" fn close_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: The boxed FINDREPLACEW and its buffers outlive the dialog. Capture
    // before default processing destroys controls and sends FR_DIALOGTERM.
    unsafe {
        if message == WM_COMMAND {
            match wp & 0xffff {
                MODE_NORMAL | MODE_EXTENDED | MODE_REGEX => {
                    crate::app::enqueue(crate::app::Event::SearchMode(match wp & 0xffff {
                        MODE_EXTENDED => SearchMode::Extended,
                        MODE_REGEX => SearchMode::Regex,
                        _ => SearchMode::Normal,
                    }));
                    return 0;
                }
                WRAP => {
                    crate::app::enqueue(crate::app::Event::Command(crate::app::SEARCH_WRAP));
                    return 0;
                }
                COUNT => {
                    crate::app::enqueue(crate::app::Event::Command(crate::app::SEARCH_COUNT));
                    return 0;
                }
                _ => {}
            }
        }
        if message == WM_NOTIFY && lp != 0 {
            let header = &*(lp as *const windows_sys::Win32::UI::Controls::NMHDR);
            if header.idFrom == SEARCH_TABS
                && header.code == windows_sys::Win32::UI::Controls::TCN_SELCHANGE
            {
                let tab = SendMessageW(
                    header.hwndFrom,
                    windows_sys::Win32::UI::Controls::TCM_GETCURSEL,
                    0,
                    0,
                );
                ShowWindow(hwnd, SW_HIDE);
                let input = GetDlgItem(hwnd, edt1 as i32);
                let mut needle = vec![0u16; GetWindowTextLengthW(input) as usize + 1];
                let length = GetWindowTextW(input, needle.as_mut_ptr(), needle.len() as i32);
                crate::app::enqueue(crate::app::Event::SearchTab(
                    match tab {
                        0 => npp_compat::commands::IDM_SEARCH_FIND,
                        1 => npp_compat::commands::IDM_SEARCH_REPLACE,
                        _ => npp_compat::commands::IDM_SEARCH_FINDINFILES,
                    },
                    String::from_utf16_lossy(&needle[..length as usize]),
                ));
                return 0;
            }
        }
        if message == WM_CLOSE || (message == WM_COMMAND && wp & 0xffff == IDCANCEL as usize) {
            let data = &mut *(data as *mut FINDREPLACEW);
            GetDlgItemTextW(
                hwnd,
                edt1 as i32,
                data.lpstrFindWhat,
                data.wFindWhatLen as i32,
            );
            if !GetDlgItem(hwnd, edt2 as i32).is_null() {
                GetDlgItemTextW(
                    hwnd,
                    edt2 as i32,
                    data.lpstrReplaceWith,
                    data.wReplaceWithLen as i32,
                );
            }
            data.Flags &= !(FR_MATCHCASE | FR_WHOLEWORD);
            if IsDlgButtonChecked(hwnd, chx2 as i32) == BST_CHECKED {
                data.Flags |= FR_MATCHCASE;
            }
            if IsDlgButtonChecked(hwnd, chx1 as i32) == BST_CHECKED {
                data.Flags |= FR_WHOLEWORD;
            }
            if !GetDlgItem(hwnd, rad1 as i32).is_null() {
                data.Flags &= !FR_DOWN;
                if IsDlgButtonChecked(hwnd, rad1 as i32) != BST_CHECKED {
                    data.Flags |= FR_DOWN;
                }
            }
        }
        DefSubclassProc(hwnd, message, wp, lp)
    }
}

unsafe fn replacement_buttons(hwnd: HWND, read_only: bool) {
    // SAFETY: Called only for this owned, live Replace dialog on its UI thread.
    unsafe {
        let enabled = !read_only && GetWindowTextLengthW(GetDlgItem(hwnd, edt1 as i32)) > 0;
        for id in [psh1, psh2] {
            EnableWindow(GetDlgItem(hwnd, id as i32), enabled as i32);
        }
    }
}

unsafe extern "system" fn replace_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    read_only: usize,
) -> LRESULT {
    // SAFETY: Windows dispatches this subclass on the dialog's owning thread.
    unsafe {
        let result = DefSubclassProc(hwnd, message, wp, lp);
        // The common dialog updates buttons when Find What changes. Apply the
        // buffer restriction after its default processing so typing cannot undo it.
        if message == WM_COMMAND && IsWindow(hwnd) != 0 {
            replacement_buttons(hwnd, read_only != 0);
        }
        result
    }
}

pub fn native_replacement(mode: SearchMode, input: &str) -> io::Result<std::borrow::Cow<'_, str>> {
    let text = mode
        .decode(input)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if text.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "The native text view cannot display embedded NUL characters.",
        ));
    }
    Ok(text)
}
pub fn message() -> u32 {
    // SAFETY: Static NUL-terminated Windows message name.
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(FINDMSGSTRINGW) })
}
#[derive(Clone, Debug)]
pub struct Request {
    pub id: usize,
    pub flags: u32,
    pub needle: String,
    pub replacement: String,
    pub options: SearchOptions,
}

/// Call only for a live FINDMSGSTRING notification, before returning to Windows.
pub unsafe fn snapshot(pointer: LPARAM) -> Request {
    // SAFETY: Caller guarantees the notification structure and its sized buffers
    // are live. Copies leave no pointers in the application event queue.
    unsafe {
        let data = &*(pointer as *const FINDREPLACEW);
        let read = |pointer: *const u16, length: u16| {
            if pointer.is_null() {
                return String::new();
            }
            let units = std::slice::from_raw_parts(pointer, length as usize);
            let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
            String::from_utf16_lossy(&units[..end])
        };
        Request {
            id: data.lCustData as usize,
            flags: data.Flags,
            needle: read(data.lpstrFindWhat, data.wFindWhatLen),
            replacement: read(data.lpstrReplaceWith, data.wReplaceWithLen),
            options: SearchOptions {
                mode: npp_editor_core::SearchMode::Normal,
                match_case: data.Flags & FR_MATCHCASE != 0,
                whole_word: data.Flags & FR_WHOLEWORD != 0,
                wrap: true,
                direction: if data.Flags & FR_DOWN != 0 {
                    SearchDirection::Forward
                } else {
                    SearchDirection::Backward
                },
            },
        }
    }
}

pub struct SearchDialog {
    pub hwnd: HWND,
    pub id: usize,
    replace: bool,
    // Stable allocations are retained until after the modeless window closes.
    _data: Box<UnsafeCell<FINDREPLACEW>>,
    _needle: Vec<u16>,
    _replacement: Vec<u16>,
}
impl SearchDialog {
    pub fn open(
        owner: HWND,
        replace: bool,
        initial: &str,
        options: SearchOptions,
    ) -> io::Result<Self> {
        if message() == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut needle = vec![0u16; 4096];
        let mut cursor = 0;
        for ch in initial.chars() {
            let mut units = [0; 2];
            let units = ch.encode_utf16(&mut units);
            if cursor + units.len() >= needle.len() {
                break;
            }
            needle[cursor..cursor + units.len()].copy_from_slice(units);
            cursor += units.len();
        }
        let mut replacement = vec![0u16; 4096];
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let data = Box::new(UnsafeCell::new(FINDREPLACEW {
            lStructSize: size_of::<FINDREPLACEW>() as u32,
            hwndOwner: owner,
            Flags: if options.direction == SearchDirection::Forward {
                FR_DOWN
            } else {
                0
            } | if options.match_case { FR_MATCHCASE } else { 0 }
                | if options.whole_word { FR_WHOLEWORD } else { 0 },
            lpstrFindWhat: needle.as_mut_ptr(),
            lpstrReplaceWith: replacement.as_mut_ptr(),
            wFindWhatLen: needle.len() as u16,
            wReplaceWithLen: replacement.len() as u16,
            lCustData: id as isize,
            ..Default::default()
        }));
        // SAFETY: Owned buffers and boxed descriptor never move or resize while
        // the native dialog retains their addresses.
        let hwnd = unsafe {
            if replace {
                ReplaceTextW(data.get())
            } else {
                FindTextW(data.get())
            }
        };
        if hwnd.is_null() {
            return Err(io::Error::other(format!(
                "Search dialog error: {}",
                unsafe { CommDlgExtendedError() }
            )));
        }
        let dialog = Self {
            hwnd,
            id,
            replace,
            _data: data,
            _needle: needle,
            _replacement: replacement,
        };
        // Keep the system dialog's keyboard and replacement behavior, adding
        // the same search navigation strip used by Find in Files.
        unsafe {
            use windows_sys::Win32::UI::Controls::*;
            EnumChildWindows(hwnd, Some(layout_child), isize::from(replace));
            let mut bounds = RECT {
                left: 0,
                top: 0,
                right: 660,
                bottom: 310,
            };
            AdjustWindowRectEx(
                &mut bounds,
                GetWindowLongPtrW(hwnd, GWL_STYLE) as u32,
                0,
                GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32,
            );
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOMOVE | SWP_NOZORDER,
            );
            let mut client = RECT::default();
            GetClientRect(hwnd, &mut client);
            let tabs = CreateWindowExW(
                0,
                WC_TABCONTROLW,
                windows_sys::core::w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                8,
                4,
                client.right - 16,
                28,
                hwnd,
                SEARCH_TABS as _,
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            SendMessageW(
                tabs,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as usize,
                1,
            );
            for (index, title) in ["Find", "Replace", "Find in Files"].iter().enumerate() {
                let mut title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: title.as_mut_ptr(),
                    ..Default::default()
                };
                SendMessageW(tabs, TCM_INSERTITEMW, index, &item as *const _ as isize);
            }
            SendMessageW(tabs, TCM_SETCURSEL, usize::from(replace), 0);
            for (class, title, id, x, y, width, style) in [
                ("STATIC", "Find what:", 630, 12, 54, 95, 0),
                ("STATIC", "Replace with:", 631, 12, 92, 95, 0),
                (
                    "BUTTON",
                    "Normal",
                    MODE_NORMAL,
                    12,
                    252,
                    90,
                    WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Extended",
                    MODE_EXTENDED,
                    110,
                    252,
                    110,
                    BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Regular expression",
                    MODE_REGEX,
                    230,
                    252,
                    185,
                    BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Wrap around",
                    WRAP,
                    12,
                    204,
                    200,
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                ),
                ("BUTTON", "Count", COUNT, 535, 204, 110, WS_TABSTOP),
            ] {
                if id == 631 && !replace {
                    continue;
                }
                let class: Vec<u16> = class.encode_utf16().chain(Some(0)).collect();
                let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
                let child = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    width,
                    26,
                    hwnd,
                    id as _,
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            CheckDlgButton(
                hwnd,
                WRAP as i32,
                if options.wrap {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                },
            );
        }
        // SAFETY: The descriptor's stable allocation is retained through Drop's DestroyWindow.
        if unsafe { SetWindowSubclass(hwnd, Some(close_proc), 2, dialog._data.get() as usize) } == 0
        {
            return Err(io::Error::last_os_error());
        }
        dialog.set_mode(options.mode);
        Ok(dialog)
    }
    pub fn show_existing(&self, replace: bool, initial: Option<&str>) -> bool {
        if self.hwnd.is_null() || self.replace != replace {
            return false;
        }
        if let Some(initial) = initial {
            self.set_query_text(initial);
        }
        // SAFETY: The live owned modeless dialog and its Find What edit are valid.
        unsafe {
            SendMessageW(
                GetDlgItem(self.hwnd, SEARCH_TABS as i32),
                windows_sys::Win32::UI::Controls::TCM_SETCURSEL,
                usize::from(replace),
                0,
            );
            ShowWindow(self.hwnd, SW_SHOW);
            SetForegroundWindow(self.hwnd);
            windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(
                self.hwnd,
                edt1 as i32,
            ));
        }
        true
    }

    pub fn is_replace(&self) -> bool {
        self.replace
    }
    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }
    pub fn set_wrap(&self, wrap: bool) {
        unsafe {
            windows_sys::Win32::UI::Controls::CheckDlgButton(
                self.hwnd,
                WRAP as i32,
                if wrap {
                    BST_CHECKED
                } else {
                    windows_sys::Win32::UI::Controls::BST_UNCHECKED
                },
            );
        }
    }

    pub fn set_replacement_text(&self, text: &str) {
        if self.hwnd.is_null() || !self.replace {
            return;
        }
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: The owned Replace dialog copies the terminated string.
        unsafe {
            SetDlgItemTextW(self.hwnd, edt2 as i32, text.as_ptr());
        }
    }

    pub fn set_query_text(&self, text: &str) {
        if self.hwnd.is_null() {
            return;
        }
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: The owned dialog's edit copies this temporary terminated text.
        unsafe {
            let edit = GetDlgItem(self.hwnd, edt1 as i32);
            SetWindowTextW(edit, text.as_ptr());
            SendMessageW(edit, EM_SETSEL, 0, -1);
        }
    }

    pub fn set_mode(&self, mode: SearchMode) {
        if self.hwnd.is_null() {
            return;
        }
        let title = format!(
            "{} — {}",
            if self.replace { "Replace" } else { "Find" },
            if mode == SearchMode::Extended {
                "Extended"
            } else if mode == SearchMode::Regex {
                "Regular expression"
            } else {
                "Normal"
            }
        );
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Live dialog copies the NUL-terminated temporary title.
        unsafe {
            EnableWindow(
                GetDlgItem(self.hwnd, chx1 as i32),
                i32::from(mode != SearchMode::Regex),
            );
            windows_sys::Win32::UI::Controls::CheckRadioButton(
                self.hwnd,
                MODE_NORMAL as i32,
                MODE_REGEX as i32,
                match mode {
                    SearchMode::Normal => MODE_NORMAL,
                    SearchMode::Extended => MODE_EXTENDED,
                    SearchMode::Regex => MODE_REGEX,
                } as i32,
            );
            SetWindowTextW(self.hwnd, title.as_ptr());
        }
    }
    pub fn set_read_only(&self, read_only: bool) -> io::Result<()> {
        if self.hwnd.is_null() || !self.replace {
            return Ok(());
        }
        // SAFETY: Subclass data is an integer, not a pointer into App. Reusing
        // this callback/ID updates its data without stacking subclasses.
        unsafe {
            if SetWindowSubclass(self.hwnd, Some(replace_proc), 1, usize::from(read_only)) == 0 {
                return Err(io::Error::other("Could not update Replace dialog state"));
            }
            replacement_buttons(self.hwnd, read_only);
        }
        Ok(())
    }
    pub fn terminated(&mut self) {
        self.hwnd = null_mut();
    }
    pub fn current_query(&self) -> Option<Request> {
        if self.hwnd.is_null() {
            return None;
        }
        // SAFETY: Read standard controls of this owned live dialog on its UI thread.
        // FINDREPLACE buffers only update on dialog actions, so read controls here.
        Some(unsafe {
            let read = |id| {
                let control = GetDlgItem(self.hwnd, id);
                let mut text = vec![0u16; GetWindowTextLengthW(control) as usize + 1];
                let length = GetWindowTextW(control, text.as_mut_ptr(), text.len() as i32);
                String::from_utf16_lossy(&text[..length as usize])
            };
            Request {
                id: self.id,
                flags: 0,
                needle: read(edt1 as i32),
                replacement: if self.replace {
                    read(edt2 as i32)
                } else {
                    String::new()
                },
                options: SearchOptions {
                    match_case: IsDlgButtonChecked(self.hwnd, chx2 as i32) == BST_CHECKED,
                    whole_word: IsDlgButtonChecked(self.hwnd, chx1 as i32) == BST_CHECKED,
                    direction: if IsDlgButtonChecked(self.hwnd, rad1 as i32) == BST_CHECKED {
                        SearchDirection::Backward
                    } else {
                        SearchDirection::Forward
                    },
                    ..Default::default()
                },
            }
        })
    }
    pub fn translate(&self, message: &MSG) -> bool {
        // SAFETY: Valid only until the dialog sends its termination notification.
        !self.hwnd.is_null() && unsafe { IsDialogMessageW(self.hwnd, message) } != 0
    }
}
impl Drop for SearchDialog {
    fn drop(&mut self) {
        // SAFETY: Close the dialog before freeing any retained buffers.
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
        }
    }
}
