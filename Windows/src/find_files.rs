use crate::app::{Event, enqueue};
use npp_editor_core::{
    SearchMode, SearchOptions,
    find_files::{self, FileSearchEvent, FileSearchHit, FileSearchQuery, OpenFileContents},
};
use std::{
    io,
    path::{Path, PathBuf},
    ptr::null,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
            LibraryLoader::*,
        },
        UI::{Controls::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub const RESULTS: i32 = 104;
pub const RESULTS_LABEL: i32 = 105;
pub const RESULTS_CLOSE: i32 = 106;
pub const TIMER: usize = 771;
const FIND: i32 = 501;
const FILTER: i32 = 502;
const DIRECTORY: i32 = 503;
const CASE: i32 = 504;
const WORD: i32 = 505;
const RECURSIVE: i32 = 506;
const HIDDEN: i32 = 507;
const NORMAL: i32 = 508;
const EXTENDED: i32 = 509;
const REGEX: i32 = 510;
const DOT: i32 = 511;
const BROWSE: i32 = 512;
const CURRENT_DIRECTORY: i32 = 513;
const STOP: i32 = 514;
const SEARCH_TABS: i32 = 515;
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn text(hwnd: HWND) -> String {
    // SAFETY: Owned live text control, output includes a terminator.
    unsafe {
        let mut buffer = vec![0; GetWindowTextLengthW(hwnd) as usize + 1];
        let length = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}
pub fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
    }
}

unsafe extern "system" fn results_owner_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        if message == WM_DRAWITEM && wp == RESULTS as usize && lp != 0 {
            let item = &*(lp as *const DRAWITEMSTRUCT);
            if item.itemID == u32::MAX {
                return 1;
            }
            let len = SendMessageW(item.hwndItem, LB_GETTEXTLEN, item.itemID as usize, 0);
            if len < 0 {
                return 1;
            }
            let mut buffer = vec![0u16; len as usize + 1];
            SendMessageW(
                item.hwndItem,
                LB_GETTEXT,
                item.itemID as usize,
                buffer.as_mut_ptr() as isize,
            );
            let selected = item.itemState & ODS_SELECTED != 0;
            let background = if selected {
                GetSysColor(COLOR_HIGHLIGHT)
            } else if item.itemData == 0 {
                0x00f5eee5
            } else {
                GetSysColor(COLOR_WINDOW)
            };
            let brush = CreateSolidBrush(background);
            FillRect(item.hDC, &item.rcItem, brush);
            DeleteObject(brush);
            SetBkMode(item.hDC, TRANSPARENT as i32);
            SetTextColor(
                item.hDC,
                if selected {
                    GetSysColor(COLOR_HIGHLIGHTTEXT)
                } else if item.itemData == 0 {
                    0x00604020
                } else {
                    GetSysColor(COLOR_WINDOWTEXT)
                },
            );
            let font = SendMessageW(item.hwndItem, WM_GETFONT, 0, 0);
            let previous = SelectObject(item.hDC, font as HGDIOBJ);
            let mut bounds = item.rcItem;
            bounds.left += 5;
            DrawTextW(
                item.hDC,
                buffer.as_ptr(),
                len as i32,
                &mut bounds,
                DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
            );
            SelectObject(item.hDC, previous);
            if item.itemState & ODS_FOCUS != 0 {
                DrawFocusRect(item.hDC, &item.rcItem);
            }
            return 1;
        }
        DefSubclassProc(hwnd, message, wp, lp)
    }
}
unsafe extern "system" fn dialog_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> isize {
    // SAFETY: All IDs below identify controls owned by this live dialog.
    unsafe {
        if message == WM_CLOSE {
            ShowWindow(hwnd, SW_HIDE);
            return 1;
        }
        if message == WM_NOTIFY && lp != 0 {
            let header = &*(lp as *const NMHDR);
            if header.idFrom == SEARCH_TABS as usize && header.code == TCN_SELCHANGE {
                let tab = SendMessageW(header.hwndFrom, TCM_GETCURSEL, 0, 0);
                if tab < 2 {
                    ShowWindow(hwnd, SW_HIDE);
                    enqueue(Event::SearchTab(
                        if tab == 0 {
                            npp_compat::commands::IDM_SEARCH_FIND
                        } else {
                            npp_compat::commands::IDM_SEARCH_REPLACE
                        },
                        text(GetDlgItem(hwnd, FIND)),
                    ));
                    SendMessageW(header.hwndFrom, TCM_SETCURSEL, 2, 0);
                }
            }
        }
        if message == WM_COMMAND {
            match (wp & 0xffff) as i32 {
                IDOK => {
                    let checked = |id| IsDlgButtonChecked(hwnd, id) == BST_CHECKED;
                    let query = FileSearchQuery {
                        needle: text(GetDlgItem(hwnd, FIND)),
                        directory: PathBuf::from(text(GetDlgItem(hwnd, DIRECTORY))),
                        filters: text(GetDlgItem(hwnd, FILTER)),
                        recursive: checked(RECURSIVE),
                        include_hidden: checked(HIDDEN),
                        regex: checked(REGEX),
                        dot_matches_newline: checked(DOT),
                        options: SearchOptions {
                            mode: if checked(EXTENDED) {
                                SearchMode::Extended
                            } else {
                                SearchMode::Normal
                            },
                            match_case: checked(CASE),
                            whole_word: checked(WORD),
                            ..SearchOptions::default()
                        },
                    };
                    enqueue(Event::FindFiles(query));
                    return 1;
                }
                IDCANCEL => {
                    ShowWindow(hwnd, SW_HIDE);
                    return 1;
                }
                STOP => {
                    enqueue(Event::CancelFindFiles);
                    return 1;
                }
                CURRENT_DIRECTORY => {
                    enqueue(Event::FindFilesDirectory);
                    return 1;
                }
                BROWSE => {
                    let initialized = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
                    let info = BROWSEINFOW {
                        hwndOwner: hwnd,
                        lpszTitle: w!("Choose search directory"),
                        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
                        ..Default::default()
                    };
                    let item = SHBrowseForFolderW(&info);
                    if !item.is_null() {
                        let mut path = [0u16; 32768];
                        if SHGetPathFromIDListEx(item, path.as_mut_ptr(), path.len() as u32, 0) != 0
                        {
                            SetDlgItemTextW(hwnd, DIRECTORY, path.as_ptr());
                        }
                        CoTaskMemFree(item.cast());
                    }
                    if initialized >= 0 {
                        CoUninitialize();
                    }
                    return 1;
                }
                NORMAL | EXTENDED | REGEX => {
                    CheckRadioButton(hwnd, NORMAL, REGEX, (wp & 0xffff) as i32);
                    EnableWindow(
                        GetDlgItem(hwnd, WORD),
                        ((wp & 0xffff) as i32 != REGEX) as i32,
                    );
                    EnableWindow(
                        GetDlgItem(hwnd, DOT),
                        ((wp & 0xffff) as i32 == REGEX) as i32,
                    );
                    return 1;
                }
                _ => (),
            }
        }
        0
    }
}
unsafe extern "system" fn result_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    // SAFETY: Subclass is attached to the live result list on its owning thread.
    unsafe {
        if message == WM_KEYDOWN && wp == VK_RETURN as usize {
            enqueue(Event::OpenFindFileResult(
                SendMessageW(hwnd, LB_GETCURSEL, 0, 0) as usize,
            ));
            return 0;
        }
        if message == WM_KEYDOWN && wp == VK_ESCAPE as usize {
            enqueue(Event::HideFindFilesResults);
            return 0;
        }
        if message == WM_CONTEXTMENU {
            let menu = CreatePopupMenu();
            for (id, label) in [
                (1, "Copy Selected Line"),
                (2, "Copy All Results"),
                (3, "Clear All Results"),
                (4, "Stop Search"),
                (5, "Hide Search Results"),
            ] {
                AppendMenuW(menu, MF_STRING, id, wide(label).as_ptr());
            }
            let mut position = POINT::default();
            GetCursorPos(&mut position);
            let action = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                position.x,
                position.y,
                0,
                hwnd,
                null(),
            );
            DestroyMenu(menu);
            match action {
                1 => enqueue(Event::CopyFindFilesResults(false)),
                2 => enqueue(Event::CopyFindFilesResults(true)),
                3 => enqueue(Event::ClearFindFilesResults),
                4 => enqueue(Event::CancelFindFiles),
                5 => enqueue(Event::HideFindFilesResults),
                _ => (),
            }
            return 0;
        }
        DefSubclassProc(hwnd, message, wp, lp)
    }
}
pub struct FindFiles {
    results_font: HFONT,
    pub dialog: HWND,
    pub list: HWND,
    label: HWND,
    owner: HWND,
    receiver: Option<Receiver<FileSearchEvent>>,
    cancel: Arc<AtomicBool>,
    rows: Vec<Option<FileSearchHit>>,
    text_rows: Vec<String>,
    errors: usize,
}
impl FindFiles {
    pub fn new(owner: HWND) -> io::Result<Self> {
        #[repr(C, align(4))]
        struct Template {
            header: DLGTEMPLATE,
            tail: [u16; 3],
        }
        let template = Template {
            header: DLGTEMPLATE {
                style: WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
                cx: 370,
                cy: 165,
                ..Default::default()
            },
            tail: [0; 3],
        };
        // SAFETY: Template lives through creation; native windows own all children.
        unsafe {
            let instance = GetModuleHandleW(null());
            let dialog =
                CreateDialogIndirectParamW(instance, &template.header, owner, Some(dialog_proc), 0);
            if dialog.is_null() {
                return Err(io::Error::last_os_error());
            }
            SetWindowTextW(dialog, w!("Find in Files"));
            // The template uses dialog units; child positions below use pixels.
            // Set the client extent explicitly so font-dependent dialog units
            // cannot clip the buttons or search-mode controls.
            let mut bounds = RECT {
                left: 0,
                top: 0,
                right: 660,
                bottom: 310,
            };
            AdjustWindowRectEx(
                &mut bounds,
                (WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32) as _,
                0,
                0,
            );
            let mut parent = RECT::default();
            GetWindowRect(owner, &mut parent);
            SetWindowPos(
                dialog,
                std::ptr::null_mut(),
                parent.left + 80,
                parent.top + 100,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let tabs = CreateWindowExW(
                0,
                WC_TABCONTROLW,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                8,
                8,
                644,
                28,
                dialog,
                SEARCH_TABS as usize as HMENU,
                instance,
                null(),
            );
            SendMessageW(
                tabs,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as usize,
                1,
            );
            for (index, title) in ["Find", "Replace", "Find in Files"].iter().enumerate() {
                let mut label = wide(title);
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: label.as_mut_ptr(),
                    ..Default::default()
                };
                SendMessageW(tabs, TCM_INSERTITEMW, index, &item as *const _ as isize);
            }
            SendMessageW(tabs, TCM_SETCURSEL, 2, 0);
            let controls = [
                ("STATIC", "Find what:", 0, 12, 14, 90, 24, 520),
                (
                    "COMBOBOX",
                    "",
                    CBS_DROPDOWN as u32 | CBS_AUTOHSCROLL as u32 | WS_VSCROLL | WS_TABSTOP,
                    110,
                    12,
                    390,
                    25,
                    FIND,
                ),
                ("STATIC", "Filters:", 0, 12, 47, 90, 24, 521),
                (
                    "COMBOBOX",
                    "*.*",
                    CBS_DROPDOWN as u32 | CBS_AUTOHSCROLL as u32 | WS_VSCROLL | WS_TABSTOP,
                    110,
                    45,
                    390,
                    25,
                    FILTER,
                ),
                ("STATIC", "Directory:", 0, 12, 80, 90, 24, 522),
                (
                    "COMBOBOX",
                    "",
                    CBS_DROPDOWN as u32 | CBS_AUTOHSCROLL as u32 | WS_VSCROLL | WS_TABSTOP,
                    110,
                    78,
                    295,
                    25,
                    DIRECTORY,
                ),
                ("BUTTON", "Browse…", WS_TABSTOP, 415, 78, 85, 25, BROWSE),
                (
                    "BUTTON",
                    "Current document directory",
                    WS_TABSTOP,
                    110,
                    110,
                    220,
                    25,
                    CURRENT_DIRECTORY,
                ),
                (
                    "BUTTON",
                    "Match case",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    12,
                    146,
                    125,
                    24,
                    CASE,
                ),
                (
                    "BUTTON",
                    "Whole word only",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    145,
                    146,
                    145,
                    24,
                    WORD,
                ),
                (
                    "BUTTON",
                    "In all sub-folders",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    12,
                    174,
                    145,
                    24,
                    RECURSIVE,
                ),
                (
                    "BUTTON",
                    "In hidden folders",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    175,
                    174,
                    145,
                    24,
                    HIDDEN,
                ),
                (
                    "BUTTON",
                    "Normal",
                    WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32,
                    12,
                    210,
                    95,
                    24,
                    NORMAL,
                ),
                (
                    "BUTTON",
                    "Extended",
                    BS_AUTORADIOBUTTON as u32,
                    110,
                    210,
                    105,
                    24,
                    EXTENDED,
                ),
                (
                    "BUTTON",
                    "Regular expression",
                    BS_AUTORADIOBUTTON as u32,
                    225,
                    210,
                    160,
                    24,
                    REGEX,
                ),
                (
                    "BUTTON",
                    ". matches newline",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    350,
                    174,
                    150,
                    24,
                    DOT,
                ),
                (
                    "BUTTON",
                    "Find All",
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                    350,
                    110,
                    75,
                    28,
                    IDOK,
                ),
                ("BUTTON", "Stop", WS_TABSTOP, 430, 110, 70, 28, STOP),
                ("BUTTON", "Close", WS_TABSTOP, 535, 110, 110, 28, IDCANCEL),
            ];
            for (class, title, style, x, y, width, height, id) in controls {
                let child = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(title).as_ptr(),
                    WS_CHILD | WS_VISIBLE | style,
                    if id == IDOK || id == STOP { 535 } else { x },
                    if id == IDOK {
                        52
                    } else if id == STOP {
                        92
                    } else {
                        y + 40
                    },
                    if id == IDOK || id == STOP { 110 } else { width },
                    if class == "COMBOBOX" { 220 } else { height },
                    dialog,
                    id as usize as HMENU,
                    instance,
                    null(),
                );
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            CheckRadioButton(dialog, NORMAL, REGEX, NORMAL);
            // CBS_DROPDOWN ignores the initial window title for its edit field.
            SetDlgItemTextW(dialog, FILTER, w!("*.*"));
            CheckDlgButton(dialog, RECURSIVE, BST_CHECKED);
            EnableWindow(GetDlgItem(dialog, DOT), 0);
            EnableWindow(GetDlgItem(dialog, STOP), 0);
            let list = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("LISTBOX"),
                null(),
                WS_CHILD
                    | WS_VSCROLL
                    | WS_HSCROLL
                    | WS_TABSTOP
                    | LBS_NOTIFY as u32
                    | LBS_OWNERDRAWFIXED as u32
                    | LBS_HASSTRINGS as u32
                    | LBS_NOINTEGRALHEIGHT as u32,
                0,
                0,
                0,
                0,
                owner,
                RESULTS as usize as HMENU,
                instance,
                null(),
            );
            let label = CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Search Results — double-click or Enter to open a match"),
                WS_CHILD,
                0,
                0,
                0,
                0,
                owner,
                RESULTS_LABEL as usize as HMENU,
                instance,
                null(),
            );
            let close = CreateWindowExW(
                0,
                w!("BUTTON"),
                w!("×"),
                WS_CHILD | WS_TABSTOP,
                0,
                0,
                0,
                0,
                owner,
                RESULTS_CLOSE as usize as HMENU,
                instance,
                null(),
            );
            for child in [list, label, close] {
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            SetWindowSubclass(list, Some(result_proc), 1, 0);
            SetWindowSubclass(owner, Some(results_owner_proc), RESULTS as usize, 0);
            let results_font = CreateFontW(
                -15,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                FIXED_PITCH as u32,
                w!("Consolas"),
            );
            SendMessageW(list, WM_SETFONT, results_font as usize, 1);
            SendMessageW(list, LB_SETITEMHEIGHT, 0, 22);
            SendMessageW(list, LB_SETHORIZONTALEXTENT, 2500, 0);
            Ok(Self {
                results_font,
                dialog,
                list,
                label,
                owner,
                receiver: None,
                cancel: Arc::new(AtomicBool::new(false)),
                rows: Vec::new(),
                text_rows: Vec::new(),
                errors: 0,
            })
        }
    }
    pub fn show(&self, needle: &str, directory: &Path) {
        // SAFETY: Native handles remain owned by this object.
        unsafe {
            if !needle.is_empty() {
                SetDlgItemTextW(self.dialog, FIND, wide(needle).as_ptr());
            }
            if text(GetDlgItem(self.dialog, DIRECTORY)).is_empty() {
                self.set_directory(directory);
            }
            ShowWindow(self.dialog, SW_SHOW);
            SetForegroundWindow(self.dialog);
            SetFocus(GetDlgItem(self.dialog, FIND));
        }
    }
    pub fn set_directory(&self, directory: &Path) {
        // SAFETY: Buffer lives through the synchronous control update.
        unsafe {
            SetDlgItemTextW(
                self.dialog,
                DIRECTORY,
                wide(&directory.to_string_lossy()).as_ptr(),
            );
        }
    }
    pub fn set_needle(&self, needle: &str) {
        unsafe {
            SetDlgItemTextW(self.dialog, FIND, wide(needle).as_ptr());
        }
    }
    pub fn translate(&self, message: &MSG) -> bool {
        // SAFETY: Dialog is live and the message comes from the owner UI thread.
        unsafe {
            if IsWindowVisible(self.dialog) == 0 {
                return false;
            }
            if (message.hwnd == self.dialog || IsChild(self.dialog, message.hwnd) != 0)
                && message.message == WM_KEYDOWN
                && message.wParam == VK_ESCAPE as usize
            {
                ShowWindow(self.dialog, SW_HIDE);
                SetFocus(GetDlgItem(self.owner, 101));
                return true;
            }
            IsDialogMessageW(self.dialog, message) != 0
        }
    }
    pub fn show_results(&self, show: bool) {
        // SAFETY: All result-panel children belong to the live main window.
        unsafe {
            for id in [RESULTS, RESULTS_LABEL, RESULTS_CLOSE] {
                ShowWindow(
                    GetDlgItem(self.owner, id),
                    if show { SW_SHOWNA } else { SW_HIDE },
                );
            }
        }
    }
    fn row(&mut self, text: &str, hit: Option<FileSearchHit>) {
        let text: String = text
            .chars()
            .flat_map(|ch| match ch {
                '\t' => "    ".chars().collect::<Vec<_>>(),
                ch if ch.is_control() => format!("\\x{:02X}", ch as u32).chars().collect(),
                ch => vec![ch],
            })
            .collect();
        // SAFETY: The list copies the supplied terminated string synchronously.
        unsafe {
            let index = SendMessageW(self.list, LB_ADDSTRING, 0, wide(&text).as_ptr() as isize);
            if index < 0 {
                return;
            }
            SendMessageW(
                self.list,
                LB_SETITEMDATA,
                index as usize,
                hit.is_some() as isize,
            );
        }
        self.rows.push(hit);
        self.text_rows.push(text);
    }
    pub fn start(
        &mut self,
        query: FileSearchQuery,
        open_files: OpenFileContents,
    ) -> io::Result<()> {
        find_files::validate_file_search(&query)?;
        // SAFETY: Combo boxes copy strings; prior values remain available for
        // reuse throughout this dialog's lifetime.
        unsafe {
            for (id, value) in [
                (FIND, query.needle.clone()),
                (FILTER, query.filters.clone()),
                (DIRECTORY, query.directory.to_string_lossy().into_owned()),
            ] {
                let value = wide(&value);
                let control = GetDlgItem(self.dialog, id);
                let index = SendMessageW(
                    control,
                    CB_FINDSTRINGEXACT,
                    usize::MAX,
                    value.as_ptr() as isize,
                );
                if index >= 0 {
                    SendMessageW(control, CB_DELETESTRING, index as usize, 0);
                }
                SendMessageW(control, CB_INSERTSTRING, 0, value.as_ptr() as isize);
                SendMessageW(control, CB_SETCURSEL, 0, 0);
                while SendMessageW(control, CB_GETCOUNT, 0, 0) > 20 {
                    SendMessageW(control, CB_DELETESTRING, 20, 0);
                }
            }
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (sender, receiver) = mpsc::sync_channel(32);
        self.receiver = Some(receiver);
        self.errors = 0;
        // SAFETY: UI changes happen on the owning thread before the worker starts.
        unsafe {
            SetWindowTextW(self.label, w!("Searching… — Stop cancels the search"));
            EnableWindow(GetDlgItem(self.dialog, STOP), 1);
            EnableWindow(GetDlgItem(self.dialog, IDOK), 0);
            SetTimer(self.owner, TIMER, 100, None);
        }
        self.show_results(true);
        self.row(
            &format!(
                "Search \"{}\" in {}",
                query.needle,
                query.directory.display()
            ),
            None,
        );
        // SAFETY: Reveal the latest search header while retaining prior searches.
        unsafe {
            SendMessageW(
                self.list,
                LB_SETTOPINDEX,
                self.rows.len().saturating_sub(1),
                0,
            );
        }
        std::thread::spawn(move || {
            find_files::search_files(query, open_files, &cancel, |event| {
                let _ = sender.send(event);
            })
        });
        Ok(())
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn clear(&mut self) {
        self.stop();
        self.receiver = None;
        self.rows.clear();
        self.text_rows.clear();
        // SAFETY: Clear the owned list and stop its background polling timer.
        unsafe {
            KillTimer(self.owner, TIMER);
            SendMessageW(self.list, LB_RESETCONTENT, 0, 0);
            SetWindowTextW(self.label, w!("Search Results"));
            EnableWindow(GetDlgItem(self.dialog, STOP), 0);
            EnableWindow(GetDlgItem(self.dialog, IDOK), 1);
        }
    }
    pub fn copy(&self, all: bool) -> io::Result<()> {
        let text = if all {
            self.text_rows.join("\r\n")
        } else {
            // SAFETY: Read the selected list row; no native pointer escapes.
            let index = unsafe { SendMessageW(self.list, LB_GETCURSEL, 0, 0) };
            if index < 0 {
                return Ok(());
            }
            self.text_rows
                .get(index as usize)
                .cloned()
                .unwrap_or_default()
        };
        crate::clipboard::write_text(self.owner, &text)
    }
    pub fn poll(&mut self) {
        for _ in 0..32 {
            let Some(event) = self.receiver.as_ref().and_then(|r| r.try_recv().ok()) else {
                break;
            };
            match event {
                FileSearchEvent::Progress(path, scanned) => {
                    // SAFETY: Update the owned label using a temporary string.
                    unsafe {
                        SetWindowTextW(
                            self.label,
                            wide(&format!("Searching {} — {scanned} files", path.display()))
                                .as_ptr(),
                        );
                    }
                }
                FileSearchEvent::Hits(hits) => {
                    if let Some(hit) = hits.first() {
                        self.row(
                            &format!("  {} ({} hits)", display_path(&hit.path), hits.len()),
                            None,
                        );
                    }
                    let width = hits.last().map_or(1, |h| (h.line + 1).to_string().len());
                    let mut previous_line = None;
                    for hit in hits {
                        if previous_line == Some(hit.line) {
                            continue;
                        }
                        previous_line = Some(hit.line);
                        self.row(
                            &format!("    Line {:>width$}: {}", hit.line + 1, hit.preview),
                            Some(hit),
                        );
                    }
                }
                FileSearchEvent::Error(path, error) => {
                    self.errors += 1;
                    self.row(
                        &format!("  Could not search {}: {error}", path.display()),
                        None,
                    );
                }
                FileSearchEvent::Finished {
                    scanned,
                    matching_files,
                    hits,
                    cancelled,
                } => {
                    let summary = format!(
                        "{hits} hits in {matching_files} files ({scanned} files searched, {} errors){}",
                        self.errors,
                        if cancelled { " — cancelled" } else { "" }
                    );
                    self.row(&summary, None);
                    // SAFETY: Update owned controls and remove this worker's timer.
                    unsafe {
                        SetWindowTextW(self.label, wide(&summary).as_ptr());
                        KillTimer(self.owner, TIMER);
                        EnableWindow(GetDlgItem(self.dialog, STOP), 0);
                        EnableWindow(GetDlgItem(self.dialog, IDOK), 1);
                    }
                    self.receiver = None;
                    break;
                }
            }
        }
    }
    pub fn hit(&self, index: usize) -> Option<FileSearchHit> {
        self.rows.get(index).and_then(Clone::clone)
    }
    pub fn adjacent(&self, forward: bool) -> Option<usize> {
        // SAFETY: Read selection from the owned list control.
        let current = unsafe { SendMessageW(self.list, LB_GETCURSEL, 0, 0) };
        let start = if current < 0 {
            if forward {
                0
            } else {
                self.rows.len().saturating_sub(1)
            }
        } else if forward {
            (current as usize + 1) % self.rows.len().max(1)
        } else {
            (current as usize + self.rows.len().saturating_sub(1)) % self.rows.len().max(1)
        };
        (0..self.rows.len())
            .map(|step| {
                if forward {
                    (start + step) % self.rows.len()
                } else {
                    (start + self.rows.len() - step) % self.rows.len()
                }
            })
            .find(|&index| self.rows[index].is_some())
    }
}
impl Drop for FindFiles {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: Dialog and timer are owned on this UI thread; children of the
        // main window are released with that window.
        unsafe {
            KillTimer(self.owner, TIMER);
            DestroyWindow(self.dialog);
            RemoveWindowSubclass(self.owner, Some(results_owner_proc), RESULTS as usize);
            SendMessageW(
                self.list,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as usize,
                0,
            );
            DeleteObject(self.results_font);
        }
    }
}
