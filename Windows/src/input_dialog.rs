use std::{
    io,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{
        Controls::*,
        Input::KeyboardAndMouse::{EnableWindow, SetFocus},
        WindowsAndMessaging::*,
    },
};

struct Input {
    title: String,
    label: String,
    value: String,
    accepted: bool,
    multiline: bool,
    search_engine: bool,
}
pub(crate) const SEARCH_ENGINES: &[(&str, &str)] = &[
    ("Google", "https://www.google.com/search?q=$(CURRENT_WORD)"),
    ("DuckDuckGo", "https://duckduckgo.com/?q=$(CURRENT_WORD)"),
    (
        "Yahoo!",
        "https://search.yahoo.com/search?q=$(CURRENT_WORD)",
    ),
    (
        "Stack Overflow",
        "https://stackoverflow.com/search?q=$(CURRENT_WORD)",
    ),
    ("Custom", ""),
];
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    // SAFETY: The modal call retains the Input allocation until EndDialog.
    unsafe {
        if msg == WM_INITDIALOG {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
            let data = &*(lp as *const Input);
            SetWindowTextW(hwnd, wide(&data.title).as_ptr());
            let mut bounds = RECT {
                left: 0,
                top: 0,
                right: 560,
                bottom: if data.multiline {
                    420
                } else if data.search_engine {
                    200
                } else {
                    148
                },
            };
            AdjustWindowRectEx(&mut bounds, WS_POPUP | WS_CAPTION | WS_SYSMENU, 0, 0);
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOMOVE | SWP_NOZORDER,
            );
            for (class, label, id, x, y, width, height, style) in [
                ("STATIC", data.label.as_str(), 100, 12, 12, 536, 38, 0),
                (
                    "EDIT",
                    data.value.as_str(),
                    101,
                    12,
                    54,
                    536,
                    if data.multiline { 290 } else { 26 },
                    WS_BORDER
                        | WS_TABSTOP
                        | ES_AUTOHSCROLL as u32
                        | if data.multiline {
                            WS_VSCROLL | WS_HSCROLL | ES_MULTILINE as u32 | ES_READONLY as u32
                        } else {
                            0
                        },
                ),
                (
                    "BUTTON",
                    "OK",
                    IDOK,
                    352,
                    if data.multiline { 374 } else { 102 },
                    92,
                    28,
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Cancel",
                    IDCANCEL,
                    456,
                    if data.multiline { 374 } else { 102 },
                    92,
                    28,
                    WS_TABSTOP,
                ),
            ] {
                let child = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    width,
                    height,
                    hwnd,
                    id as usize as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            if data.search_engine {
                let combo = CreateWindowExW(
                    0,
                    wide("COMBOBOX").as_ptr(),
                    null(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
                    12,
                    148,
                    330,
                    180,
                    hwnd,
                    102usize as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                SendMessageW(
                    combo,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
                for (name, _) in SEARCH_ENGINES {
                    SendMessageW(combo, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
                }
                let selected = SEARCH_ENGINES
                    .iter()
                    .position(|(_, url)| *url == data.value)
                    .unwrap_or(SEARCH_ENGINES.len() - 1);
                SendMessageW(combo, CB_SETCURSEL, selected, 0);
                EnableWindow(
                    GetDlgItem(hwnd, 101),
                    (selected == SEARCH_ENGINES.len() - 1) as i32,
                );
            }
            SetFocus(GetDlgItem(hwnd, if data.search_engine { 102 } else { 101 }));
            SendMessageW(GetDlgItem(hwnd, 101), EM_SETSEL, 0, -1);
            return 0;
        }
        if msg == WM_COMMAND && wp & 0xffff == 102 && (wp >> 16) as u32 == CBN_SELCHANGE {
            let selected = SendMessageW(GetDlgItem(hwnd, 102), CB_GETCURSEL, 0, 0) as usize;
            if let Some((_, url)) = SEARCH_ENGINES.get(selected) {
                EnableWindow(GetDlgItem(hwnd, 101), url.is_empty() as i32);
                if !url.is_empty() {
                    SetWindowTextW(GetDlgItem(hwnd, 101), wide(url).as_ptr());
                }
            }
            return 1;
        }
        if msg == WM_CLOSE || (msg == WM_COMMAND && wp & 0xffff == IDCANCEL as usize) {
            EndDialog(hwnd, 0);
            return 1;
        }
        if msg == WM_COMMAND && wp & 0xffff == IDOK as usize {
            let data = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Input);
            let child = GetDlgItem(hwnd, 101);
            let mut value = vec![0u16; GetWindowTextLengthW(child) as usize + 1];
            let length = GetWindowTextW(child, value.as_mut_ptr(), value.len() as i32);
            data.value = String::from_utf16_lossy(&value[..length as usize]);
            data.accepted = true;
            EndDialog(hwnd, 1);
            return 1;
        }
        0
    }
}

pub fn ask(owner: HWND, title: &str, label: &str, value: &str) -> io::Result<Option<String>> {
    show(owner, title, label, value, false, false)
}

pub fn results(owner: HWND, title: &str, label: &str, value: &str) -> io::Result<()> {
    show(owner, title, label, value, true, false).map(|_| ())
}

fn show(
    owner: HWND,
    title: &str,
    label: &str,
    value: &str,
    multiline: bool,
    search_engine: bool,
) -> io::Result<Option<String>> {
    #[repr(C, align(4))]
    struct Template {
        header: DLGTEMPLATE,
        tail: [u16; 3],
    }
    let template = Template {
        header: DLGTEMPLATE {
            style: WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
            x: 40,
            y: 40,
            cx: 300,
            cy: 90,
            ..Default::default()
        },
        tail: [0; 3],
    };
    let mut data = Input {
        title: title.into(),
        label: label.into(),
        value: value.into(),
        accepted: false,
        multiline,
        search_engine,
    };
    let result = unsafe {
        DialogBoxIndirectParamW(
            GetModuleHandleW(null()),
            &template.header,
            owner,
            Some(procedure),
            &mut data as *mut _ as isize,
        )
    };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(data.accepted.then_some(data.value))
    }
}
