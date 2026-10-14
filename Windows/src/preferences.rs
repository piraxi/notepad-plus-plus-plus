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

pub const NEW_ENCODINGS: &[(&str, npp_editor_core::Encoding)] = &[
    ("UTF-8", npp_editor_core::Encoding::Utf8),
    ("UTF-8 with BOM", npp_editor_core::Encoding::Utf8Bom),
    ("UTF-16 LE with BOM", npp_editor_core::Encoding::Utf16Le),
    ("UTF-16 BE with BOM", npp_editor_core::Encoding::Utf16Be),
    (
        "UTF-16 LE without BOM",
        npp_editor_core::Encoding::Utf16LeNoBom,
    ),
    (
        "UTF-16 BE without BOM",
        npp_editor_core::Encoding::Utf16BeNoBom,
    ),
    ("Windows-1252", npp_editor_core::Encoding::Windows1252),
    ("Windows-1251", npp_editor_core::Encoding::Windows1251),
    ("Shift-JIS", npp_editor_core::Encoding::ShiftJis),
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    #[serde(skip)]
    pub initial_page: usize,
    pub find_files_prefer_disk: bool,
    pub rendering_mode: usize,
    pub date_first: bool,
    pub date_format: String,
    pub tab_width: usize,
    pub spaces: bool,
    pub copy_cut_line: bool,
    pub double_click_close: bool,
    pub exit_last_tab: bool,
    pub lock_tab_drag: bool,
    pub multiline_tabs: bool,
    pub content_tab_names: bool,
    pub scroll_beyond_last_line: bool,
    pub caret_width: usize,
    pub caret_blink: usize,
    pub current_line_mode: usize,
    pub current_line_frame: usize,
    pub wrap_indent: usize,
    pub smooth_font: bool,
    pub right_click_keeps_selection: bool,
    pub disable_text_drag: bool,
    pub dynamic_line_numbers: bool,
    pub bookmark_margin: bool,
    pub distraction_divisor: usize,
    pub padding_left: usize,
    pub padding_right: usize,
    pub fold_symbols: usize,
    pub edge_columns: Vec<usize>,
    pub edge_background: bool,
    pub toggle_fold_commands: bool,
    pub short_title: bool,
    pub save_all_confirm: bool,
    pub multi_edit: bool,
    pub column_to_multi: bool,
    pub virtual_space: bool,
    pub backspace_unindent: bool,
    pub compact_tab_length: u32,
    pub recent_file_limit: usize,
    pub check_history_files: bool,
    pub recent_in_submenu: bool,
    pub selected_text_foreground: bool,
    pub disable_advanced_scrolling: bool,
    pub sql_backslash_escape: bool,
    pub compact_language_menu: bool,
    pub excluded_languages: Vec<u32>,
    pub recent_label_length: i32,
    pub tab_close_buttons: bool,
    pub inactive_tab_buttons: bool,
    pub search_engine: String,
    pub auto_indent: bool,
    pub advanced_indent: bool,
    pub new_document_eol: usize,
    pub new_document_encoding: usize,
    pub new_document_language: u32,
    pub word_wrap: bool,
    pub toolbar: bool,
    pub line_numbers: bool,
    pub status_bar: bool,
    pub tab_bar: bool,
    pub zoom: i32,
    pub show_whitespace: bool,
    pub show_non_printing: bool,
    pub show_controls: bool,
    pub show_eol: bool,
    pub indent_guides: bool,
    pub wrap_symbols: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            initial_page: 0,
            find_files_prefer_disk: false,
            rendering_mode: 0,
            date_first: false,
            date_format: crate::date_time::DEFAULT_CUSTOM_FORMAT.to_owned(),
            tab_width: 4,
            spaces: false,
            copy_cut_line: true,
            double_click_close: false,
            exit_last_tab: false,
            lock_tab_drag: false,
            multiline_tabs: false,
            content_tab_names: false,
            scroll_beyond_last_line: true,
            caret_width: 1,
            caret_blink: 600,
            current_line_mode: 1,
            current_line_frame: 1,
            wrap_indent: 1,
            smooth_font: false,
            right_click_keeps_selection: false,
            disable_text_drag: false,
            dynamic_line_numbers: true,
            bookmark_margin: true,
            distraction_divisor: 4,
            padding_left: 0,
            padding_right: 0,
            fold_symbols: 4,
            edge_columns: Vec::new(),
            edge_background: false,
            toggle_fold_commands: false,
            short_title: false,
            save_all_confirm: true,
            multi_edit: true,
            column_to_multi: true,
            virtual_space: false,
            backspace_unindent: false,
            compact_tab_length: 0,
            recent_file_limit: 10,
            check_history_files: false,
            recent_in_submenu: true,
            selected_text_foreground: false,
            disable_advanced_scrolling: false,
            sql_backslash_escape: true,
            compact_language_menu: true,
            excluded_languages: Vec::new(),
            recent_label_length: -1,
            tab_close_buttons: true,
            inactive_tab_buttons: false,
            search_engine: "https://www.google.com/search?q=$(CURRENT_WORD)".into(),
            auto_indent: true,
            advanced_indent: true,
            new_document_eol: 0,
            new_document_encoding: 0,
            new_document_language: npp_compat::commands::IDM_LANG_TEXT,
            word_wrap: false,
            toolbar: true,
            line_numbers: true,
            status_bar: true,
            tab_bar: true,
            zoom: 0,
            show_whitespace: false,
            show_non_printing: false,
            show_controls: true,
            show_eol: false,
            indent_guides: true,
            wrap_symbols: false,
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
fn show_page(hwnd: HWND, page: usize) {
    // Controls stay alive while hidden, retaining edits until OK/Cancel.
    let groups: &[&[i32]] = &[
        &[103, 105, 106, 116, 123, 124],
        &[102, 107, 115, 132, 214, 215, 216, 217, 222, 223],
        &[108, 109, 110, 111, 112, 113, 114, 202, 203],
        &[204, 205, 206, 207, 130, 131],
        &[210, 211, 212],
        &[218, 219, 220, 221],
        &[117, 118, 133],
        &[104, 119, 120, 224, 225, 226, 227, 228, 229, 122],
        &[230, 231, 121, 232, 233],
        &[125, 126, 127],
        &[100, 128, 200, 201, 234, 235, 236],
        &[237, 238, 239, 240, 241, 242, 243],
        &[134, 135, 244, 245, 246, 247, 248, 249],
        &[250, 251, 252, 253, 254],
        &[255, 256, 257],
        &[258, 259, 260],
    ];
    unsafe {
        for (group_index, ids) in groups.iter().enumerate() {
            let mut row = 0;
            for &id in *ids {
                let child = GetDlgItem(hwnd, id);
                if id == 126 {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                        child,
                        (IsDlgButtonChecked(hwnd, 125) == BST_CHECKED) as i32,
                    );
                }
                ShowWindow(
                    child,
                    if group_index == page {
                        SW_SHOWNA
                    } else {
                        SW_HIDE
                    },
                );
                if group_index == page {
                    let input =
                        ((200..=207).contains(&id) || (214..=229).contains(&id)) && id % 2 == 1;
                    if (210..=212).contains(&id) {
                        MoveWindow(
                            child,
                            180,
                            if id == 210 {
                                22
                            } else if id == 211 {
                                66
                            } else {
                                105
                            },
                            430,
                            if id == 211 { 160 } else { 26 },
                            1,
                        );
                        continue;
                    }
                    if (244..=249).contains(&id) {
                        let (x, y, width, height) = match id {
                            244 => (430, 88, 180, 24),
                            245 => (430, 112, 180, 174),
                            246 => (180, 88, 180, 24),
                            247 => (180, 112, 180, 174),
                            248 => (370, 150, 50, 28),
                            _ => (370, 192, 50, 28),
                        };
                        MoveWindow(child, x, y, width, height, 1);
                        continue;
                    }
                    MoveWindow(
                        child,
                        if input { 490 } else { 180 },
                        22 + row * 30,
                        if input {
                            115
                        } else if id >= 230 {
                            430
                        } else if id >= 200 {
                            300
                        } else {
                            430
                        },
                        if id == 245 {
                            170
                        } else if matches!(id, 215 | 219 | 223 | 229 | 233 | 241 | 243 | 256) {
                            180
                        } else {
                            26
                        },
                        1,
                    );
                    if !(((200..=207).contains(&id) || (214..=229).contains(&id)) && id % 2 == 0) {
                        row += 1;
                    }
                }
            }
        }
    }
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
                right: 640,
                bottom: 350,
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
                value.copy_cut_line,
                value.double_click_close,
                value.exit_last_tab,
                value.lock_tab_drag,
                value.multiline_tabs,
                value.tab_close_buttons,
                value.inactive_tab_buttons,
                value.content_tab_names,
                value.scroll_beyond_last_line,
                value.smooth_font,
                value.right_click_keeps_selection,
                value.disable_text_drag,
                value.dynamic_line_numbers,
                value.bookmark_margin,
                value.edge_background,
                value.toggle_fold_commands,
                value.short_title,
                value.save_all_confirm,
                value.multi_edit,
                value.column_to_multi,
                value.virtual_space,
                value.backspace_unindent,
                value.advanced_indent,
                value.check_history_files,
                value.recent_in_submenu,
                value.selected_text_foreground,
                value.disable_advanced_scrolling,
                value.sql_backslash_escape,
                value.compact_language_menu,
            ];
            for (index, label) in [
                "Replace tab with spaces",
                "Enable auto-indent",
                "Word wrap",
                "Show toolbar",
                "Show line numbers",
                "Show status bar",
                "Show tab bar",
                "Copy/Cut line when nothing is selected",
                "Close tab on double-click",
                "Exit when the last tab is closed",
                "Lock tab dragging",
                "Show tabs on multiple rows",
                "Show tab close buttons",
                "Show close buttons on inactive tabs",
                "Name untitled tabs from the first line",
                "Scroll beyond the last line",
                "Enable smooth font rendering",
                "Right-click keeps the current selection",
                "Disable dragging selected text",
                "Size line-number margin to visible lines",
                "Show bookmark margin",
                "Background beyond column (single guide only)",
                "Fold / Unfold Current Level toggle the block",
                "Show only filename in the window title",
                "Confirm before saving all documents",
                "Enable multiple selection",
                "Convert column selection to multiple carets",
                "Enable virtual space beyond line endings",
                "Backspace unindents to the previous tab stop",
                "Advanced language-aware indentation (otherwise Basic)",
                "Check recent files exist at startup",
                "Put recent files in a submenu",
                "Apply selection foreground color to selected text",
                "Disable advanced scrolling (touchpad compatibility)",
                "Treat backslash as an escape character in SQL",
                "Use compact alphabetical Language submenus",
            ]
            .iter()
            .enumerate()
            {
                if matches!(index, 1 | 29) {
                    continue;
                }
                let child = CreateWindowExW(
                    0,
                    w!("BUTTON"),
                    wide(label).as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    18,
                    18 + index as i32 * 26,
                    430,
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
                    420,
                    150,
                    0,
                ),
                (
                    "EDIT",
                    value.tab_width.to_string(),
                    201,
                    175,
                    418,
                    60,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Tab label limit (0–257):".to_owned(),
                    202,
                    245,
                    420,
                    160,
                    0,
                ),
                (
                    "EDIT",
                    value.compact_tab_length.to_string(),
                    203,
                    405,
                    418,
                    58,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Recent files (0–30):".to_owned(),
                    204,
                    18,
                    452,
                    150,
                    0,
                ),
                (
                    "EDIT",
                    value.recent_file_limit.to_string(),
                    205,
                    175,
                    450,
                    60,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Recent label (-1 full, 0 name):".to_owned(),
                    206,
                    245,
                    452,
                    200,
                    0,
                ),
                (
                    "EDIT",
                    value.recent_label_length.to_string(),
                    207,
                    405,
                    476,
                    58,
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                ),
                ("STATIC", "Caret style".to_owned(), 214, 0, 0, 300, 0),
                (
                    "COMBOBOX",
                    String::new(),
                    215,
                    0,
                    0,
                    100,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                (
                    "STATIC",
                    "Blink milliseconds (0 = no blinking)".to_owned(),
                    216,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "EDIT",
                    value.caret_blink.to_string(),
                    217,
                    0,
                    0,
                    100,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Current-line highlight".to_owned(),
                    218,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "COMBOBOX",
                    String::new(),
                    219,
                    0,
                    0,
                    100,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                (
                    "STATIC",
                    "Frame width (1–6 pixels)".to_owned(),
                    220,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "EDIT",
                    value.current_line_frame.to_string(),
                    221,
                    0,
                    0,
                    100,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Wrapped-line indentation".to_owned(),
                    222,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "COMBOBOX",
                    String::new(),
                    223,
                    0,
                    0,
                    100,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                (
                    "STATIC",
                    "Left text padding (0–9 pixels)".to_owned(),
                    224,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "EDIT",
                    value.padding_left.to_string(),
                    225,
                    0,
                    0,
                    100,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Right text padding (0–9 pixels)".to_owned(),
                    226,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "EDIT",
                    value.padding_right.to_string(),
                    227,
                    0,
                    0,
                    100,
                    WS_BORDER | WS_TABSTOP | ES_NUMBER as u32,
                ),
                (
                    "STATIC",
                    "Fold margin symbols".to_owned(),
                    228,
                    0,
                    0,
                    300,
                    0,
                ),
                (
                    "COMBOBOX",
                    String::new(),
                    229,
                    0,
                    0,
                    100,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                (
                    "STATIC",
                    "Columns (space-separated, 0–8192; empty disables)".to_owned(),
                    230,
                    0,
                    0,
                    430,
                    0,
                ),
                (
                    "EDIT",
                    value
                        .edge_columns
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                    231,
                    0,
                    0,
                    430,
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                ),
                (
                    "STATIC",
                    "Distraction Free margin: window width divided by".to_owned(),
                    232,
                    0,
                    0,
                    430,
                    0,
                ),
                (
                    "COMBOBOX",
                    String::new(),
                    233,
                    0,
                    0,
                    430,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                (
                    "BUTTON",
                    "Auto-indent: None".to_owned(),
                    234,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32 | WS_GROUP,
                ),
                (
                    "BUTTON",
                    "Auto-indent: Basic".to_owned(),
                    235,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Auto-indent: Advanced".to_owned(),
                    236,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Default line ending: Windows (CR LF)".to_owned(),
                    237,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32 | WS_GROUP,
                ),
                (
                    "BUTTON",
                    "Default line ending: Unix (LF)".to_owned(),
                    238,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Default line ending: Macintosh (CR)".to_owned(),
                    239,
                    0,
                    0,
                    430,
                    WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                ),
                ("STATIC", "Default encoding:".to_owned(), 240, 0, 0, 430, 0),
                (
                    "COMBOBOX",
                    String::new(),
                    241,
                    0,
                    0,
                    430,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                ("STATIC", "Default language:".to_owned(), 242, 0, 0, 430, 0),
                (
                    "COMBOBOX",
                    String::new(),
                    243,
                    0,
                    0,
                    430,
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                ),
                ("STATIC", "Disabled items".to_owned(), 244, 0, 0, 430, 0),
                (
                    "LISTBOX",
                    String::new(),
                    245,
                    0,
                    0,
                    430,
                    WS_BORDER | WS_TABSTOP | WS_VSCROLL | LBS_EXTENDEDSEL as u32 | LBS_SORT as u32,
                ),
                ("STATIC", "Available items".to_owned(), 246, 0, 0, 180, 0),
                (
                    "LISTBOX",
                    String::new(),
                    247,
                    0,
                    0,
                    180,
                    WS_BORDER | WS_TABSTOP | WS_VSCROLL | LBS_EXTENDEDSEL as u32 | LBS_SORT as u32,
                ),
                (
                    "BUTTON",
                    "->".to_owned(),
                    248,
                    0,
                    0,
                    50,
                    WS_TABSTOP | BS_PUSHBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "<-".to_owned(),
                    249,
                    0,
                    0,
                    50,
                    WS_TABSTOP | BS_PUSHBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "OK".to_owned(),
                    IDOK,
                    270,
                    520,
                    90,
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                ),
                (
                    "BUTTON",
                    "Cancel".to_owned(),
                    IDCANCEL,
                    372,
                    520,
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
            for (class, text, id, style) in [
                ("STATIC", "Find in Files:", 258, 0),
                (
                    "BUTTON",
                    "Prefer on-disk file content over open buffers",
                    259,
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                ),
                ("STATIC", "Unsaved changes remain in the editor.", 260, 0),
                ("STATIC", "Editor rendering mode:", 255, 0),
                (
                    "COMBOBOX",
                    "",
                    256,
                    CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP,
                ),
                (
                    "STATIC",
                    "Restart the application to apply this setting.",
                    257,
                    0,
                ),
                (
                    "BUTTON",
                    "Reverse default order (date before time)",
                    250,
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                ),
                (
                    "STATIC",
                    "Custom format (Windows date/time format):",
                    251,
                    0,
                ),
                (
                    "EDIT",
                    value.date_format.as_str(),
                    252,
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                ),
                ("STATIC", "Preview:", 253, 0),
                (
                    "EDIT",
                    "",
                    254,
                    WS_BORDER | WS_TABSTOP | ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
                ),
                (
                    "STATIC",
                    "Search URL: $(CURRENT_WORD) inserts the selected text",
                    210,
                    0,
                ),
                (
                    "COMBOBOX",
                    "",
                    211,
                    CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP,
                ),
                (
                    "EDIT",
                    value.search_engine.as_str(),
                    212,
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                ),
            ] {
                let child = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD | style,
                    0,
                    0,
                    430,
                    26,
                    hwnd,
                    id as HMENU,
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
            CheckDlgButton(
                hwnd,
                250,
                if value.date_first {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                },
            );
            SendMessageW(GetDlgItem(hwnd, 252), EM_SETLIMITTEXT, 128, 0);
            SetWindowTextW(
                GetDlgItem(hwnd, 254),
                wide(&crate::date_time::current_custom(&value.date_format)).as_ptr(),
            );
            for label in [
                "GDI (most compatible)",
                "DirectWrite",
                "DirectWrite (retain frames)",
                "DirectWrite (draw to GDI DC)",
                "DirectWrite (DirectX 11)",
            ] {
                SendMessageW(
                    GetDlgItem(hwnd, 256),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(
                GetDlgItem(hwnd, 256),
                CB_SETCURSEL,
                value.rendering_mode.min(4),
                0,
            );
            if crate::scintilla::direct_write_unavailable() {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    GetDlgItem(hwnd, 256),
                    0,
                );
                SetWindowTextW(
                    GetDlgItem(hwnd, 257),
                    w!("DirectWrite is unavailable here; GDI will be used."),
                );
            }
            CheckDlgButton(
                hwnd,
                259,
                if value.find_files_prefer_disk {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                },
            );
            let engines = crate::input_dialog::SEARCH_ENGINES;
            for (name, _) in engines {
                SendMessageW(
                    GetDlgItem(hwnd, 211),
                    CB_ADDSTRING,
                    0,
                    wide(name).as_ptr() as isize,
                );
            }
            let selected = engines
                .iter()
                .position(|(_, url)| *url == value.search_engine)
                .unwrap_or(engines.len() - 1);
            SendMessageW(GetDlgItem(hwnd, 211), CB_SETCURSEL, selected, 0);
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                GetDlgItem(hwnd, 212),
                (selected == engines.len() - 1) as i32,
            );
            for (id, selected, labels) in [
                (
                    215,
                    value.caret_width.min(5),
                    &[
                        "Invisible",
                        "1 pixel",
                        "2 pixels",
                        "3 pixels",
                        "Block",
                        "Block After",
                    ][..],
                ),
                (
                    219,
                    value.current_line_mode.min(2),
                    &["None", "Background", "Frame"][..],
                ),
                (
                    223,
                    value.wrap_indent.min(2),
                    &["Default", "Aligned", "Indented"][..],
                ),
            ] {
                let combo = GetDlgItem(hwnd, id);
                for label in labels {
                    SendMessageW(combo, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
                }
                SendMessageW(combo, CB_SETCURSEL, selected, 0);
            }
            for label in ["None", "Simple", "Arrow", "Circle", "Box"] {
                SendMessageW(
                    GetDlgItem(hwnd, 229),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(
                GetDlgItem(hwnd, 229),
                CB_SETCURSEL,
                value.fold_symbols.min(4),
                0,
            );
            for divisor in 3..=9 {
                SendMessageW(
                    GetDlgItem(hwnd, 233),
                    CB_ADDSTRING,
                    0,
                    wide(&divisor.to_string()).as_ptr() as isize,
                );
            }
            SendMessageW(
                GetDlgItem(hwnd, 233),
                CB_SETCURSEL,
                value.distraction_divisor.clamp(3, 9) - 3,
                0,
            );
            CheckRadioButton(
                hwnd,
                234,
                236,
                if !value.auto_indent {
                    234
                } else if value.advanced_indent {
                    236
                } else {
                    235
                },
            );
            CheckRadioButton(hwnd, 237, 239, 237 + value.new_document_eol.min(2) as i32);
            for (label, _) in NEW_ENCODINGS {
                SendMessageW(
                    GetDlgItem(hwnd, 241),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            SendMessageW(
                GetDlgItem(hwnd, 241),
                CB_SETCURSEL,
                value.new_document_encoding.min(NEW_ENCODINGS.len() - 1),
                0,
            );
            for language in crate::languages::LANGUAGES.iter() {
                SendMessageW(
                    GetDlgItem(hwnd, 243),
                    CB_ADDSTRING,
                    0,
                    wide(language.name.as_ref()).as_ptr() as isize,
                );
            }
            let selected = crate::languages::LANGUAGES
                .iter()
                .position(|language| language.command == value.new_document_language)
                .unwrap_or(0);
            SendMessageW(GetDlgItem(hwnd, 243), CB_SETCURSEL, selected, 0);
            for language in crate::languages::LANGUAGES.iter().skip(1) {
                let list = GetDlgItem(
                    hwnd,
                    if value.excluded_languages.contains(&language.command) {
                        245
                    } else {
                        247
                    },
                );
                let index = SendMessageW(
                    list,
                    LB_ADDSTRING,
                    0,
                    wide(&language.name).as_ptr() as isize,
                );
                if index >= 0 {
                    SendMessageW(
                        list,
                        LB_SETITEMDATA,
                        index as usize,
                        language.command as isize,
                    );
                }
            }
            let pages = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("LISTBOX"),
                null(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | LBS_NOTIFY as u32,
                12,
                16,
                150,
                280,
                hwnd,
                300usize as HMENU,
                GetModuleHandleW(null()),
                null(),
            );
            SendMessageW(
                pages,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as usize,
                1,
            );
            for name in [
                "General",
                "Editing",
                "Tab Bar",
                "Recent Files History",
                "Search Engine",
                "Current Line",
                "Mouse",
                "Margins",
                "Column Guides",
                "Multi-edit",
                "Indentation",
                "New Document",
                "Language",
                "Date & Time",
                "Rendering",
                "Searching",
            ] {
                SendMessageW(pages, LB_ADDSTRING, 0, wide(name).as_ptr() as isize);
            }
            SendMessageW(pages, LB_SETCURSEL, value.initial_page.min(15), 0);
            MoveWindow(GetDlgItem(hwnd, IDOK), 430, 310, 90, 28, 1);
            MoveWindow(GetDlgItem(hwnd, IDCANCEL), 532, 310, 90, 28, 1);
            show_page(hwnd, value.initial_page.min(15));
            return 1;
        }
        if message == WM_COMMAND && wp & 0xffff == 300 && (wp >> 16) as u32 == LBN_SELCHANGE {
            show_page(
                hwnd,
                SendMessageW(GetDlgItem(hwnd, 300), LB_GETCURSEL, 0, 0) as usize,
            );
            return 1;
        }
        if message == WM_COMMAND && wp & 0xffff == 252 && (wp >> 16) as u32 == EN_CHANGE {
            let mut buffer = [0u16; 129];
            let length = GetWindowTextW(
                GetDlgItem(hwnd, 252),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            );
            let format = String::from_utf16_lossy(&buffer[..length as usize]);
            SetWindowTextW(
                GetDlgItem(hwnd, 254),
                wide(&crate::date_time::current_custom(&format)).as_ptr(),
            );
            return 1;
        }
        if message == WM_COMMAND && matches!(wp & 0xffff, 248 | 249) {
            let (source, destination) = if wp & 0xffff == 248 {
                (247, 245)
            } else {
                (245, 247)
            };
            let source = GetDlgItem(hwnd, source);
            let destination = GetDlgItem(hwnd, destination);
            for index in (0..SendMessageW(source, LB_GETCOUNT, 0, 0)).rev() {
                if SendMessageW(source, LB_GETSEL, index as usize, 0) <= 0 {
                    continue;
                }
                let id = SendMessageW(source, LB_GETITEMDATA, index as usize, 0) as u32;
                if let Some(language) = crate::languages::by_command(id) {
                    let added = SendMessageW(
                        destination,
                        LB_ADDSTRING,
                        0,
                        wide(&language.name).as_ptr() as isize,
                    );
                    if added >= 0 {
                        SendMessageW(destination, LB_SETITEMDATA, added as usize, id as isize);
                        SendMessageW(source, LB_DELETESTRING, index as usize, 0);
                    }
                }
            }
            return 1;
        }
        if message == WM_COMMAND && wp & 0xffff == 125 {
            let enabled = IsDlgButtonChecked(hwnd, 125) == BST_CHECKED;
            if !enabled {
                CheckDlgButton(hwnd, 126, BST_UNCHECKED);
            }
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                GetDlgItem(hwnd, 126),
                enabled as i32,
            );
            return 1;
        }
        if message == WM_COMMAND && wp & 0xffff == 211 && (wp >> 16) as u32 == CBN_SELCHANGE {
            let selected = SendMessageW(GetDlgItem(hwnd, 211), CB_GETCURSEL, 0, 0) as usize;
            if let Some((_, url)) = crate::input_dialog::SEARCH_ENGINES.get(selected) {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    GetDlgItem(hwnd, 212),
                    url.is_empty() as i32,
                );
                if !url.is_empty() {
                    SetWindowTextW(GetDlgItem(hwnd, 212), wide(url).as_ptr());
                }
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
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 1, 0);
                show_page(hwnd, 1);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 201));
                MessageBoxW(
                    hwnd,
                    w!("Enter a tab width from 1 to 32."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let compact = GetDlgItemInt(hwnd, 203, &mut valid, 0);
            if valid == 0 || compact > 257 {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 2, 0);
                show_page(hwnd, 2);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 203));
                MessageBoxW(
                    hwnd,
                    w!("Tab label limit must be 0–257 (0 shows the full name)."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let recent_limit = GetDlgItemInt(hwnd, 205, &mut valid, 0);
            if valid == 0 || recent_limit > 30 {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 3, 0);
                show_page(hwnd, 3);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 205));
                MessageBoxW(
                    hwnd,
                    w!("Recent file count must be 0–30. Zero disables history."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let label_length = GetDlgItemInt(hwnd, 207, &mut valid, 1) as i32;
            if valid == 0 || !(-1..=259).contains(&label_length) {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 3, 0);
                show_page(hwnd, 3);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 207));
                MessageBoxW(
                    hwnd,
                    w!(
                        "Recent label: -1 for full path, 0 for filename, or 1–259 for a compact path."
                    ),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let edit = GetDlgItem(hwnd, 212);
            let mut buffer = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
            let length = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
            let url: String = String::from_utf16_lossy(&buffer[..length as usize])
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 4, 0);
                show_page(hwnd, 4);
                MessageBoxW(
                    hwnd,
                    w!("Enter an HTTP or HTTPS search URL."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let caret_width = SendMessageW(GetDlgItem(hwnd, 215), CB_GETCURSEL, 0, 0);
            if !(0..=5).contains(&caret_width) {
                MessageBoxW(
                    hwnd,
                    w!("Caret style must be 0–5."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let caret_blink = GetDlgItemInt(hwnd, 217, &mut valid, 0);
            if valid == 0 || caret_blink > 2000 {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 1, 0);
                show_page(hwnd, 1);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 217));
                MessageBoxW(
                    hwnd,
                    w!("Blink interval must be 0–2000 milliseconds."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let line_mode = SendMessageW(GetDlgItem(hwnd, 219), CB_GETCURSEL, 0, 0);
            if !(0..=2).contains(&line_mode) {
                MessageBoxW(
                    hwnd,
                    w!("Current-line mode must be 0–2."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let line_frame = GetDlgItemInt(hwnd, 221, &mut valid, 0);
            if valid == 0 || !(1..=6).contains(&line_frame) {
                SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 5, 0);
                show_page(hwnd, 5);
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(hwnd, 221));
                MessageBoxW(
                    hwnd,
                    w!("Frame width must be 1–6 pixels."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let wrap_indent = SendMessageW(GetDlgItem(hwnd, 223), CB_GETCURSEL, 0, 0);
            if !(0..=2).contains(&wrap_indent) {
                MessageBoxW(
                    hwnd,
                    w!("Wrap indentation mode must be 0–2."),
                    w!("Preferences"),
                    MB_OK,
                );
                return 1;
            }
            let mut padding = [0u32; 2];
            for (index, id) in [225, 227].into_iter().enumerate() {
                padding[index] = GetDlgItemInt(hwnd, id, &mut valid, 0);
                if valid == 0 || padding[index] > 9 {
                    SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 7, 0);
                    show_page(hwnd, 7);
                    MessageBoxW(
                        hwnd,
                        w!("Text padding must be 0–9 pixels."),
                        w!("Preferences"),
                        MB_OK,
                    );
                    return 1;
                }
            }
            let edge_edit = GetDlgItem(hwnd, 231);
            let mut edge_text = vec![0u16; GetWindowTextLengthW(edge_edit) as usize + 1];
            let length = GetWindowTextW(edge_edit, edge_text.as_mut_ptr(), edge_text.len() as i32);
            let edge_text = String::from_utf16_lossy(&edge_text[..length as usize]);
            let columns: Result<Vec<usize>, _> =
                edge_text.split_whitespace().map(str::parse).collect();
            let mut columns = match columns {
                Ok(columns) if columns.iter().all(|&column| column <= 8192) => columns,
                _ => {
                    SendMessageW(GetDlgItem(hwnd, 300), LB_SETCURSEL, 8, 0);
                    show_page(hwnd, 8);
                    MessageBoxW(
                        hwnd,
                        w!("Enter space-separated column numbers from 0 to 8192."),
                        w!("Preferences"),
                        MB_OK,
                    );
                    return 1;
                }
            };
            columns.sort_unstable();
            columns.dedup();
            let value = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Preferences);
            value.find_files_prefer_disk = IsDlgButtonChecked(hwnd, 259) == BST_CHECKED;
            value.rendering_mode =
                (SendMessageW(GetDlgItem(hwnd, 256), CB_GETCURSEL, 0, 0).max(0) as usize).min(4);
            value.date_first = IsDlgButtonChecked(hwnd, 250) == BST_CHECKED;
            let mut date_format = [0u16; 129];
            let length = GetWindowTextW(
                GetDlgItem(hwnd, 252),
                date_format.as_mut_ptr(),
                date_format.len() as i32,
            );
            value.date_format = String::from_utf16_lossy(&date_format[..length as usize]);
            value.edge_columns = columns;
            value.edge_background = IsDlgButtonChecked(hwnd, 121) == BST_CHECKED;
            value.toggle_fold_commands = IsDlgButtonChecked(hwnd, 122) == BST_CHECKED;
            value.short_title = IsDlgButtonChecked(hwnd, 123) == BST_CHECKED;
            value.save_all_confirm = IsDlgButtonChecked(hwnd, 124) == BST_CHECKED;
            value.multi_edit = IsDlgButtonChecked(hwnd, 125) == BST_CHECKED;
            value.column_to_multi =
                value.multi_edit && IsDlgButtonChecked(hwnd, 126) == BST_CHECKED;
            value.virtual_space = IsDlgButtonChecked(hwnd, 127) == BST_CHECKED;
            value.backspace_unindent = IsDlgButtonChecked(hwnd, 128) == BST_CHECKED;
            value.advanced_indent = IsDlgButtonChecked(hwnd, 236) == BST_CHECKED;
            value.distraction_divisor =
                SendMessageW(GetDlgItem(hwnd, 233), CB_GETCURSEL, 0, 0).clamp(0, 6) as usize + 3;
            value.padding_left = padding[0] as usize;
            value.padding_right = padding[1] as usize;
            value.fold_symbols =
                SendMessageW(GetDlgItem(hwnd, 229), CB_GETCURSEL, 0, 0).clamp(0, 4) as usize;
            value.wrap_indent = wrap_indent as usize;
            value.current_line_mode = line_mode as usize;
            value.current_line_frame = line_frame as usize;
            value.caret_width = caret_width as usize;
            value.caret_blink = caret_blink as usize;
            value.search_engine = url;
            value.recent_label_length = label_length;
            value.recent_file_limit = recent_limit as usize;
            value.compact_tab_length = compact;
            let checked = |id| IsDlgButtonChecked(hwnd, id) == BST_CHECKED;
            value.tab_width = width as usize;
            value.spaces = checked(100);
            value.new_document_encoding = SendMessageW(GetDlgItem(hwnd, 241), CB_GETCURSEL, 0, 0)
                .clamp(0, (NEW_ENCODINGS.len() - 1) as isize)
                as usize;
            let language = SendMessageW(GetDlgItem(hwnd, 243), CB_GETCURSEL, 0, 0).max(0) as usize;
            value.new_document_language = crate::languages::LANGUAGES
                .get(language)
                .unwrap_or(&crate::languages::LANGUAGES[0])
                .command;
            value.check_history_files = checked(130);
            value.recent_in_submenu = checked(131);
            value.selected_text_foreground = checked(132);
            value.disable_advanced_scrolling = checked(133);
            value.sql_backslash_escape = checked(134);
            value.compact_language_menu = checked(135);
            value.excluded_languages = (0..SendMessageW(GetDlgItem(hwnd, 245), LB_GETCOUNT, 0, 0))
                .map(|index| {
                    SendMessageW(GetDlgItem(hwnd, 245), LB_GETITEMDATA, index as usize, 0) as u32
                })
                .collect();
            value.excluded_languages.sort_unstable();
            value.auto_indent = !checked(234);
            value.new_document_eol = if checked(238) {
                1
            } else if checked(239) {
                2
            } else {
                0
            };
            value.word_wrap = checked(102);
            value.toolbar = checked(103);
            value.line_numbers = checked(104);
            value.status_bar = checked(105);
            value.tab_bar = checked(106);
            value.copy_cut_line = checked(107);
            value.double_click_close = checked(108);
            value.exit_last_tab = checked(109);
            value.lock_tab_drag = checked(110);
            value.multiline_tabs = checked(111);
            value.tab_close_buttons = checked(112);
            value.inactive_tab_buttons = checked(113);
            value.content_tab_names = checked(114);
            value.scroll_beyond_last_line = checked(115);
            value.smooth_font = checked(116);
            value.right_click_keeps_selection = checked(117);
            value.disable_text_drag = checked(118);
            value.dynamic_line_numbers = checked(119);
            value.bookmark_margin = checked(120);
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
