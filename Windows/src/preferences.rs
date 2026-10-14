use serde::{Deserialize, Serialize};
use std::{
    io,
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::*, WindowsAndMessaging::*},
    },
    core::w,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub tab_width: usize,
    pub spaces: bool,
    pub auto_indent: bool,
    pub word_wrap: bool,
    pub toolbar: bool,
    pub line_numbers: bool,
    pub status_bar: bool,
    pub tab_bar: bool,
    pub zoom: i32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            tab_width: 4,
            spaces: false,
            auto_indent: true,
            word_wrap: false,
            toolbar: true,
            line_numbers: true,
            status_bar: true,
            tab_bar: true,
            zoom: 0,
        }
    }
}
fn path() -> io::Result<std::path::PathBuf> {
    let root =
        std::env::var_os("APPDATA").ok_or_else(|| io::Error::other("APPDATA is not available"))?;
    Ok(std::path::PathBuf::from(root)
        .join("NotepadPlusPlusRust")
        .join("preferences.json"))
}
pub fn load() -> io::Result<Preferences> {
    serde_json::from_slice(&std::fs::read(path()?)?).map_err(io::Error::other)
}
pub fn save(value: &Preferences) -> io::Result<()> {
    let path = path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    crate::file_io::atomic_write(
        &path,
        &serde_json::to_vec_pretty(value).map_err(io::Error::other)?,
    )
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> isize {
    unsafe {
        if message == WM_INITDIALOG {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lp);
            let value = &*(lp as *const Preferences);
            SetWindowTextW(hwnd, w!("Preferences"));
            let mut bounds = RECT {
                left: 0,
                top: 0,
                right: 480,
                bottom: 330,
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
            let values = [
                value.spaces,
                value.auto_indent,
                value.word_wrap,
                value.toolbar,
                value.line_numbers,
                value.status_bar,
                value.tab_bar,
            ];
            for (index, label) in [
                "Replace tab with spaces",
                "Language-aware auto-indent",
                "Word wrap",
                "Show toolbar",
                "Show line numbers",
                "Show status bar",
                "Show tab bar",
            ]
            .iter()
            .enumerate()
            {
                let child = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    wide(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    18,
                    18 + index as i32 * 30,
                    300,
                    25,
                    hwnd,
                    (100 + index) as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
                CheckDlgButton(
                    hwnd,
                    100 + index as i32,
                    if values[index] {
                        BST_CHECKED
                    } else {
                        BST_UNCHECKED
                    },
                );
            }
            for (class, title, id, x, y, width, style) in [
                (
                    "STATIC",
                    "Tab width (1–32):".to_owned(),
                    200,
                    18,
                    240,
                    150,
                    0,
                ),
                (
                    "EDIT",
                    value.tab_width.to_string(),
                    201,
                    175,
                    238,
                    60,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "BUTTON",
                    "OK".to_owned(),
                    IDOK,
                    270,
                    286,
                    90,
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Cancel".to_owned(),
                    IDCANCEL,
                    372,
                    286,
                    90,
                    WS_TABSTOP,
                ),
            ] {
                let child = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(&title).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    width,
                    26,
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
            return 1;
        }
        if message == WM_CLOSE || (message == WM_COMMAND && wp & 0xffff == IDCANCEL as usize) {
            EndDialog(hwnd, 0);
            return 1;
        }
        if message == WM_COMMAND && wp & 0xffff == IDOK as usize {
            let mut valid = 0;
            let width = GetDlgItemInt(hwnd, 201, &mut valid, 0);
            if valid == 0 || !(1..=32).contains(&width) {
                MessageBoxW(
                    hwnd,
                    w!("Enter a tab width from 1 to 32."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let value = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Preferences);
            let checked = |id| IsDlgButtonChecked(hwnd, id) == BST_CHECKED;
            value.tab_width = width as usize;
            value.spaces = checked(100);
            value.auto_indent = checked(101);
            value.word_wrap = checked(102);
            value.toolbar = checked(103);
            value.line_numbers = checked(104);
            value.status_bar = checked(105);
            value.tab_bar = checked(106);
            EndDialog(hwnd, 1);
            return 1;
        }
        0
    }
}
pub fn show(owner: HWND, mut value: Preferences) -> io::Result<Option<Preferences>> {
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
            cy: 200,
            ..Default::default()
        },
        tail: [0; 3],
    };
    let result = unsafe {
        DialogBoxIndirectParamW(
            GetModuleHandleW(null()),
            &template.header,
            owner,
            Some(procedure),
            &mut value as *mut _ as isize,
        )
    };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok((result == 1).then_some(value))
    }
}
