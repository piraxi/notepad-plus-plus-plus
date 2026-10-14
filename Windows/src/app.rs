use crate::search_dialog::{self, Request, SearchDialog};
use crate::{
    file_io::{atomic_write, normalized_save_path},
    projection::Projection,
};
use npp_compat::commands::*;
use npp_compat::{
    plugin::{NotifyHeader, ScNotification},
    registry::PluginRegistry,
};
use npp_editor_core::{
    BufferId, CloseScope, Document, DuplicateLines, Encoding, IndentationStyle, LineEnding,
    LineOrder, SearchDirection, SearchMode, SearchOptions, Selection, TrimMode,
    WhitespaceConversion, Workspace,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    mem::size_of,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        System::SystemServices::{
            APPCOMMAND_BROWSER_BACKWARD, APPCOMMAND_BROWSER_FORWARD, MK_CONTROL, MK_RBUTTON,
            MK_SHIFT,
        },
        UI::{
            Controls::{Dialogs::*, *},
            Input::KeyboardAndMouse::*,
            Shell::{DefSubclassProc, DragAcceptFiles, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

// Keep dynamic history/control IDs outside the 500-entry UDL command range.
const RECENT_FILE_FIRST: u32 = 63000;
const RECENT_FILE_LAST: u32 = RECENT_FILE_FIRST + 29;
const POST_IT_RESTORE: i32 = 63031;
const _: () = assert!(
    crate::languages::USER_LANGUAGE_FIRST + crate::languages::USER_LANGUAGE_LIMIT as u32
        <= RECENT_FILE_FIRST
);
const EDIT_ID: usize = 101;
const TAB_ID: usize = 102;
const STATUS_ID: usize = 103;
const SHOW_STATUS_BAR: u32 = 61029;
const SHOW_TAB_BAR: u32 = 61030;
const SHOW_MENU_BAR: u32 = 61031;
const LOAD_MACRO: u32 = 61057;
const SHOW_TOOLBAR: u32 = 61058;
const SHOW_LINE_NUMBERS: u32 = 61059;
const SEARCH_REGEX: u32 = 61060;
// Rewrite-specific explicit open/convert actions, distinct from upstream's
// system-ANSI and reinterpret-current-buffer commands.
const OPEN_1252: u32 = 61001;
const OPEN_1251: u32 = 61002;
const OPEN_SHIFT_JIS: u32 = 61003;
const CONVERT_1252: u32 = 61004;
const CONVERT_1251: u32 = 61005;
const CONVERT_SHIFT_JIS: u32 = 61006;
const OPEN_UTF16LE_NO_BOM: u32 = 61007;
const OPEN_UTF16BE_NO_BOM: u32 = 61008;
const CONVERT_UTF16LE_NO_BOM: u32 = 61009;
const CONVERT_UTF16BE_NO_BOM: u32 = 61010;
const SEARCH_EXTENDED: u32 = 61011;
pub(crate) const SEARCH_COUNT: u32 = 61012;
const SEARCH_IN_SELECTION: u32 = 61013;
pub(crate) const SEARCH_WRAP: u32 = 61054;
const SEARCH_BOOKMARK_MATCHES: u32 = 61055;
const CLEAR_BOOKMARKS_IN_SELECTION: u32 = 61056;
const TAB_WIDTH_2: u32 = 61014;
const TAB_WIDTH_4: u32 = 61015;
const TAB_WIDTH_8: u32 = 61016;
const INSERT_TAB: u32 = 61017;
const BACK_TAB: u32 = 61018;
const INDENT_WITH_SPACES: u32 = 61019;
const DELETE_LINE: u32 = 61020;
const TRANSPOSE_LINE: u32 = 61021;
const DELETE_LINE_LEFT: u32 = 61022;
const DELETE_LINE_RIGHT: u32 = 61023;
const INSERT_NEWLINE: u32 = 61024;
const BASIC_AUTO_INDENT: u32 = 61025;
const CUT_LINES: u32 = 61026;
const COPY_LINES: u32 = 61027;
const DATE_FIRST: u32 = 61028;
const DELETE_WORD_LEFT: u32 = 61032;
const DELETE_WORD_RIGHT: u32 = 61033;
const WORD_LEFT: u32 = 61034;
const WORD_RIGHT: u32 = 61035;
const WORD_LEFT_EXTEND: u32 = 61036;
const WORD_RIGHT_EXTEND: u32 = 61037;
const SMART_HOME: u32 = 61038;
const SMART_HOME_EXTEND: u32 = 61039;
const LINE_END: u32 = 61040;
const LINE_END_EXTEND: u32 = 61041;
const DOCUMENT_START: u32 = 61042;
const DOCUMENT_START_EXTEND: u32 = 61043;
const DOCUMENT_END: u32 = 61044;
const DOCUMENT_END_EXTEND: u32 = 61045;
const PARAGRAPH_UP: u32 = 61046;
const PARAGRAPH_DOWN: u32 = 61047;
const PARAGRAPH_UP_EXTEND: u32 = 61048;
const PARAGRAPH_DOWN_EXTEND: u32 = 61049;
const WORD_PART_LEFT: u32 = 61050;
const WORD_PART_RIGHT: u32 = 61051;
const WORD_PART_LEFT_EXTEND: u32 = 61052;
const WORD_PART_RIGHT_EXTEND: u32 = 61053;
// Commands that cannot run on a read-only buffer. Back-tab is excluded because
// outside indentation it is a navigation operation, even in read-only text.
const FORMAT_DOCUMENT: u32 = 61100;
const FORMAT_SELECTION: u32 = 61101;
const EDIT_COMMANDS: &[u32] = &[
    FORMAT_DOCUMENT,
    FORMAT_SELECTION,
    IDM_EDIT_PASTE_AS_HTML,
    IDM_EDIT_PASTE_AS_RTF,
    IDM_EDIT_EOL2WS,
    IDM_EDIT_TRIMALL,
    IDM_EDIT_AUTOCOMPLETE,
    IDM_EDIT_DELETE,
    IDM_EDIT_REDACT_SELECTION,
    IDM_EDIT_AUTOCOMPLETE_CURRENTFILE,
    IDM_EDIT_BLOCK_COMMENT,
    IDM_EDIT_BLOCK_COMMENT_SET,
    IDM_EDIT_BLOCK_UNCOMMENT,
    IDM_EDIT_STREAM_COMMENT,
    IDM_EDIT_STREAM_UNCOMMENT,
    IDM_SEARCH_DELETEMARKEDLINES,
    IDM_SEARCH_DELETEUNMARKEDLINES,
    IDM_SEARCH_CUTMARKEDLINES,
    IDM_SEARCH_PASTEMARKEDLINES,
    IDM_EDIT_SORTLINES_LOCALE_ASCENDING,
    IDM_EDIT_SORTLINES_LOCALE_DESCENDING,
    IDM_EDIT_INSERT_DATETIME_SHORT,
    IDM_EDIT_INSERT_DATETIME_LONG,
    IDM_EDIT_INSERT_DATETIME_CUSTOMIZED,
    CUT_LINES,
    INSERT_NEWLINE,
    INSERT_TAB,
    IDM_EDIT_INS_TAB,
    IDM_EDIT_RMV_TAB,
    IDM_EDIT_UPPERCASE,
    IDM_EDIT_LOWERCASE,
    IDM_EDIT_INVERTCASE,
    IDM_EDIT_RANDOMCASE,
    IDM_EDIT_PROPERCASE_FORCE,
    IDM_EDIT_PROPERCASE_BLEND,
    IDM_EDIT_SENTENCECASE_FORCE,
    IDM_EDIT_SENTENCECASE_BLEND,
    IDM_EDIT_DUP_LINE,
    DELETE_LINE,
    TRANSPOSE_LINE,
    DELETE_LINE_LEFT,
    DELETE_LINE_RIGHT,
    DELETE_WORD_LEFT,
    DELETE_WORD_RIGHT,
    IDM_EDIT_TRIMTRAILING,
    IDM_EDIT_TRIMLINEHEAD,
    IDM_EDIT_TRIM_BOTH,
    IDM_EDIT_TAB2SW,
    IDM_EDIT_SW2TAB_LEADING,
    IDM_EDIT_SW2TAB_ALL,
    IDM_EDIT_REMOVE_CONSECUTIVE_DUP_LINES,
    IDM_EDIT_REMOVE_ANY_DUP_LINES,
    IDM_EDIT_REMOVEEMPTYLINES,
    IDM_EDIT_REMOVEEMPTYLINESWITHBLANK,
    IDM_EDIT_JOIN_LINES,
    IDM_EDIT_SPLIT_LINES,
    IDM_EDIT_LINE_UP,
    IDM_EDIT_LINE_DOWN,
    IDM_EDIT_BLANKLINEABOVECURRENT,
    IDM_EDIT_BLANKLINEBELOWCURRENT,
    IDM_EDIT_SORTLINES_LEXICOGRAPHIC_ASCENDING,
    IDM_EDIT_SORTLINES_LEXICOGRAPHIC_DESCENDING,
    IDM_EDIT_SORTLINES_REVERSE_ORDER,
    IDM_EDIT_SORTLINES_RANDOMLY,
    IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_ASCENDING,
    IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_DESCENDING,
    IDM_EDIT_SORTLINES_LENGTH_ASCENDING,
    IDM_EDIT_SORTLINES_LENGTH_DESCENDING,
    IDM_EDIT_SORTLINES_INTEGER_ASCENDING,
    IDM_EDIT_SORTLINES_INTEGER_DESCENDING,
    IDM_EDIT_SORTLINES_DECIMALDOT_ASCENDING,
    IDM_EDIT_SORTLINES_DECIMALDOT_DESCENDING,
    IDM_EDIT_SORTLINES_DECIMALCOMMA_ASCENDING,
    IDM_EDIT_SORTLINES_DECIMALCOMMA_DESCENDING,
    IDM_EDIT_UNDO,
    IDM_EDIT_REDO,
    IDM_EDIT_CUT,
    IDM_EDIT_CUT_BINARY,
    IDM_EDIT_PASTE_BINARY,
    IDM_EDIT_PASTE,
    IDM_FORMAT_TODOS,
    IDM_FORMAT_TOUNIX,
    IDM_FORMAT_TOMAC,
    IDM_FORMAT_CONV2_AS_UTF_8,
    IDM_FORMAT_CONV2_UTF_8,
    IDM_FORMAT_CONV2_UTF_16LE,
    IDM_FORMAT_CONV2_UTF_16BE,
    CONVERT_1252,
    CONVERT_1251,
    CONVERT_SHIFT_JIS,
    CONVERT_UTF16LE_NO_BOM,
    CONVERT_UTF16BE_NO_BOM,
];
pub(crate) enum Event {
    BookmarkMargin(usize),
    LiteralCharacter(char),
    NativeChanged,
    RehideMarkedLines,
    GoTo(i64, bool),
    GoToRefresh,
    Wheel(i32, bool),
    HorizontalWheel(i32, bool),
    CheckFileAttributes,
    FileAttributesReady,
    OpenPaths(Vec<PathBuf>),
    FindFiles(npp_editor_core::find_files::FileSearchQuery),
    CancelFindFiles,
    PollFindFiles,
    FindFilesDirectory,
    OpenFindFileResult(usize),
    HideFindFilesResults,
    CopyFindFilesResults(bool),
    ClearFindFilesResults,
    LoadSession(PathBuf),
    Search(Request),
    SearchTab(u32, String),
    SearchMode(SearchMode),
    Command(u32),
    Changed(String, usize, usize),
    Selection(usize, usize),
    Tab(usize),
    CloseTab(usize),
    DoubleClickTab(usize),
    MoveTab(usize, usize),
    Resize,
    Close,
}
thread_local! {
    static EVENTS: RefCell<VecDeque<Event>> = const { RefCell::new(VecDeque::new()) };
    static UPDATING: Cell<bool> = const { Cell::new(false) };
}
pub(crate) fn enqueue(event: Event) {
    if !UPDATING.get() {
        EVENTS.with_borrow_mut(|q| q.push_back(event));
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn control_text(hwnd: HWND) -> String {
    // SAFETY: Called on a live control on its owning thread; buffer includes NUL.
    unsafe {
        let mut text = vec![0u16; GetWindowTextLengthW(hwnd) as usize + 1];
        let length = GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        String::from_utf16_lossy(&text[..length as usize])
    }
}
fn is_status_hidden(window: HWND) -> bool {
    is_control_hidden(window, STATUS_ID)
}

fn is_control_hidden(window: HWND, control: usize) -> bool {
    // SAFETY: Read this window's child control style. Unlike IsWindowVisible,
    // this remains accurate while the parent window is hidden or minimized.
    unsafe {
        GetWindowLongPtrW(GetDlgItem(window, control as i32), GWL_STYLE) as u32 & WS_VISIBLE == 0
    }
}

fn resize_controls(window: HWND) {
    // SAFETY: These child handles belong to the calling thread's live window.
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(window, &mut rect);
        let restore = GetDlgItem(window, POST_IT_RESTORE);
        if !restore.is_null() {
            let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(window).max(96) as i32;
            let width = 110 * dpi / 96;
            let height = 26 * dpi / 96;
            SetWindowPos(
                restore,
                HWND_TOP,
                (rect.right - width - 4).max(0),
                4,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
        if !GetPropW(window, w!("DistractionFree")).is_null() {
            let divisor = (GetPropW(window, w!("DistractionDivisor")) as usize).clamp(3, 9);
            let padding = rect.right.max(0) / divisor as i32;
            crate::scintilla::send(
                GetDlgItem(window, EDIT_ID as i32),
                crate::scintilla::ids::SCI_SETMARGINLEFT,
                0,
                padding as isize,
            );
            crate::scintilla::send(
                GetDlgItem(window, EDIT_ID as i32),
                crate::scintilla::ids::SCI_SETMARGINRIGHT,
                0,
                padding as isize,
            );
        }
        let hidden = is_status_hidden(window);
        let tabs_hidden = is_control_hidden(window, TAB_ID);
        let toolbar_height = if is_control_hidden(window, crate::editor_chrome::TOOLBAR as usize) {
            0
        } else {
            28
        };
        let gutter_width = if is_control_hidden(window, crate::editor_chrome::GUTTER as usize) {
            0
        } else {
            crate::scintilla::line_number_width(GetDlgItem(window, EDIT_ID as i32)) as i32
        };
        crate::scintilla::send(
            GetDlgItem(window, EDIT_ID as i32),
            crate::scintilla::ids::SCI_SETMARGINWIDTHN,
            0,
            gutter_width as isize,
        );
        let gutter_width = 0;
        let tabs = GetDlgItem(window, TAB_ID as i32);
        // Set width before asking for row count; wrapping depends on this width.
        MoveWindow(tabs, 0, toolbar_height, rect.right.max(0), 28, 0);
        let tab_height = if tabs_hidden {
            0
        } else if GetWindowLongPtrW(tabs, GWL_STYLE) as u32 & TCS_MULTILINE != 0 {
            let rows = SendMessageW(tabs, TCM_GETROWCOUNT, 0, 0).max(1) as i32;
            let mut item = RECT::default();
            let height =
                if SendMessageW(tabs, TCM_GETITEMRECT, 0, &mut item as *mut _ as isize) != 0 {
                    (item.bottom - item.top).max(1)
                } else {
                    24
                };
            rows.saturating_mul(height).saturating_add(6)
        } else {
            28
        };
        let top = toolbar_height + tab_height;
        let result_list = GetDlgItem(window, crate::find_files::RESULTS);
        let result_height = if !result_list.is_null()
            && GetWindowLongPtrW(result_list, GWL_STYLE) as u32 & WS_VISIBLE != 0
        {
            (rect.bottom / 3).clamp(100, 260)
        } else {
            0
        };
        let result_top = rect.bottom - if hidden { 0 } else { 24 } - result_height;
        if result_height != 0 {
            MoveWindow(
                GetDlgItem(window, crate::find_files::RESULTS_LABEL),
                6,
                result_top,
                (rect.right - 40).max(0),
                24,
                1,
            );
            MoveWindow(
                GetDlgItem(window, crate::find_files::RESULTS_CLOSE),
                (rect.right - 30).max(0),
                result_top,
                28,
                24,
                1,
            );
            MoveWindow(
                result_list,
                0,
                result_top + 24,
                rect.right,
                (result_height - 24).max(0),
                1,
            );
        }
        MoveWindow(
            GetDlgItem(window, crate::editor_chrome::TOOLBAR),
            0,
            0,
            rect.right,
            28,
            1,
        );
        MoveWindow(
            GetDlgItem(window, TAB_ID as i32),
            0,
            toolbar_height,
            rect.right,
            tab_height,
            1,
        );
        MoveWindow(
            GetDlgItem(window, crate::editor_chrome::GUTTER),
            0,
            top,
            0,
            (result_top - top).max(0),
            1,
        );
        MoveWindow(
            GetDlgItem(window, EDIT_ID as i32),
            gutter_width,
            top,
            (rect.right - gutter_width).max(0),
            (rect.bottom - top - if hidden { 0 } else { 24 } - result_height).max(0),
            1,
        );
        MoveWindow(
            GetDlgItem(window, STATUS_ID as i32),
            6,
            (rect.bottom - 22).max(0),
            (rect.right - 12).max(0),
            22,
            1,
        );
        CheckMenuItem(
            crate::plugin_host::main_menu(window),
            SHOW_STATUS_BAR,
            MF_BYCOMMAND | if hidden { MF_UNCHECKED } else { MF_CHECKED },
        );
        CheckMenuItem(
            crate::plugin_host::main_menu(window),
            SHOW_TAB_BAR,
            MF_BYCOMMAND
                | if tabs_hidden {
                    MF_UNCHECKED
                } else {
                    MF_CHECKED
                },
        );
    }
}

fn control_selection(hwnd: HWND) -> (usize, usize) {
    let (mut start, mut end) = (0u32, 0u32);
    // SAFETY: EM_GETSEL writes DWORDs into these live stack variables.
    unsafe {
        SendMessageW(
            hwnd,
            EM_GETSEL,
            &mut start as *mut _ as usize,
            &mut end as *mut _ as isize,
        );
    }
    (start as usize, end as usize)
}
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    // SAFETY: Windows invokes this subclass with a live HWND on the UI thread.
    thread_local! { static REDIRECTING_WHEEL: Cell<bool> = const { Cell::new(false) }; }
    thread_local! { static RIGHT_WHEEL_SWITCH: Cell<bool> = const { Cell::new(false) }; }
    unsafe {
        if message == WM_RBUTTONDOWN {
            RIGHT_WHEEL_SWITCH.set(false);
        }
        if message == WM_CONTEXTMENU && lp != -1 && RIGHT_WHEEL_SWITCH.get() {
            return 0;
        }
        if message == WM_RBUTTONUP && RIGHT_WHEEL_SWITCH.get() {
            // Consume release so the completed tab gesture does not open a popup.
            RIGHT_WHEEL_SWITCH.set(false);
            return 0;
        }
        if matches!(message, WM_MOUSEWHEEL | WM_MOUSEHWHEEL)
            && !REDIRECTING_WHEEL.get()
            && GetPropW(hwnd, w!("DisableAdvancedScrolling")).is_null()
        {
            let target = WindowFromPoint(POINT {
                x: lp as i16 as i32,
                y: (lp >> 16) as i16 as i32,
            });
            let mut class = [0u16; 64];
            let length = GetClassNameW(target, class.as_mut_ptr(), class.len() as i32);
            let synaptics = String::from_utf16_lossy(&class[..length.max(0) as usize])
                == "SynTrackCursorWindowClass";
            // Redirect within this UI thread; never block on an unrelated app.
            if !target.is_null()
                && target != hwnd
                && !synaptics
                && GetWindowThreadProcessId(target, null_mut())
                    == GetWindowThreadProcessId(hwnd, null_mut())
            {
                REDIRECTING_WHEEL.set(true);
                let result = SendMessageW(target, message, wp, lp);
                REDIRECTING_WHEEL.set(false);
                return result;
            }
        }
        if message == WM_MOUSEWHEEL && wp & MK_RBUTTON as usize != 0 {
            let delta = (wp >> 16) as u16 as i16;
            if delta != 0 {
                RIGHT_WHEEL_SWITCH.set(true);
                enqueue(Event::Command(if delta > 0 {
                    IDM_VIEW_TAB_PREV
                } else {
                    IDM_VIEW_TAB_NEXT
                }));
            }
            return 0;
        }
        if message == WM_MOUSEHWHEEL || (message == WM_MOUSEWHEEL && wp & MK_SHIFT as usize != 0) {
            let horizontal = message == WM_MOUSEHWHEEL;
            let delta = ((wp >> 16) as u16 as i16) as i32;
            enqueue(Event::HorizontalWheel(delta, horizontal));
            return isize::from(horizontal);
        }
        if message == WM_MOUSEWHEEL && wp & MK_SHIFT as usize == 0 {
            let zoom = wp & MK_CONTROL as usize != 0;
            let delta = ((wp >> 16) as u16 as i16) as i32;
            enqueue(Event::Wheel(delta, zoom));
            if zoom {
                return 0;
            }
        }
        if message == WM_KEYDOWN && !GetPropW(hwnd, w!("ColumnToMulti")).is_null() {
            use crate::scintilla::{ids::*, send};
            let mode = send(hwnd, SCI_GETSELECTIONMODE, 0, 0) as u32;
            if matches!(mode, SC_SEL_RECTANGLE | SC_SEL_THIN) {
                if matches!(
                    wp as u16,
                    VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_RETURN | VK_BACK
                ) {
                    send(hwnd, SCI_SETSELECTIONMODE, SC_SEL_STREAM as usize, 0);
                    send(hwnd, SCI_SETSELECTIONMODE, SC_SEL_STREAM as usize, 0);
                } else if wp == VK_ESCAPE as usize {
                    let main = send(hwnd, SCI_GETMAINSELECTION, 0, 0) as usize;
                    let caret = send(hwnd, SCI_GETSELECTIONNCARET, main, 0);
                    send(hwnd, SCI_SETSELECTION, caret as usize, caret);
                    send(hwnd, SCI_SETSELECTIONMODE, SC_SEL_STREAM as usize, 0);
                }
            }
        }
        // Native commands retain every caret for multi-selection newline/tab
        // edits; the single-selection core commands would collapse the ranges.
        let multi_selection = matches!(message, WM_KEYDOWN | WM_CHAR)
            && matches!(wp, x if x == VK_RETURN as usize || x == VK_TAB as usize)
            && crate::scintilla::send(hwnd, crate::scintilla::ids::SCI_GETSELECTIONS, 0, 0) > 1;
        // TranslateMessage still posts WM_CHAR. Keep Scintilla's key-down
        // bindings from inserting a second newline/tab before our core edit.
        if message == WM_KEYDOWN
            && !multi_selection
            && matches!(wp, x if x == VK_RETURN as usize || x == VK_TAB as usize)
        {
            return 0;
        }
        if message == WM_CHAR && wp == VK_RETURN as usize && !multi_selection {
            let (start, end) = control_selection(hwnd);
            enqueue(Event::Selection(start, end));
            enqueue(Event::Command(INSERT_NEWLINE));
            return 0;
        }
        if message == WM_CHAR && wp == VK_TAB as usize && !multi_selection {
            let (start, end) = control_selection(hwnd);
            enqueue(Event::Selection(start, end));
            enqueue(Event::Command(if GetKeyState(VK_SHIFT as i32) < 0 {
                BACK_TAB
            } else {
                INSERT_TAB
            }));
            return 0;
        }
        if message == WM_UNDO || message == EM_UNDO {
            enqueue(Event::Command(IDM_EDIT_UNDO));
            return 0;
        }
        if message == WM_CHAR && matches!(wp, 0x2400 | 0x241b) {
            enqueue(Event::LiteralCharacter(char::from_u32(wp as u32).unwrap()));
            return 0;
        }
        if matches!(message, WM_COPY | WM_CUT | WM_PASTE) {
            enqueue(Event::Command(match message {
                WM_COPY => IDM_EDIT_COPY,
                WM_CUT => IDM_EDIT_CUT,
                _ => IDM_EDIT_PASTE,
            }));
            return 0;
        }
        if matches!(
            message,
            WM_CHAR | WM_KEYDOWN | WM_CUT | WM_PASTE | WM_CLEAR | WM_KEYUP | WM_LBUTTONUP
        ) {
            let (start, end) = control_selection(hwnd);
            enqueue(Event::Selection(start, end));
        }
        let result = DefSubclassProc(hwnd, message, wp, lp);
        if matches!(
            message,
            WM_PAINT
                | WM_VSCROLL
                | WM_MOUSEWHEEL
                | WM_KEYUP
                | WM_SETTEXT
                | EM_LINESCROLL
                | WM_SETFONT
        ) {
            let gutter = GetDlgItem(GetParent(hwnd), crate::editor_chrome::GUTTER);
            if !gutter.is_null() {
                InvalidateRect(gutter, null(), 0);
            }
        }
        result
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: Message payloads are used only during their synchronous dispatch.
    unsafe {
        if message != 0 && message == search_dialog::message() {
            enqueue(Event::Search(search_dialog::snapshot(lp)));
            return 0;
        }
        if let Some(result) = crate::plugin_host::query(hwnd, message, wp, lp) {
            return result;
        }
        if let Some(result) = crate::plugin_host::modeless_dialog(hwnd, message, wp, lp) {
            return result;
        }
        if let Some(result) = crate::plugin_host::menu_message(hwnd, message, wp, lp) {
            return result;
        }
        if let Some(result) = crate::plugin_host::query_text(hwnd, message, wp, lp) {
            return result;
        }
        match message {
            WM_TIMER if wp == crate::find_files::TIMER => {
                enqueue(Event::PollFindFiles);
                0
            }
            WM_COMMAND
                if wp & 0xffff == crate::find_files::RESULTS as usize
                    && (wp >> 16) as u32 == LBN_DBLCLK =>
            {
                enqueue(Event::OpenFindFileResult(
                    SendMessageW(lp as HWND, LB_GETCURSEL, 0, 0) as usize,
                ));
                0
            }
            WM_APPCOMMAND => {
                // GET_APPCOMMAND_LPARAM excludes the input-device flag bits.
                let command = ((lp as usize >> 16) & 0x0fff) as u32;
                if command == APPCOMMAND_BROWSER_BACKWARD || command == APPCOMMAND_BROWSER_FORWARD {
                    enqueue(Event::Command(if command == APPCOMMAND_BROWSER_FORWARD {
                        IDM_VIEW_TAB_NEXT
                    } else {
                        IDM_VIEW_TAB_PREV
                    }));
                    1
                } else {
                    DefWindowProcW(hwnd, message, wp, lp)
                }
            }
            WM_COMMAND if wp & 0xffff == POST_IT_RESTORE as usize => {
                enqueue(Event::Command(
                    if GetPropW(hwnd, w!("DistractionFree")).is_null() {
                        IDM_VIEW_POSTIT
                    } else {
                        IDM_VIEW_DISTRACTIONFREE
                    },
                ));
                0
            }
            WM_COMMAND if wp & 0xffff == crate::find_files::RESULTS_CLOSE as usize => {
                enqueue(Event::HideFindFilesResults);
                0
            }
            npp_compat::messages::NPPM_LOADSESSION => {
                if lp == 0 {
                    enqueue(Event::Command(IDM_FILE_LOADSESSION));
                } else if let Some(path) = crate::plugin_host::session_path(lp as *const u16) {
                    enqueue(Event::LoadSession(path));
                }
                1
            }
            npp_compat::messages::NPPM_ISMENUHIDDEN => GetMenu(hwnd).is_null() as isize,
            npp_compat::messages::NPPM_HIDEMENU => {
                let was_hidden = GetMenu(hwnd).is_null();
                let hide = lp == 1;
                if hide != was_hidden {
                    let menu = crate::plugin_host::main_menu(hwnd);
                    if SetMenu(hwnd, if hide { null_mut() } else { menu }) != 0 {
                        crate::plugin_host::set_menu_hidden(hwnd, hide);
                        CheckMenuItem(
                            menu,
                            SHOW_MENU_BAR,
                            MF_BYCOMMAND | if hide { MF_UNCHECKED } else { MF_CHECKED },
                        );
                        DrawMenuBar(hwnd);
                        resize_controls(hwnd);
                    }
                }
                was_hidden as isize
            }
            WM_ENTERMENULOOP | WM_EXITMENULOOP => {
                let entering = message == WM_ENTERMENULOOP;
                if let Some(attach) = crate::plugin_host::menu_loop(hwnd, entering, wp != 0) {
                    SetMenu(
                        hwnd,
                        if attach {
                            crate::plugin_host::main_menu(hwnd)
                        } else {
                            null_mut()
                        },
                    );
                    DrawMenuBar(hwnd);
                    resize_controls(hwnd);
                }
                entering as isize
            }
            WM_SYSCOMMAND => {
                let command = wp as u32 & 0xfff0;
                if (command == SC_KEYMENU && lp == VK_SPACE as isize) || command == SC_MOUSEMENU {
                    crate::plugin_host::entering_system_menu(hwnd);
                }
                DefWindowProcW(hwnd, message, wp, lp)
            }
            WM_NCRBUTTONDOWN => {
                if wp == HTCAPTION as usize {
                    crate::plugin_host::entering_system_menu(hwnd);
                }
                DefWindowProcW(hwnd, message, wp, lp)
            }
            npp_compat::messages::NPPM_ISTABBARHIDDEN => is_control_hidden(hwnd, TAB_ID) as isize,
            npp_compat::messages::NPPM_HIDETABBAR => {
                let was_hidden = is_control_hidden(hwnd, TAB_ID);
                let hide = lp != 0;
                if hide != was_hidden {
                    ShowWindow(
                        GetDlgItem(hwnd, TAB_ID as i32),
                        if hide { SW_HIDE } else { SW_SHOWNA },
                    );
                    resize_controls(hwnd);
                }
                was_hidden as isize
            }
            npp_compat::messages::NPPM_ISSTATUSBARHIDDEN => is_status_hidden(hwnd) as isize,
            npp_compat::messages::NPPM_HIDESTATUSBAR => {
                let was_shown = !is_status_hidden(hwnd);
                let show = lp != 1;
                if show != was_shown {
                    ShowWindow(
                        GetDlgItem(hwnd, STATUS_ID as i32),
                        if show { SW_SHOWNA } else { SW_HIDE },
                    );
                    resize_controls(hwnd);
                }
                was_shown as isize
            }
            WM_ACTIVATEAPP if wp != 0 => {
                enqueue(Event::CheckFileAttributes);
                DefWindowProcW(hwnd, message, wp, lp)
            }
            crate::file_attributes::READY => {
                enqueue(Event::FileAttributesReady);
                0
            }
            WM_DROPFILES => {
                enqueue(Event::OpenPaths(crate::file_drop::take_paths(wp as _)));
                0
            }
            WM_COMMAND if wp & 0xffff == EDIT_ID && (wp >> 16) as u32 == EN_CHANGE => {
                let (start, end) = control_selection(lp as HWND);
                enqueue(Event::Changed(control_text(lp as HWND), start, end));
                0
            }
            WM_COMMAND
                if lp == 0 || lp as HWND == GetDlgItem(hwnd, crate::editor_chrome::TOOLBAR) =>
            {
                enqueue(Event::Command((wp & 0xffff) as u32));
                0
            }
            WM_NOTIFY => {
                let notification = &*(lp as *const NMHDR);
                if notification.idFrom == EDIT_ID && !UPDATING.get() {
                    use crate::scintilla::ids::*;
                    if notification.code == SCN_MODIFIED {
                        let detail = &*(lp as *const crate::scintilla::Notification);
                        if detail.modification_type as u32
                            & (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT | SC_MOD_CHANGEFOLD)
                            != 0
                        {
                            EVENTS.with_borrow_mut(|events| {
                                if !events
                                    .iter()
                                    .any(|event| matches!(event, Event::RehideMarkedLines))
                                {
                                    events.push_back(Event::RehideMarkedLines);
                                }
                            });
                        }
                        if detail.modification_type as u32 & (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT)
                            != 0
                        {
                            EVENTS.with_borrow_mut(|events| {
                                if !events
                                    .iter()
                                    .any(|event| matches!(event, Event::NativeChanged))
                                {
                                    events.push_back(Event::NativeChanged);
                                }
                            });
                        }
                    } else if notification.code == SCN_MARGINCLICK {
                        let detail = &*(lp as *const crate::scintilla::Notification);
                        if detail.margin == 1 && detail.modifiers == 0 {
                            let line = crate::scintilla::send(
                                notification.hwndFrom,
                                SCI_LINEFROMPOSITION,
                                detail.position.max(0) as usize,
                                0,
                            );
                            if line >= 0
                                && !crate::scintilla::reveal_hidden_lines(
                                    notification.hwndFrom,
                                    line as usize,
                                )
                            {
                                enqueue(Event::BookmarkMargin(line as usize));
                            }
                        }
                    } else if notification.code == SCN_MARGINRIGHTCLICK {
                        let detail = &*(lp as *const crate::scintilla::Notification);
                        if detail.margin == 1 && detail.modifiers == 0 {
                            let search = GetSubMenu(crate::plugin_host::main_menu(hwnd), 2);
                            for index in 0..GetMenuItemCount(search) {
                                let submenu = GetSubMenu(search, index);
                                if !submenu.is_null()
                                    && GetMenuState(
                                        submenu,
                                        IDM_SEARCH_TOGGLE_BOOKMARK,
                                        MF_BYCOMMAND,
                                    ) != u32::MAX
                                {
                                    let mut point = POINT::default();
                                    GetCursorPos(&mut point);
                                    let command = TrackPopupMenu(
                                        submenu,
                                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                                        point.x,
                                        point.y,
                                        0,
                                        hwnd,
                                        null(),
                                    );
                                    if command != 0 {
                                        enqueue(Event::Command(command as u32));
                                    }
                                    break;
                                }
                            }
                        }
                    } else if notification.code == SCN_UPDATEUI {
                        let (start, end) = control_selection(notification.hwndFrom);
                        enqueue(Event::Selection(start, end));
                    }
                }
                if notification.idFrom == TAB_ID && notification.code == TCN_SELCHANGE {
                    enqueue(Event::Tab(
                        SendMessageW(notification.hwndFrom, TCM_GETCURSEL, 0, 0) as usize,
                    ));
                }
                0
            }
            WM_SIZE => {
                enqueue(Event::Resize);
                0
            }
            WM_SETFOCUS => {
                SetFocus(GetDlgItem(hwnd, EDIT_ID as i32));
                0
            }
            WM_CLOSE => {
                enqueue(Event::Close);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wp, lp),
        }
    }
}

type HiddenSectionsSnapshot = (Vec<(usize, usize)>, std::sync::Arc<()>);
type RecentMenuSnapshot = (usize, i32, bool, Vec<PathBuf>);

struct App {
    applied_language: Option<u32>,
    fold_states: std::collections::HashMap<BufferId, Vec<usize>>,
    hidden_sections: std::collections::HashMap<BufferId, HiddenSectionsSnapshot>,
    rtl_buffers: std::collections::HashSet<BufferId>,
    native_selections:
        std::collections::HashMap<BufferId, (Vec<u8>, Selection, std::sync::Arc<()>)>,
    show_non_printing: Cell<bool>,
    show_controls: Cell<bool>,
    copy_cut_line: bool,
    double_click_close: bool,
    exit_last_tab: bool,
    lock_tab_drag: bool,
    multiline_tabs: bool,
    content_tab_names: bool,
    persist_history: bool,
    recent_label_length: i32,
    caret_width: usize,
    current_line_mode: usize,
    current_line_frame: usize,
    fold_symbol_style: usize,
    edge_columns: Vec<usize>,
    edge_background: bool,
    short_title: bool,
    save_all_confirm: bool,
    compact_tab_length: u32,
    tab_close_buttons: bool,
    inactive_tab_buttons: bool,
    search_engine: String,
    horizontal_scroll: std::collections::HashMap<BufferId, (usize, usize)>,
    user_languages: Vec<crate::languages::Language>,
    languages: std::collections::HashMap<BufferId, u32>,
    default_languages: std::collections::HashSet<BufferId>,
    fullscreen: Option<(isize, RECT)>,
    post_it: Option<(isize, bool, bool, bool, bool, bool)>,
    distraction_free: Option<(isize, isize, bool)>,
    last_regex_empty: Option<(BufferId, usize, String)>,
    word_wrap: bool,
    macro_recording: bool,
    macro_steps: Vec<crate::macros::Step>,
    run_command: String,
    window: HWND,
    edit: HWND,
    tabs: HWND,
    status: HWND,
    workspace: Workspace,
    launch_availability: std::cell::RefCell<Option<(PathBuf, bool, bool)>>,
    selection_gesture: npp_editor_core::SelectionGesture,
    column_selection_gesture: bool,
    multi_select_flags: usize,
    projection: Projection,
    displayed_buffer: Option<BufferId>,
    first_visible_lines: std::collections::HashMap<BufferId, usize>,
    search_dialog: Option<SearchDialog>,
    find_files: Option<crate::find_files::FindFiles>,
    goto_dialog: Option<crate::goto_dialog::GoToDialog>,
    search_text: String,
    search_replacement: String,
    search_options: SearchOptions,
    search_in_selection: bool,
    tab_width: std::num::NonZeroUsize,
    indentation_style: IndentationStyle,
    basic_auto_indent: bool,
    advanced_indent: bool,
    new_document_eol: usize,
    new_document_encoding: usize,
    new_document_language: u32,
    check_history_files: bool,
    recent_in_submenu: bool,
    selected_text_foreground: bool,
    sql_backslash_escape: bool,
    compact_language_menu: bool,
    excluded_languages: Vec<u32>,
    recent_menu_snapshot: RefCell<Option<RecentMenuSnapshot>>,
    zoom: i32,
    wheel_remainder: i32,
    horizontal_wheel_remainder: i32,
    find_files_prefer_disk: bool,
    rendering_mode: usize,
    date_first: bool,
    date_format: String,
    zoom_font: HFONT,
    attribute_check: Option<crate::file_attributes::AttributeCheck>,
    disk_states: std::collections::HashMap<BufferId, crate::file_attributes::DiskState>,
    plugins: PluginRegistry,
    plugin_menu: HMENU,
    plugin_accelerators: crate::plugin_accelerators::PluginAccelerators,
}
impl App {
    fn create(visible: bool) -> io::Result<Self> {
        crate::scintilla::register()?;
        // SAFETY: All windows are created and operated on the current UI thread.
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: w!("NppRustMain"),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err(io::Error::last_os_error());
            }
            let common = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_TAB_CLASSES | ICC_BAR_CLASSES,
            };
            if InitCommonControlsEx(&common) == 0 {
                return Err(io::Error::last_os_error());
            }
            let window = CreateWindowExW(
                0,
                class.lpszClassName,
                w!("Notepad++ Rust"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1100,
                760,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if window.is_null() {
                return Err(io::Error::last_os_error());
            }
            let tabs = CreateWindowExW(
                0,
                WC_TABCONTROLW,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                100,
                28,
                window,
                TAB_ID as HMENU,
                instance,
                null(),
            );
            let edit = CreateWindowExW(
                0,
                w!("Scintilla"),
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WS_HSCROLL
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_AUTOHSCROLL as u32
                    | ES_NOHIDESEL as u32,
                0,
                28,
                100,
                100,
                window,
                EDIT_ID as HMENU,
                instance,
                null(),
            );
            let status = CreateWindowExW(
                0,
                w!("STATIC"),
                w!(""),
                WS_CHILD | WS_VISIBLE,
                0,
                128,
                100,
                24,
                window,
                STATUS_ID as HMENU,
                instance,
                null(),
            );
            if tabs.is_null()
                || edit.is_null()
                || status.is_null()
                || SetWindowSubclass(edit, Some(edit_proc), 1, 0) == 0
                || !crate::tabs::install(tabs)
            {
                let error = io::Error::last_os_error();
                DestroyWindow(window);
                return Err(error);
            }
            crate::scintilla::install(edit)?;
            SendMessageW(edit, EM_LIMITTEXT, 0x7fff_fffe, 0);
            SendMessageW(
                edit,
                WM_SETFONT,
                GetStockObject(ANSI_FIXED_FONT) as usize,
                1,
            );
            for control in [tabs, status] {
                SendMessageW(
                    control,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            let menu = create_menu();
            crate::editor_chrome::create(window)?;
            SetMenu(window, menu);
            crate::plugin_host::publish_main_menu(window, menu);
            CheckMenuItem(menu, SHOW_MENU_BAR, MF_BYCOMMAND | MF_CHECKED);
            CheckMenuItem(menu, SEARCH_WRAP, MF_BYCOMMAND | MF_CHECKED);
            DragAcceptFiles(window, 1);
            let mut app = Self {
                applied_language: None,
                fold_states: Default::default(),
                hidden_sections: Default::default(),
                rtl_buffers: Default::default(),
                native_selections: Default::default(),
                show_non_printing: Cell::new(false),
                show_controls: Cell::new(true),
                copy_cut_line: true,
                double_click_close: false,
                exit_last_tab: false,
                lock_tab_drag: false,
                multiline_tabs: false,
                content_tab_names: false,
                persist_history: visible,
                recent_label_length: -1,
                caret_width: 1,
                current_line_mode: 1,
                current_line_frame: 1,
                fold_symbol_style: 4,
                edge_columns: Vec::new(),
                edge_background: false,
                short_title: false,
                save_all_confirm: true,
                compact_tab_length: 0,
                tab_close_buttons: true,
                inactive_tab_buttons: false,
                search_engine: crate::preferences::Preferences::default().search_engine,
                horizontal_scroll: Default::default(),
                user_languages: crate::languages::load_user(),
                languages: Default::default(),
                default_languages: Default::default(),
                fullscreen: None,
                post_it: None,
                distraction_free: None,
                last_regex_empty: None,
                word_wrap: false,
                macro_recording: false,
                macro_steps: Vec::new(),
                run_command: String::new(),
                window,
                edit,
                tabs,
                status,
                workspace: Workspace::default(),
                launch_availability: Default::default(),
                selection_gesture: npp_editor_core::SelectionGesture::default(),
                column_selection_gesture: false,
                multi_select_flags: 0,
                projection: Projection::new(""),
                displayed_buffer: None,
                first_visible_lines: Default::default(),
                search_dialog: None,
                find_files: None,
                goto_dialog: None,
                search_text: String::new(),
                search_replacement: String::new(),
                search_options: SearchOptions::default(),
                search_in_selection: false,
                tab_width: std::num::NonZeroUsize::new(4).unwrap(),
                indentation_style: IndentationStyle::default(),
                basic_auto_indent: true,
                advanced_indent: true,
                new_document_eol: 0,
                new_document_encoding: 0,
                new_document_language: IDM_LANG_TEXT,
                check_history_files: false,
                recent_in_submenu: true,
                selected_text_foreground: false,
                sql_backslash_escape: true,
                compact_language_menu: true,
                excluded_languages: Vec::new(),
                recent_menu_snapshot: RefCell::new(None),
                zoom: i32::MIN,
                wheel_remainder: 0,
                horizontal_wheel_remainder: 0,
                find_files_prefer_disk: false,
                rendering_mode: 0,
                date_first: false,
                date_format: crate::date_time::DEFAULT_CUSTOM_FORMAT.to_owned(),
                zoom_font: null_mut(),
                attribute_check: None,
                disk_states: Default::default(),
                plugins: PluginRegistry::default(),
                plugin_menu: null_mut(),
                plugin_accelerators: Default::default(),
            };
            app.rebuild_plugin_menu()?;
            app.workspace.add(Document::new());
            app.set_zoom(0)?;
            app.set_tab_width(4)?;
            app.refresh();
            app.resize();
            app.apply_preferences(&crate::preferences::Preferences::default())?;
            if visible {
                match crate::preferences::load() {
                    Ok(settings) => app.apply_preferences(&settings)?,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => app.report(format!("Cannot load preferences: {error}")),
                }
                if !crate::scintilla::configure_rendering(app.edit, app.rendering_mode) {
                    app.report("The selected DirectWrite rendering mode could not be loaded. The editor is using GDI. You can change the mode in Preferences > Rendering.");
                }
                match crate::recent_files::load(&mut app.workspace) {
                    Ok(()) => {
                        if app.check_history_files {
                            let missing: Vec<_> = app
                                .workspace
                                .recent_files()
                                .filter(|entry| !entry.path.is_file())
                                .map(|entry| entry.path.clone())
                                .collect();
                            for path in missing {
                                app.workspace.forget_recent_path(&path);
                            }
                            app.save_recent_files();
                        }
                        app.caption();
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => app.report(format!("Cannot load recent files: {error}")),
                }
                let document = app.new_document();
                *app.document_mut() = document;
                app.languages
                    .insert(app.active_id(), app.new_document_language);
                app.default_languages.insert(app.active_id());
                app.refresh();
                ShowWindow(window, SW_SHOW);
                SetFocus(app.edit);
            }
            Ok(app)
        }
    }
    fn save_recent_files(&self) {
        if self.persist_history {
            if let Err(error) = crate::recent_files::save(&self.workspace) {
                self.report(format!("Cannot save recent files: {error}"));
            }
        }
    }
    fn active_id(&self) -> BufferId {
        self.workspace
            .active()
            .expect("editor always has a document")
    }
    fn set_tab_width(&mut self, width: usize) -> io::Result<()> {
        let width = std::num::NonZeroUsize::new(width)
            .filter(|width| (1..=32).contains(&width.get()))
            .ok_or_else(|| io::Error::other("Unsupported tab width"))?;
        let spacing = (width.get() * 4) as u32;
        // SAFETY: The multiline edit and menu belong to this UI thread. The
        // synchronous message reads one live unsigned dialog-unit distance.
        unsafe {
            if SendMessageW(self.edit, EM_SETTABSTOPS, 1, &spacing as *const _ as isize) == 0 {
                return Err(io::Error::other("Could not set tab spacing"));
            }
            InvalidateRect(self.edit, null(), 1);
            let command = match width.get() {
                2 => TAB_WIDTH_2,
                4 => TAB_WIDTH_4,
                8 => TAB_WIDTH_8,
                _ => 0,
            };
            CheckMenuRadioItem(
                crate::plugin_host::main_menu(self.window),
                TAB_WIDTH_2,
                TAB_WIDTH_8,
                command,
                MF_BYCOMMAND,
            );
        }
        self.tab_width = width;
        self.caption();
        Ok(())
    }
    fn set_zoom(&mut self, zoom: i32) -> io::Result<()> {
        let zoom = zoom.clamp(-10, 60);
        if zoom == self.zoom {
            return Ok(());
        }
        // SAFETY: GDI objects and window are used on the owning UI thread. The
        // old owned font is deleted only after it is deselected from the edit.
        unsafe {
            let font = {
                let mut description = LOGFONTW::default();
                for (target, unit) in description
                    .lfFaceName
                    .iter_mut()
                    .zip("Consolas".encode_utf16())
                {
                    *target = unit;
                }
                let dc = GetDC(self.edit);
                if dc.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let dpi = GetDeviceCaps(dc, LOGPIXELSY as i32);
                ReleaseDC(self.edit, dc);
                description.lfHeight = -((11 + zoom).max(3) * dpi / 72).max(2);
                description.lfWidth = 0;
                description.lfQuality = CLEARTYPE_QUALITY;
                // The stock fixed font can be a bitmap face with one size.
                // Request its scalable TrueType counterpart for zoomed text.
                description.lfOutPrecision = OUT_TT_ONLY_PRECIS;
                let created = CreateFontIndirectW(&description);
                if created.is_null() {
                    return Err(io::Error::last_os_error());
                }
                created
            };
            let margins = SendMessageW(self.edit, EM_GETMARGINS, 0, 0);
            SendMessageW(self.edit, WM_SETFONT, font as usize, 1);
            SendMessageW(
                self.edit,
                EM_SETMARGINS,
                (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize,
                margins,
            );
            let previous = self.zoom_font;
            self.zoom_font = font;
            self.zoom = zoom;
            if !previous.is_null() {
                DeleteObject(previous);
            }
            // EDIT caches tab positions in pixels; recompute them for this font.
            let spacing = (self.tab_width.get() * 4) as u32;
            SendMessageW(self.edit, EM_SETTABSTOPS, 1, &spacing as *const _ as isize);
            SendMessageW(self.edit, EM_SCROLLCARET, 0, 0);
        }
        Ok(())
    }
    fn document(&self) -> &Document {
        self.workspace.get(self.active_id()).unwrap()
    }
    fn launch_file_with(
        &self,
        action: crate::file_launch::Action,
        launch: impl FnOnce(&Path, crate::file_launch::Action) -> io::Result<()>,
    ) -> io::Result<()> {
        if let Some(path) = self.document().path() {
            launch(path, action)?;
        }
        Ok(())
    }
    fn copy_file_path(
        &self,
        part: crate::path_clipboard::Part,
        writer: impl FnOnce(&std::ffi::OsStr) -> io::Result<()>,
    ) -> io::Result<()> {
        let path = self
            .document()
            .path()
            .map(|path| path.as_os_str())
            .unwrap_or_else(|| {
                std::ffi::OsStr::new(
                    self.workspace
                        .untitled_name(self.active_id())
                        .expect("untitled name"),
                )
            });
        writer(&crate::path_clipboard::text(path, part))
    }

    fn paste_bookmarked_lines(
        &mut self,
        reader: impl FnOnce() -> io::Result<Option<String>>,
    ) -> io::Result<()> {
        if self.document().is_read_only() {
            return Ok(());
        }
        let Some(text) = reader()? else {
            return Ok(());
        };
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        self.document_mut()
            .paste_to_bookmarked_lines(&text)
            .map_err(io::Error::other)?;
        self.refresh();
        Ok(())
    }

    fn copy_bookmarked_lines(&self, writer: impl FnOnce(&str) -> io::Result<()>) -> io::Result<()> {
        writer(&self.document().bookmarked_text())
    }

    fn cut_bookmarked_lines(
        &mut self,
        writer: impl FnOnce(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let text = self
            .document_mut()
            .cut_bookmarked_lines()
            .map_err(io::Error::other)?;
        self.refresh();
        // Source cutMarkedLines finishes its undo group before publishing text.
        writer(&text)
    }
    fn document_mut(&mut self) -> &mut Document {
        let document = self.workspace.get_mut(self.active_id()).unwrap();
        document.track_selection_gesture(self.selection_gesture.clone());
        document
    }
    fn report(&self, error: impl std::fmt::Display) {
        // SAFETY: Owner and temporary strings live throughout the modal call.
        unsafe {
            MessageBoxW(
                self.window,
                wide(&error.to_string()).as_ptr(),
                w!("Notepad++ Rust"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    fn resize(&self) {
        resize_controls(self.window);
    }
    fn rebuild_language_layout(&self) {
        unsafe {
            let menu = GetSubMenu(crate::plugin_host::main_menu(self.window), 5);
            if menu.is_null() {
                return;
            }
            while GetMenuItemCount(menu) > 0 {
                DeleteMenu(menu, 0, MF_BYPOSITION);
            }
            AppendMenuW(
                menu,
                MF_STRING,
                IDM_LANG_TEXT as usize,
                w!("None (Normal Text)"),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, null());
            let mut languages: Vec<_> = crate::languages::LANGUAGES
                .iter()
                .filter(|language| {
                    language.command != IDM_LANG_TEXT
                        && !self.excluded_languages.contains(&language.command)
                })
                .collect();
            languages.sort_by_key(|language| language.name.to_lowercase());
            let mut groups = std::collections::BTreeMap::new();
            for language in languages {
                let target = if self.compact_language_menu {
                    let initial = language
                        .name
                        .chars()
                        .next()
                        .unwrap_or('#')
                        .to_ascii_uppercase();
                    *groups.entry(initial).or_insert_with(|| {
                        let popup = CreatePopupMenu();
                        AppendMenuW(
                            menu,
                            MF_POPUP,
                            popup as usize,
                            wide(&initial.to_string()).as_ptr(),
                        );
                        popup
                    })
                } else {
                    menu
                };
                crate::languages::append_language(target, language);
            }
            crate::languages::append_user_menu(menu);
            DrawMenuBar(self.window);
        }
    }
    fn apply_language(&mut self) {
        let language = self.language().command;
        if self.applied_language != Some(language) {
            crate::scintilla::language(self.edit, self.language());
            crate::scintilla::control_characters(self.edit, self.show_controls.get(), language);
            if language == IDM_LANG_SQL {
                crate::scintilla::sql_backslash_escape(self.edit, self.sql_backslash_escape);
            }
            self.applied_language = Some(language);
        }
    }
    fn language(&self) -> &crate::languages::Language {
        self.languages
            .get(&self.active_id())
            .and_then(|&id| {
                self.user_languages
                    .iter()
                    .find(|l| l.command == id)
                    .or_else(|| crate::languages::by_command(id))
            })
            .unwrap_or_else(|| {
                let extension = self
                    .document()
                    .path()
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                self.user_languages
                    .iter()
                    .find(|l| {
                        !extension.is_empty()
                            && l.extensions
                                .split_whitespace()
                                .any(|e| e.eq_ignore_ascii_case(extension))
                    })
                    .unwrap_or_else(|| crate::languages::detect(self.document().path()))
            })
    }
    fn refresh_recent_menu(&self) {
        unsafe {
            let file = GetSubMenu(crate::plugin_host::main_menu(self.window), 0);
            if file.is_null() {
                return;
            }
            if self.recent_menu_snapshot.borrow().as_ref().is_some_and(
                |(menu, length, submenu, paths)| {
                    *menu == file as usize
                        && *length == self.recent_label_length
                        && *submenu == self.recent_in_submenu
                        && paths
                            .iter()
                            .eq(self.workspace.recent_files().map(|entry| &entry.path))
                },
            ) {
                return;
            }
            // Remove only our history items; preserve all other File commands.
            for index in (0..GetMenuItemCount(file)).rev() {
                let submenu = GetSubMenu(file, index);
                let id = GetMenuItemID(file, index);
                if (!submenu.is_null()
                    && GetMenuState(submenu, IDM_CLEAN_RECENT_FILE_LIST, MF_BYCOMMAND) != u32::MAX)
                    || (RECENT_FILE_FIRST..=RECENT_FILE_LAST).contains(&id)
                    || matches!(id, IDM_OPEN_ALL_RECENT_FILE | IDM_CLEAN_RECENT_FILE_LIST)
                {
                    DeleteMenu(file, index as u32, MF_BYPOSITION);
                }
            }
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }
            let entries: Vec<_> = self.workspace.recent_files().collect();
            for (index, entry) in entries.iter().enumerate() {
                let raw = entry.path.to_string_lossy();
                let path = if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
                    format!(r"\\{unc}")
                } else {
                    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_owned()
                };
                let path = match self.recent_label_length {
                    0 => entry
                        .path
                        .file_name()
                        .unwrap_or(entry.path.as_os_str())
                        .to_string_lossy()
                        .into_owned(),
                    limit if limit > 0 => crate::tabs::compact_label(&path, limit as u32),
                    _ => path,
                };
                let label = format!(
                    "{}{} {}",
                    if index < 9 { "&" } else { "" },
                    index + 1,
                    path.replace('&', "&&")
                );
                AppendMenuW(
                    menu,
                    MF_STRING,
                    (RECENT_FILE_FIRST + index as u32) as usize,
                    wide(&label).as_ptr(),
                );
            }
            let flags = MF_STRING
                | if entries.is_empty() {
                    MF_GRAYED
                } else {
                    MF_ENABLED
                };
            AppendMenuW(
                menu,
                flags,
                IDM_OPEN_ALL_RECENT_FILE as usize,
                w!("Open All Recent Files"),
            );
            AppendMenuW(
                menu,
                flags,
                IDM_CLEAN_RECENT_FILE_LIST as usize,
                w!("Clear Recent Files List"),
            );
            let position = (0..GetMenuItemCount(file))
                .find(|&index| GetMenuItemID(file, index) == IDM_FILE_RESTORELASTCLOSEDFILE)
                .unwrap_or(GetMenuItemCount(file)) as u32;
            if self.recent_in_submenu {
                InsertMenuW(
                    file,
                    position,
                    MF_BYPOSITION | MF_POPUP | MF_STRING,
                    menu as usize,
                    w!("Recent Files"),
                );
            } else {
                for index in 0..GetMenuItemCount(menu) {
                    let mut label = vec![0u16; 32768];
                    let mut item = MENUITEMINFOW {
                        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                        fMask: MIIM_ID | MIIM_STATE | MIIM_STRING | MIIM_FTYPE,
                        dwTypeData: label.as_mut_ptr(),
                        cch: label.len() as u32,
                        ..Default::default()
                    };
                    if GetMenuItemInfoW(menu, index as u32, 1, &mut item) != 0 {
                        InsertMenuItemW(file, position + index as u32, 1, &item);
                    }
                }
                DestroyMenu(menu);
            }
            *self.recent_menu_snapshot.borrow_mut() = Some((
                file as usize,
                self.recent_label_length,
                self.recent_in_submenu,
                self.workspace
                    .recent_files()
                    .map(|entry| entry.path.clone())
                    .collect(),
            ));
        }
    }
    fn caption(&self) {
        if !is_control_hidden(self.window, crate::editor_chrome::GUTTER as usize) {
            crate::scintilla::send(
                self.edit,
                crate::scintilla::ids::SCI_SETMARGINWIDTHN,
                0,
                crate::scintilla::line_number_width(self.edit),
            );
        }
        self.refresh_recent_menu();
        crate::scintilla::bookmarks(self.edit, self.document().bookmarked_lines());
        let matching_brace = crate::scintilla::highlight_braces(self.edit);
        unsafe {
            for command in [
                IDM_SEARCH_GOTOMATCHINGBRACE,
                IDM_SEARCH_SELECTMATCHINGBRACES,
            ] {
                EnableMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    command,
                    MF_BYCOMMAND
                        | if matching_brace {
                            MF_ENABLED
                        } else {
                            MF_GRAYED
                        },
                );
            }
        }
        // SAFETY: Menu belongs to the live window on this UI thread.
        unsafe {
            let menu = crate::plugin_host::main_menu(self.window);
            for (command, checked) in [
                (IDM_EDIT_RTL, self.rtl_buffers.contains(&self.active_id())),
                (IDM_EDIT_LTR, !self.rtl_buffers.contains(&self.active_id())),
            ] {
                CheckMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
                );
            }
            for language in crate::languages::LANGUAGES
                .iter()
                .chain(self.user_languages.iter())
            {
                CheckMenuItem(
                    menu,
                    language.command,
                    MF_BYCOMMAND
                        | if language.command == self.language().command {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                );
            }
            let path = self.document().path();
            for command in [
                IDM_VIEW_IN_FIREFOX,
                IDM_VIEW_IN_CHROME,
                IDM_VIEW_IN_EDGE,
                IDM_VIEW_IN_IE,
            ] {
                EnableMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    command,
                    MF_BYCOMMAND
                        | if path.is_some() {
                            MF_ENABLED
                        } else {
                            MF_GRAYED
                        },
                );
            }

            let mut availability = self.launch_availability.borrow_mut();
            if availability.as_ref().map(|entry| entry.0.as_path()) != path {
                *availability = path.map(|p| {
                    (
                        p.to_owned(),
                        p.is_file(),
                        crate::file_launch::has_association(p),
                    )
                });
            }
            for (command, enabled) in [
                (
                    IDM_FILE_OPEN_CMD,
                    path.and_then(Path::parent).is_some_and(Path::is_dir),
                ),
                (
                    IDM_FILE_OPEN_POWERSHELL,
                    path.and_then(Path::parent).is_some_and(Path::is_dir),
                ),
                (
                    IDM_FILE_OPEN_FOLDER,
                    availability.as_ref().is_some_and(|entry| entry.1),
                ),
                (
                    IDM_FILE_OPEN_DEFAULT_VIEWER,
                    availability.as_ref().is_some_and(|entry| entry.2),
                ),
            ] {
                EnableMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED },
                );
            }
            for (command, enabled) in [
                (IDM_MACRO_STARTRECORDINGMACRO, !self.macro_recording),
                (IDM_MACRO_STOPRECORDINGMACRO, self.macro_recording),
                (
                    IDM_MACRO_PLAYBACKRECORDEDMACRO,
                    !self.macro_recording
                        && !self.macro_steps.is_empty()
                        && !self.document().is_read_only(),
                ),
                (
                    IDM_MACRO_RUNMULTIMACRODLG,
                    !self.macro_recording
                        && !self.macro_steps.is_empty()
                        && !self.document().is_read_only(),
                ),
                (IDM_MACRO_SAVECURRENTMACRO, !self.macro_steps.is_empty()),
            ] {
                EnableMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED },
                );
            }
            CheckMenuItem(
                menu,
                IDM_EDIT_BEGINENDSELECT,
                MF_BYCOMMAND
                    | if self.selection_gesture.is_started() && !self.column_selection_gesture {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
            CheckMenuItem(
                menu,
                IDM_EDIT_BEGINENDSELECT_COLUMNMODE,
                MF_BYCOMMAND
                    | if self.selection_gesture.is_started() && self.column_selection_gesture {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
            for (command, column) in [
                (IDM_EDIT_BEGINENDSELECT, false),
                (IDM_EDIT_BEGINENDSELECT_COLUMNMODE, true),
            ] {
                EnableMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND
                        | if self.selection_gesture.is_started()
                            && column != self.column_selection_gesture
                        {
                            MF_GRAYED
                        } else {
                            MF_ENABLED
                        },
                );
            }
            EnableMenuItem(
                menu,
                IDM_EDIT_TOGGLESYSTEMREADONLY,
                MF_BYCOMMAND
                    | if self.document().path().is_some() {
                        MF_ENABLED
                    } else {
                        MF_GRAYED
                    },
            );
            CheckMenuItem(
                menu,
                IDM_EDIT_TOGGLESYSTEMREADONLY,
                MF_BYCOMMAND
                    | if self.document().is_file_read_only() {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        // SAFETY: The main window owns this menu on the calling thread.
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                IDM_EDIT_TOGGLEREADONLY,
                MF_BYCOMMAND
                    | if self.document().is_user_read_only() {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        if let Some(dialog) = &self.search_dialog {
            if let Err(error) = dialog.set_read_only(self.document().is_read_only()) {
                self.report(error);
            }
        }
        // SAFETY: The live window owns this menu on the calling UI thread.
        unsafe {
            let menu = crate::plugin_host::main_menu(self.window);
            for &command in EDIT_COMMANDS {
                let enabled = !self.document().is_read_only()
                    && match command {
                        IDM_EDIT_UNDO => self.document().can_undo(),
                        IDM_EDIT_REDO => self.document().can_redo(),
                        FORMAT_SELECTION => !self.document().selection().range().is_empty(),
                        IDM_EDIT_BLOCK_COMMENT
                        | IDM_EDIT_BLOCK_COMMENT_SET
                        | IDM_EDIT_BLOCK_UNCOMMENT => !self.language().line.is_empty(),
                        IDM_EDIT_STREAM_COMMENT | IDM_EDIT_STREAM_UNCOMMENT => {
                            !self.language().open.is_empty() && !self.language().close.is_empty()
                        }
                        _ => true,
                    };
                EnableMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED },
                );
                SendMessageW(
                    GetDlgItem(self.window, crate::editor_chrome::TOOLBAR),
                    TB_ENABLEBUTTON,
                    command as usize,
                    enabled as isize,
                );
            }
        }
        let mut scroll_lines = self.first_visible_lines.clone();
        // SAFETY: Read-only query of the active native EDIT control.
        let first_line =
            unsafe { SendMessageW(self.edit, EM_GETFIRSTVISIBLELINE, 0, 0) }.max(0) as usize;
        scroll_lines.insert(self.active_id(), first_line);
        crate::plugin_host::publish(self.window, &self.workspace, self.tab_width, &scroll_lines);
        let doc = self.document();
        let name = doc
            .path()
            .map(|path| {
                if self.short_title {
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .into_owned()
                } else {
                    crate::find_files::display_path(path)
                }
            })
            .unwrap_or_else(|| {
                self.workspace
                    .untitled_name(self.active_id())
                    .expect("untitled buffer name")
                    .to_owned()
            });
        let (line, column) = doc
            .line_column(doc.selection().caret, self.tab_width)
            .expect("valid document caret");
        let selection_status =
            if let Some(status) = crate::scintilla::multiple_selection_status(self.edit) {
                status
            } else if doc.selection().range().is_empty() {
                format!("Pos: {}", doc.selection().caret + 1)
            } else {
                let (characters, lines) = doc.selected_characters_and_lines();
                format!("Sel: {characters} | {lines}")
            };
        let mut status = format!(
            "Ln {}, Col {}   |   {}   |   {} bytes, {} lines   |   {:?}   |   {:?}{}",
            line + 1,
            column + 1,
            selection_status,
            doc.text().len(),
            doc.line_starts().len(),
            doc.encoding(),
            doc.line_ending(),
            if doc.is_read_only() {
                "   |   Read only"
            } else {
                ""
            }
        );
        status.push_str("   |   ");
        status.push_str(&self.language().name);
        use crate::file_attributes::DiskState;
        if doc.bookmarked_lines().any(|marked| marked == line) {
            status.push_str("   |   Bookmarked");
        }
        status.push_str(match self.disk_states.get(&self.active_id()) {
            Some(DiskState::Changed) => "   |   File changed on disk — Reload to read changes",
            Some(DiskState::Deleted) => "   |   File deleted on disk — buffer retained",
            Some(DiskState::Unavailable) => "   |   Unable to check file on disk",
            _ => "",
        });
        // SAFETY: Synchronous calls copy their temporary NUL-terminated strings.
        unsafe {
            SetWindowTextW(
                self.window,
                wide(&format!(
                    "{}{} — Notepad++ Rust",
                    if doc.is_dirty() { "*" } else { "" },
                    name
                ))
                .as_ptr(),
            );
            SetWindowTextW(self.status, wide(&status).as_ptr());
        }
    }
    fn refresh_tabs(&self) {
        // SAFETY: Each TCITEMW and its text remain valid during SendMessageW.
        unsafe {
            SendMessageW(self.tabs, WM_SETREDRAW, 0, 0);
            let count = self.workspace.buffers().count();
            while SendMessageW(self.tabs, TCM_GETITEMCOUNT, 0, 0) as usize > count {
                SendMessageW(self.tabs, TCM_DELETEITEM, count, 0);
            }
            for (index, (id, doc)) in self.workspace.buffers().enumerate() {
                let name = doc
                    .path()
                    .map(|p| {
                        p.file_name()
                            .unwrap_or(p.as_os_str())
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_else(|| {
                        self.workspace
                            .untitled_name(id)
                            .expect("untitled buffer name")
                            .to_owned()
                    });
                let name = crate::tabs::compact_label(&name, self.compact_tab_length);
                let mut text = wide(&format!(
                    "{}{}{}",
                    if doc.is_dirty() { "*" } else { "" },
                    name,
                    if self.tab_close_buttons
                        && (self.inactive_tab_buttons || self.workspace.active() == Some(id))
                    {
                        "    ×"
                    } else {
                        ""
                    }
                ));
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: text.as_mut_ptr(),
                    ..Default::default()
                };
                SendMessageW(
                    self.tabs,
                    if index < SendMessageW(self.tabs, TCM_GETITEMCOUNT, 0, 0) as usize {
                        TCM_SETITEMW
                    } else {
                        TCM_INSERTITEMW
                    },
                    index,
                    &item as *const _ as isize,
                );
                if self.workspace.active() == Some(id) {
                    SendMessageW(self.tabs, TCM_SETCURSEL, index, 0);
                }
            }
            SendMessageW(self.tabs, WM_SETREDRAW, 1, 0);
            InvalidateRect(self.tabs, null(), 0);
            resize_controls(self.window);
        }
    }
    fn refresh(&mut self) {
        if self.content_tab_names {
            self.workspace.name_from_first_line(self.active_id());
        }
        self.default_languages
            .retain(|id| self.workspace.get(*id).is_some());
        let active = self.active_id();
        let switching = self.displayed_buffer != Some(active);
        if switching {
            self.capture_scroll();
        } else {
            self.capture_hidden_sections();
        }
        self.hidden_sections
            .retain(|id, _| self.workspace.get(*id).is_some());
        self.first_visible_lines
            .retain(|id, _| self.workspace.get(*id).is_some());
        self.horizontal_scroll
            .retain(|id, _| self.workspace.get(*id).is_some());
        self.native_selections
            .retain(|id, _| self.workspace.get(*id).is_some());
        self.fold_states
            .retain(|id, _| self.workspace.get(*id).is_some());
        self.rtl_buffers
            .retain(|id| self.workspace.get(*id).is_some());
        crate::scintilla::text_direction(self.edit, self.rtl_buffers.contains(&active));
        self.projection = Projection::new(self.document().text());
        let selection = self.document().selection();
        UPDATING.set(true);
        // SAFETY: Notifications are suppressed while copying model state to UI.
        unsafe {
            crate::scintilla::set_text(self.edit, &self.projection.text);
            let eol = match self.document().line_ending() {
                LineEnding::CrLf => crate::scintilla::ids::SC_EOL_CRLF,
                LineEnding::Lf => crate::scintilla::ids::SC_EOL_LF,
                LineEnding::Cr => crate::scintilla::ids::SC_EOL_CR,
            };
            crate::scintilla::send(
                self.edit,
                crate::scintilla::ids::SCI_SETEOLMODE,
                eol as usize,
                0,
            );
            self.apply_language();
            SendMessageW(
                self.edit,
                EM_SETSEL,
                self.projection.utf16_offset(selection.anchor),
                self.projection.utf16_offset(selection.caret) as isize,
            );
            SendMessageW(
                self.edit,
                EM_SETREADONLY,
                self.document().is_read_only() as usize,
                0,
            );
            if switching {
                if let Some((snapshot, saved_selection, revision)) =
                    self.native_selections.get(&active)
                {
                    // Explicit navigation (search/session/plugin) takes precedence.
                    if *saved_selection == selection
                        && std::sync::Arc::ptr_eq(revision, &self.document().text_revision_token())
                    {
                        crate::scintilla::restore_selection(self.edit, snapshot);
                    }
                }
            }
            SendMessageW(self.edit, EM_SCROLLCARET, 0, 0);
            if switching {
                crate::scintilla::restore_folds(
                    self.edit,
                    self.fold_states.get(&active).map_or(&[], Vec::as_slice),
                );
            }
        }
        self.displayed_buffer = Some(active);
        if let Some((sections, revision)) = self.hidden_sections.get(&active) {
            if std::sync::Arc::ptr_eq(revision, &self.document().text_revision_token()) {
                crate::scintilla::restore_hidden_sections(self.edit, sections);
            }
        }
        if switching {
            self.restore_horizontal_scroll(active);
            if let Some(&line) = self.first_visible_lines.get(&active) {
                self.scroll_to_line(line);
            }
        }
        self.refresh_tabs();
        UPDATING.set(false);
        self.caption();
    }
    fn capture_scroll(&mut self) {
        self.capture_hidden_sections();
        if let Some(id) = self.displayed_buffer {
            if let Some(document) = self.workspace.get(id) {
                self.native_selections.insert(
                    id,
                    (
                        crate::scintilla::selection_snapshot(self.edit),
                        document.selection(),
                        document.text_revision_token(),
                    ),
                );
            }
            use crate::scintilla::{ids::*, send};
            self.horizontal_scroll.insert(
                id,
                (
                    send(self.edit, SCI_GETXOFFSET, 0, 0).max(0) as usize,
                    send(self.edit, SCI_GETSCROLLWIDTH, 0, 0).max(1) as usize,
                ),
            );
            self.fold_states
                .insert(id, crate::scintilla::collapsed(self.edit));
        }
        if let Some(id) = self.displayed_buffer {
            // SAFETY: Read-only query of our live EDIT control.
            let line =
                unsafe { SendMessageW(self.edit, EM_GETFIRSTVISIBLELINE, 0, 0) }.max(0) as usize;
            self.first_visible_lines.insert(id, line);
        }
    }
    fn add_new_document(&mut self) {
        let id = self.workspace.add(self.new_document());
        self.languages.insert(id, self.new_document_language);
        self.default_languages.insert(id);
    }
    fn new_document(&self) -> Document {
        Document::new_with_format(
            match self.new_document_eol {
                1 => LineEnding::Lf,
                2 => LineEnding::Cr,
                _ => LineEnding::CrLf,
            },
            crate::preferences::NEW_ENCODINGS[self.new_document_encoding].1,
        )
    }
    fn capture_hidden_sections(&mut self) {
        if let Some(id) = self.displayed_buffer {
            if let Some(document) = self.workspace.get(id) {
                // Model-driven edits may already have changed line positions.
                // Native edits update the projection before the next refresh.
                if self.projection.text == document.text() {
                    self.hidden_sections.insert(
                        id,
                        (
                            crate::scintilla::hidden_sections(self.edit),
                            document.text_revision_token(),
                        ),
                    );
                } else {
                    self.hidden_sections.remove(&id);
                }
            }
        }
    }
    fn scroll_to_line(&self, line: usize) {
        let line = line.min(self.document().line_starts().len().saturating_sub(1));
        use crate::scintilla::{ids::*, send};
        let visible = send(self.edit, SCI_VISIBLEFROMDOCLINE, line, 0).max(0);
        send(self.edit, SCI_SETFIRSTVISIBLELINE, visible as usize, 0);
    }
    fn restore_horizontal_scroll(&self, id: BufferId) {
        use crate::scintilla::{ids::*, send};
        let (offset, width) = self.horizontal_scroll.get(&id).copied().unwrap_or((0, 1));
        send(self.edit, SCI_SETSCROLLWIDTH, width, 0);
        send(self.edit, SCI_SETXOFFSET, offset, 0);
    }
    /// Use only when the document text and its native projection are unchanged.
    fn refresh_selection(&self) {
        let selection = self.document().selection();
        // SAFETY: Only the selection and caret visibility of our live edit change.
        unsafe {
            SendMessageW(
                self.edit,
                EM_SETSEL,
                self.projection.utf16_offset(selection.anchor),
                self.projection.utf16_offset(selection.caret) as isize,
            );
            SendMessageW(self.edit, EM_SCROLLCARET, 0, 0);
        }
        self.caption();
    }

    fn sync_native_selection(&mut self, start: usize, end: usize) -> Selection {
        let selection = self.projection.selection(start, end);
        // EM_GETSEL sorts endpoints. Keep our known direction while the native
        // range is unchanged, including key-up after a Rust navigation command.
        if selection.range() != self.document().selection().range() {
            let _ = self.document_mut().select(selection);
        }
        self.document().selection()
    }
    fn line_clipboard(
        &mut self,
        cut: bool,
        write: impl FnOnce(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        if cut && self.document().is_read_only() {
            return Ok(());
        }
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let range = self.document().selected_line_range();
        write(&self.document().text()[range])?;
        if cut {
            self.document_mut()
                .cut_selected_lines()
                .map_err(io::Error::other)?;
            self.refresh();
        }
        Ok(())
    }

    fn open(&mut self, path: &Path) -> io::Result<()> {
        self.open_as(path, None)
    }
    fn open_as(&mut self, path: &Path, encoding: Option<Encoding>) -> io::Result<()> {
        let path = std::fs::canonicalize(path)?;
        let existing = self
            .workspace
            .buffers()
            .find(|(_, doc)| doc.path() == Some(path.as_path()))
            .map(|(id, _)| id);
        if let Some(id) = existing {
            if encoding
                .is_some_and(|encoding| self.workspace.get(id).unwrap().encoding() != encoding)
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "This file is already open with a different encoding. Close its tab before reopening with an explicit encoding.",
                ));
            }
            self.workspace.activate(id);
        } else {
            let doc = Document::open_as(&path, encoding)?;
            let empty = self.document().path().is_none()
                && self.document().text().is_empty()
                && !self.document().is_dirty();
            let old = self.active_id();
            self.workspace.add(doc);
            if empty {
                let _ = self.workspace.close(old, false);
            }
        }
        self.save_recent_files();
        self.refresh();
        Ok(())
    }
    fn save(&mut self, save_as: bool) -> bool {
        self.save_mode(save_as, false)
    }
    fn insert_date_time_text(&mut self, text: &str) -> io::Result<()> {
        if self.document().is_read_only() {
            return Ok(());
        }
        let (start, end) = control_selection(self.edit);
        let selection = self.sync_native_selection(start, end);
        self.document_mut()
            .replace(selection.range(), text)
            .map_err(io::Error::other)?;
        self.refresh();
        Ok(())
    }
    fn save_mode(&mut self, save_as: bool, copy: bool) -> bool {
        let path = if save_as || self.document().path().is_none() {
            match file_dialog_titled(
                self.window,
                true,
                if copy { Some("Save a Copy As") } else { None },
            ) {
                Ok(Some(path)) => path,
                Ok(None) => return false,
                Err(e) => {
                    self.report(e);
                    return false;
                }
            }
        } else {
            self.document().path().unwrap().to_owned()
        };
        let write = |app: &mut Self, overwrite| {
            if copy {
                app.save_copy_to_checked(&path, overwrite)
            } else {
                app.save_to_checked(&path, overwrite)
            }
        };
        let success = match write(self, false) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // SAFETY: Prompt is owned by the live main window; cancellation
                // leaves the document and the external file unchanged.
                let overwrite = unsafe {
                    MessageBoxW(
                        self.window,
                        w!(
                            "The file changed or was deleted outside the editor. Overwrite it with this document?"
                        ),
                        w!("File Changed on Disk"),
                        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                    )
                } == IDYES;
                if overwrite {
                    match write(self, true) {
                        Ok(()) => true,
                        Err(e) => {
                            self.report(e);
                            false
                        }
                    }
                } else {
                    false
                }
            }
            Err(e) => {
                self.report(e);
                false
            }
        };
        self.caption();
        self.refresh_tabs();
        success
    }
    #[cfg(test)]
    fn save_to(&mut self, path: &Path) -> io::Result<()> {
        self.save_to_checked(path, false)
    }
    fn save_to_checked(&mut self, path: &Path, overwrite_external: bool) -> io::Result<()> {
        let path = self.checked_save_path(path, overwrite_external)?;
        let changed_path = self.document().path() != Some(path.as_path());
        self.document_mut().save_with(&path, atomic_write)?;
        if changed_path && self.default_languages.remove(&self.active_id()) {
            // A new-document default is not an explicit Language menu choice.
            self.languages.remove(&self.active_id());
            self.apply_language();
        }
        self.launch_availability.borrow_mut().take();
        self.disk_states.remove(&self.active_id());
        self.attribute_check = None;
        self.workspace.forget_recent_path(&path);
        self.save_recent_files();
        // A successful Save As can move a read-only document to a writable file.
        self.document_mut().set_file_read_only(false);
        // SAFETY: Edit HWND is owned by this App and receives a scalar flag.
        unsafe {
            SendMessageW(
                self.edit,
                EM_SETREADONLY,
                self.document().is_read_only() as usize,
                0,
            );
        }
        Ok(())
    }
    fn save_copy_to_checked(&self, path: &Path, overwrite_external: bool) -> io::Result<()> {
        let path = self.checked_save_path(path, overwrite_external)?;
        self.document().save_copy_with(&path, atomic_write)
    }
    fn checked_save_path(&self, path: &Path, overwrite_external: bool) -> io::Result<PathBuf> {
        let path = normalized_save_path(path)?;
        if self
            .workspace
            .buffers()
            .any(|(id, doc)| id != self.active_id() && doc.path() == Some(path.as_path()))
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "This file is open in another tab. Switch to that tab before saving it.",
            ));
        }
        if !overwrite_external && self.document().path() == Some(path.as_path()) {
            let changed = match std::fs::read(&path) {
                Ok(bytes) => self.document().disk_content_changed(&bytes),
                Err(e) if e.kind() == io::ErrorKind::NotFound => true,
                Err(e) => return Err(e),
            };
            if changed {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "file changed outside the editor",
                ));
            }
        }
        Ok(path)
    }
    fn save_all_with(&mut self, mut save: impl FnMut(&mut Self) -> bool) -> bool {
        let original = self.active_id();
        let ids: Vec<_> = self
            .workspace
            .buffers()
            .filter(|(_, doc)| doc.is_dirty())
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            self.workspace.activate(id);
            self.refresh();
            if !save(self) {
                return false;
            }
        }
        self.workspace.activate(original);
        self.refresh();
        true
    }
    fn close_documents(
        &mut self,
        ids: Vec<BufferId>,
        mut confirm: impl FnMut(&mut Self) -> bool,
    ) -> bool {
        let original = self.active_id();
        for id in ids {
            if !self.workspace.activate(id) {
                continue;
            }
            self.refresh();
            if !confirm(self) {
                return false;
            }
            let _ = self.workspace.close(id, true);
            self.save_recent_files();
        }
        if self.workspace.active().is_none() {
            self.add_new_document();
            if self.exit_last_tab {
                enqueue(Event::Close);
            }
        }
        self.workspace.activate(original);
        self.refresh();
        true
    }
    fn load_session_from(&mut self, path: &Path) -> io::Result<()> {
        // Parse the entire session before changing the workspace.
        let session = npp_editor_core::Session::from_xml(&std::fs::read(path)?)?;
        if session.main_view.files.is_empty() && session.sub_view.files.is_empty() {
            return Ok(());
        }
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let mut failures = Vec::new();
        let mut active = [None, None];
        let mut last = None;
        for (view_index, view) in [&session.main_view, &session.sub_view]
            .into_iter()
            .enumerate()
        {
            let mut loaded = Vec::new();
            for file in &view.files {
                let filename = file.filename().unwrap_or_default();
                let result = (|| {
                    self.open_as(Path::new(filename), file.explicit_encoding()?)?;
                    file.restore_document_state(self.document_mut())?;
                    if file
                        .attributes
                        .get("RTL")
                        .is_some_and(|value| value == "yes")
                    {
                        self.rtl_buffers.insert(self.active_id());
                    } else {
                        self.rtl_buffers.remove(&self.active_id());
                    }
                    if let Some(name) = file.attributes.get("lang") {
                        if let Some(language) = crate::languages::LANGUAGES
                            .iter()
                            .chain(self.user_languages.iter())
                            .find(|l| &l.name == name)
                        {
                            self.default_languages.remove(&self.active_id());
                            self.languages.insert(self.active_id(), language.command);
                        }
                    }
                    Ok::<_, io::Error>(())
                })();
                match result {
                    Ok(()) => {
                        let first_line = file
                            .attributes
                            .get("firstVisibleLine")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(0);
                        let state = (self.active_id(), first_line);
                        self.refresh();
                        crate::scintilla::restore_folds(self.edit, &file.folds);
                        let position = |name: &str, default| {
                            file.attributes
                                .get(name)
                                .and_then(|s| s.parse::<isize>().ok())
                                .filter(|&n| n >= 0)
                                .map_or(default, |n| n as usize)
                        };
                        self.horizontal_scroll.insert(
                            self.active_id(),
                            (position("xOffset", 0), position("scrollWidth", 1).max(1)),
                        );
                        self.restore_horizontal_scroll(self.active_id());
                        self.scroll_to_line(first_line);
                        self.capture_scroll();
                        loaded.push(state);
                        last = Some(state);
                    }
                    Err(error) => failures.push(format!("{filename}: {error}")),
                }
            }
            // The source removes failed entries before resolving activeIndex.
            active[view_index] = loaded
                .get(view.active_index)
                .copied()
                .or_else(|| loaded.last().copied());
        }
        // Until separate edit views exist, both view lists share the tab strip.
        // Preserve the requested active file when switching from either list.
        let selected = if session.active_view == 1 && active[1].is_some() {
            active[1]
        } else {
            active[0]
        }
        .or(last);
        if let Some((id, first_line)) = selected {
            self.workspace.activate(id);
            self.refresh();
            crate::scintilla::restore_folds(
                self.edit,
                self.fold_states.get(&id).map_or(&[], Vec::as_slice),
            );
            self.restore_horizontal_scroll(id);
            self.scroll_to_line(first_line);
        } else {
            self.refresh();
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Some session files could not be restored:\n{}",
                failures.join("\n")
            )))
        }
    }

    fn save_session_to(
        &mut self,
        path: &Path,
        writer: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        self.capture_scroll();
        let mut session = self.workspace.capture_session()?;
        // Manual source sessions omit untitled and missing files; document text
        // is not saved by this command.
        session.main_view.files.retain(|file| {
            file.filename()
                .is_some_and(|name| Path::new(name).is_file())
        });
        for file in &mut session.main_view.files {
            let id = self
                .workspace
                .buffers()
                .find(|(_, document)| document.path().and_then(Path::to_str) == file.filename())
                .map(|(id, _)| id);
            if let Some(id) = id {
                let language = self
                    .languages
                    .get(&id)
                    .and_then(|&cmd| {
                        self.user_languages
                            .iter()
                            .find(|l| l.command == cmd)
                            .or_else(|| crate::languages::by_command(cmd))
                    })
                    .unwrap_or_else(|| {
                        crate::languages::detect(self.workspace.get(id).and_then(|d| d.path()))
                    });
                file.attributes.insert("lang".into(), language.name.clone());
                file.attributes.insert(
                    "RTL".into(),
                    if self.rtl_buffers.contains(&id) {
                        "yes"
                    } else {
                        "no"
                    }
                    .into(),
                );
                file.folds = self.fold_states.get(&id).cloned().unwrap_or_default();
                let (offset, width) = self.horizontal_scroll.get(&id).copied().unwrap_or((0, 1));
                file.attributes.insert("xOffset".into(), offset.to_string());
                file.attributes
                    .insert("scrollWidth".into(), width.to_string());
            }
            if let Some(line) = id.and_then(|id| self.first_visible_lines.get(&id)) {
                file.attributes
                    .insert("firstVisibleLine".into(), line.to_string());
            }
        }
        let bytes = session.to_xml()?;
        npp_editor_core::Session::from_xml(&bytes)?;
        writer(&normalized_save_path(path)?, &bytes)
    }

    fn reload(&mut self) -> io::Result<()> {
        let Some(path) = self.document().path().map(Path::to_owned) else {
            return Ok(());
        };
        let bytes = std::fs::read(&path)?;
        let readonly = std::fs::metadata(&path)?.permissions().readonly();
        self.document_mut()
            .reload_from_bytes(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.document_mut().set_file_read_only(readonly);
        self.disk_states.remove(&self.active_id());
        self.attribute_check = None;
        self.refresh();
        Ok(())
    }
    fn confirm_close(&mut self) -> bool {
        if !self.document().is_dirty() {
            return true;
        }
        // SAFETY: Modal prompt is owned by the live main window.
        let choice = unsafe {
            MessageBoxW(
                self.window,
                w!("Save changes to this document?"),
                w!("Notepad++ Rust"),
                MB_YESNOCANCEL | MB_ICONQUESTION,
            )
        };
        match choice {
            IDYES => self.save(false),
            IDNO => true,
            _ => false,
        }
    }
    fn show_find_files(&mut self) -> io::Result<()> {
        if let Some(dialog) = &self.search_dialog {
            dialog.hide();
        }
        if let Some(query) = self
            .search_dialog
            .as_ref()
            .and_then(SearchDialog::current_query)
        {
            self.search_text = query.needle;
        }
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let selected = &self.document().text()[self.document().selection().range()];
        let needle = if selected.is_empty() {
            self.search_text.clone()
        } else {
            selected.to_owned()
        };
        let directory = self
            .document()
            .path()
            .and_then(Path::parent)
            .map(Path::to_owned)
            .unwrap_or(std::env::current_dir()?);
        if self.find_files.is_none() {
            self.find_files = Some(crate::find_files::FindFiles::new(self.window)?);
        }
        self.find_files.as_ref().unwrap().show(&needle, &directory);
        Ok(())
    }
    fn open_find_result(&mut self, index: usize) -> io::Result<()> {
        let Some(hit) = self
            .find_files
            .as_ref()
            .and_then(|search| search.hit(index))
        else {
            return Ok(());
        };
        self.open_as(
            &hit.path,
            if hit.encoding.requires_explicit_decode() {
                Some(hit.encoding)
            } else {
                None
            },
        )?;
        let boundary = |offset: usize| {
            let mut offset = offset.min(self.document().text().len());
            while !self.document().text().is_char_boundary(offset) {
                offset -= 1;
            }
            offset
        };
        let selection = Selection {
            anchor: boundary(hit.range.start),
            caret: boundary(hit.range.end),
        };
        self.document_mut()
            .select(selection)
            .map_err(io::Error::other)?;
        self.refresh_selection();
        // SAFETY: Owned result list and edit control, scalar selection/focus.
        unsafe {
            if let Some(search) = &self.find_files {
                SendMessageW(search.list, LB_SETCURSEL, index, 0);
            }
            SetFocus(self.edit);
        }
        Ok(())
    }
    fn show_search(&mut self, replace: bool) {
        if let Some(dialog) = &self.find_files {
            unsafe {
                ShowWindow(dialog.dialog, SW_HIDE);
            }
        }
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let _ = self.document_mut().expand_word_selection();
        self.refresh_selection();
        let selected = &self.document().text()[self.document().selection().range()];
        let initial =
            (!selected.is_empty() && selected.chars().count() <= 1024).then(|| selected.to_owned());
        if let Some(dialog) = &self.search_dialog {
            if dialog.show_existing(replace, initial.as_deref()) {
                return;
            }
            if let Some(query) = dialog.current_query() {
                self.search_text = query.needle;
                if dialog.is_replace() {
                    self.search_replacement = query.replacement;
                }
                self.search_options.match_case = query.options.match_case;
                self.search_options.whole_word = query.options.whole_word;
                if !dialog.is_replace() {
                    self.search_options.direction = query.options.direction;
                }
            }
        }
        let initial = initial.as_deref().unwrap_or(&self.search_text);
        match SearchDialog::open(self.window, replace, initial, self.search_options) {
            Ok(dialog) => {
                dialog.set_replacement_text(&self.search_replacement);
                if let Err(error) = dialog.set_read_only(self.document().is_read_only()) {
                    self.report(error);
                }
                self.search_dialog = Some(dialog);
            }
            Err(e) => self.report(e),
        }
    }
    fn select_search_match(
        &mut self,
        found: std::ops::Range<usize>,
        start: usize,
        direction: SearchDirection,
    ) {
        let wrapped = match direction {
            SearchDirection::Forward => found.start < start,
            SearchDirection::Backward => found.end > start,
        };
        let _ = self.document_mut().select(Selection {
            anchor: found.start,
            caret: found.end,
        });
        self.refresh_selection();
        if wrapped {
            let message = match direction {
                SearchDirection::Forward => "Reached end of page, continued from top",
                SearchDirection::Backward => "Reached top of page, continued from bottom",
            };
            // SAFETY: The owned status control copies this terminated string.
            unsafe {
                SetWindowTextW(self.status, wide(message).as_ptr());
            }
        }
    }

    fn repeat_find(&mut self, direction: SearchDirection) {
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        if let Some(query) = self
            .search_dialog
            .as_ref()
            .and_then(SearchDialog::current_query)
        {
            self.search_options.match_case = query.options.match_case;
            self.search_options.whole_word = query.options.whole_word;
        }
        self.find(direction);
    }

    fn find(&mut self, direction: SearchDirection) {
        self.find_using_mode(direction, self.search_options.mode);
    }
    fn find_using_mode(&mut self, direction: SearchDirection, mode: SearchMode) {
        self.find_using_mode_from(direction, mode, false);
    }

    fn find_using_mode_from(
        &mut self,
        direction: SearchDirection,
        mode: SearchMode,
        include_selection: bool,
    ) {
        if self.search_text.is_empty() {
            self.show_search(false);
            return;
        }
        let options = SearchOptions {
            direction,
            mode,
            ..self.search_options
        };
        let mut start = self.document().search_start(direction, include_selection);
        if mode == SearchMode::Regex
            && !include_selection
            && self
                .last_regex_empty
                .as_ref()
                .is_some_and(|(id, at, query)| {
                    *id == self.active_id() && *at == start && query == &self.search_text
                })
        {
            start = match direction {
                SearchDirection::Forward => match self.document().text()[start..].chars().next() {
                    Some(ch) => start + ch.len_utf8(),
                    None if options.wrap => 0,
                    None => return,
                },
                SearchDirection::Backward => {
                    match self.document().text()[..start].char_indices().next_back() {
                        Some((at, _)) => at,
                        None if options.wrap => self.document().text().len(),
                        None => return,
                    }
                }
            };
        }
        match self.document().search(&self.search_text, start, options) {
            Ok(Some(found)) => {
                self.last_regex_empty = (mode == SearchMode::Regex && found.is_empty())
                    .then(|| (self.active_id(), found.start, self.search_text.clone()));
                self.select_search_match(found, start, direction);
            }
            Ok(None) => {
                // SAFETY: Live status control copies the temporary string.
                unsafe {
                    SetWindowTextW(self.status, wide("No match found").as_ptr());
                }
            }
            Err(e) => self.report(e),
        }
    }
    fn set_and_find(&mut self, direction: SearchDirection) {
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let _ = self.document_mut().expand_word_selection();
        self.refresh_selection();
        let selected = &self.document().text()[self.document().selection().range()];
        if selected.is_empty() || selected.chars().count() > 1024 {
            return;
        }
        let needle = selected.to_owned();
        if let Some(dialog) = &self.search_dialog {
            if let Some(query) = dialog.current_query() {
                self.search_options.match_case = query.options.match_case;
                self.search_options.whole_word = query.options.whole_word;
            }
            dialog.set_query_text(&needle);
        }
        self.search_text = needle;
        self.find_using_mode(direction, SearchMode::Normal);
    }

    fn find_selected(&mut self, direction: SearchDirection) {
        let (start, end) = control_selection(self.edit);
        self.sync_native_selection(start, end);
        let _ = self.document_mut().expand_word_selection();
        let selected = self.document().selection().range();
        let needle = &self.document().text()[selected.clone()];
        if needle.is_empty() {
            return;
        }
        let options = SearchOptions {
            direction,
            ..SearchOptions::default()
        };
        let start = self.document().search_start(direction, false);
        match self.document().search(needle, start, options) {
            Ok(Some(found)) => {
                self.select_search_match(found, start, direction);
            }
            Ok(None) => self.refresh_selection(),
            Err(error) => self.report(error),
        }
    }

    fn search_request(&mut self, mut request: Request) {
        if request.flags & FR_DIALOGTERM != 0 {
            if let Some(dialog) = &mut self.search_dialog {
                if dialog.id == request.id {
                    self.search_text = request.needle;
                    self.search_options.match_case = request.options.match_case;
                    self.search_options.whole_word = request.options.whole_word;
                    if dialog.is_replace() {
                        self.search_replacement = request.replacement;
                    } else {
                        self.search_options.direction = request.options.direction;
                    }
                    dialog.terminated();
                }
            }
            return;
        }
        // The system dialog has no mode control; keep the Search menu choice.
        request.options.mode = self.search_options.mode;
        request.options.wrap = self.search_options.wrap;
        self.search_text = request.needle.clone();
        self.search_options = request.options;
        if request.flags & (FR_REPLACE | FR_REPLACEALL) != 0 {
            self.search_replacement = request.replacement.clone();
        }
        if self.document().is_read_only() && request.flags & (FR_REPLACE | FR_REPLACEALL) != 0 {
            return;
        }
        let replacement = if request.flags & (FR_REPLACE | FR_REPLACEALL) != 0 {
            match search_dialog::native_replacement(request.options.mode, &request.replacement) {
                Ok(text) => text,
                Err(error) => {
                    self.report(error);
                    return;
                }
            }
        } else {
            std::borrow::Cow::Borrowed(request.replacement.as_str())
        };
        if request.flags & FR_REPLACEALL != 0 {
            let scope = self.search_scope();
            let original_length = self.document().text().len();
            let result = if self.search_in_selection || !self.search_options.wrap {
                self.document_mut().replace_matches_in(
                    &request.needle,
                    &request.replacement,
                    request.options,
                    scope.clone(),
                )
            } else {
                self.document_mut().replace_matches(
                    &request.needle,
                    &request.replacement,
                    request.options,
                )
            };
            match result {
                Ok(count) => {
                    if count > 0 && self.search_in_selection {
                        let new_end = scope.start + self.document().text().len()
                            - (original_length - scope.len());
                        let _ = self.document_mut().select(Selection {
                            anchor: scope.start,
                            caret: new_end,
                        });
                        if new_end == scope.start {
                            self.command(SEARCH_IN_SELECTION);
                        }
                    }
                    self.refresh();
                    // SAFETY: Live status control copies the temporary string.
                    unsafe {
                        SetWindowTextW(
                            self.status,
                            wide(&format!("Replaced {count} occurrences")).as_ptr(),
                        );
                    }
                }
                Err(e) => self.report(e),
            }
            return;
        }
        if request.flags & FR_REPLACE != 0 {
            let (native_start, native_end) = control_selection(self.edit);
            let selected = self.sync_native_selection(native_start, native_end).range();
            let start = self
                .document()
                .search_start(request.options.direction, true);
            match self
                .document()
                .search(&request.needle, start, request.options)
            {
                Ok(Some(found)) if found == selected => {
                    let replacement = if request.options.mode == SearchMode::Regex {
                        match npp_editor_core::regex_search::replacements(
                            self.document().text(),
                            &request.needle,
                            &request.replacement,
                            request.options,
                            selected.clone(),
                        ) {
                            Ok(edits) => std::borrow::Cow::Owned(
                                edits
                                    .into_iter()
                                    .find(|(range, _)| range == &selected)
                                    .map(|(_, text)| text)
                                    .unwrap_or_default(),
                            ),
                            Err(error) => {
                                self.report(error);
                                return;
                            }
                        }
                    } else {
                        replacement
                    };
                    if let Err(e) = self
                        .document_mut()
                        .replace_search_target(selected, &replacement)
                    {
                        self.report(e);
                        return;
                    }
                    self.refresh();
                }
                Ok(Some(found)) => {
                    self.select_search_match(found, start, request.options.direction);
                    return;
                }
                Ok(None) => {
                    // SAFETY: The live status control copies this string.
                    unsafe {
                        SetWindowTextW(self.status, wide("No match found").as_ptr());
                    }
                    return;
                }
                Err(e) => {
                    self.report(e);
                    return;
                }
            }
        }
        self.find_using_mode_from(
            request.options.direction,
            request.options.mode,
            request.flags & FR_REPLACE != 0,
        );
    }
    fn rebuild_plugin_menu(&mut self) -> io::Result<()> {
        let accelerators = crate::plugin_accelerators::PluginAccelerators::build(&self.plugins)?;
        // SAFETY: This app owns its live window; the plugin popup stays last.
        self.plugin_menu =
            unsafe { crate::plugin_menu::install(self.window, self.plugin_menu, &self.plugins)? };
        crate::plugin_host::publish_plugin_menu(self.window, self.plugin_menu, &self.plugins);
        self.plugin_accelerators = accelerators;
        Ok(())
    }

    fn search_scope(&mut self) -> std::ops::Range<usize> {
        let (start, end) = control_selection(self.edit);
        let selection = self.sync_native_selection(start, end).range();
        if self.search_in_selection {
            selection
        } else if !self.search_options.wrap {
            match self.search_options.direction {
                SearchDirection::Forward => selection.start..self.document().text().len(),
                SearchDirection::Backward => 0..selection.end,
            }
        } else {
            0..self.document().text().len()
        }
    }

    fn current_preferences(&self) -> crate::preferences::Preferences {
        use crate::scintilla::{ids::*, send};
        crate::preferences::Preferences {
            initial_page: 0,
            show_whitespace: send(self.edit, SCI_GETVIEWWS, 0, 0) != 0,
            show_non_printing: self.show_non_printing.get(),
            show_controls: self.show_controls.get(),
            copy_cut_line: self.copy_cut_line,
            double_click_close: self.double_click_close,
            exit_last_tab: self.exit_last_tab,
            lock_tab_drag: self.lock_tab_drag,
            multiline_tabs: self.multiline_tabs,
            content_tab_names: self.content_tab_names,
            scroll_beyond_last_line: send(self.edit, SCI_GETENDATLASTLINE, 0, 0) == 0,
            compact_tab_length: self.compact_tab_length,
            recent_file_limit: self.workspace.recent_limit(),
            recent_label_length: self.recent_label_length,
            caret_width: self.caret_width,
            current_line_mode: self.current_line_mode,
            current_line_frame: self.current_line_frame,
            fold_symbols: self.fold_symbol_style,
            edge_columns: self.edge_columns.clone(),
            edge_background: self.edge_background,
            short_title: self.short_title,
            save_all_confirm: self.save_all_confirm,
            multi_edit: crate::scintilla::send(
                self.edit,
                crate::scintilla::ids::SCI_GETMULTIPLESELECTION,
                0,
                0,
            ) != 0,
            virtual_space: crate::scintilla::send(
                self.edit,
                crate::scintilla::ids::SCI_GETVIRTUALSPACEOPTIONS,
                0,
                0,
            ) & 2
                != 0,
            column_to_multi: unsafe { !GetPropW(self.edit, w!("ColumnToMulti")).is_null() },
            backspace_unindent: send(self.edit, SCI_GETBACKSPACEUNINDENTS, 0, 0) != 0,
            toggle_fold_commands: unsafe {
                !GetPropW(self.edit, w!("ToggleFoldCommands")).is_null()
            },
            wrap_indent: send(self.edit, SCI_GETWRAPINDENTMODE, 0, 0).clamp(0, 2) as usize,
            smooth_font: send(self.edit, SCI_GETFONTQUALITY, 0, 0) == 3,
            bookmark_margin: send(self.edit, SCI_GETMARGINWIDTHN, 1, 0) != 0,
            distraction_divisor: unsafe {
                (GetPropW(self.window, w!("DistractionDivisor")) as usize).clamp(3, 9)
            },
            padding_left: self
                .distraction_free
                .map_or_else(|| send(self.edit, SCI_GETMARGINLEFT, 0, 0), |state| state.0)
                .max(0) as usize,
            padding_right: self
                .distraction_free
                .map_or_else(
                    || send(self.edit, SCI_GETMARGINRIGHT, 0, 0),
                    |state| state.1,
                )
                .max(0) as usize,
            disable_text_drag: send(self.edit, SCI_GETDRAGDROPENABLED, 0, 0) == 0,
            dynamic_line_numbers: unsafe {
                GetPropW(self.edit, w!("ConstantLineNumberWidth")).is_null()
            },
            right_click_keeps_selection: unsafe {
                !GetPropW(self.edit, w!("KeepSelectionOnRightClick")).is_null()
            },
            caret_blink: send(self.edit, SCI_GETCARETPERIOD, 0, 0).max(0) as usize,
            tab_close_buttons: self.tab_close_buttons,
            inactive_tab_buttons: self.inactive_tab_buttons,
            search_engine: self.search_engine.clone(),
            show_eol: send(self.edit, SCI_GETVIEWEOL, 0, 0) != 0,
            indent_guides: send(self.edit, SCI_GETINDENTATIONGUIDES, 0, 0) != 0,
            wrap_symbols: send(self.edit, SCI_GETWRAPVISUALFLAGS, 0, 0) != 0,
            tab_width: self.tab_width.get(),
            spaces: self.indentation_style == IndentationStyle::Spaces,
            auto_indent: self.basic_auto_indent,
            advanced_indent: self.advanced_indent,
            new_document_eol: self.new_document_eol,
            new_document_encoding: self.new_document_encoding,
            new_document_language: self.new_document_language,
            find_files_prefer_disk: self.find_files_prefer_disk,
            rendering_mode: self.rendering_mode,
            date_first: self.date_first,
            date_format: self.date_format.clone(),
            check_history_files: self.check_history_files,
            recent_in_submenu: self.recent_in_submenu,
            selected_text_foreground: self.selected_text_foreground,
            sql_backslash_escape: self.sql_backslash_escape,
            compact_language_menu: self.compact_language_menu,
            excluded_languages: self.excluded_languages.clone(),
            disable_advanced_scrolling: unsafe {
                !GetPropW(self.edit, w!("DisableAdvancedScrolling")).is_null()
            },
            word_wrap: self.word_wrap,
            zoom: self.zoom,
            toolbar: self.post_it.map_or_else(
                || !is_control_hidden(self.window, crate::editor_chrome::TOOLBAR as usize),
                |state| state.1,
            ),
            line_numbers: !is_control_hidden(self.window, crate::editor_chrome::GUTTER as usize),
            status_bar: self
                .post_it
                .map_or_else(|| !is_status_hidden(self.window), |state| state.3),
            tab_bar: self
                .post_it
                .map_or_else(|| !is_control_hidden(self.window, TAB_ID), |state| state.2),
        }
    }
    fn apply_preferences(&mut self, value: &crate::preferences::Preferences) -> io::Result<()> {
        self.apply_symbols(value);
        self.copy_cut_line = value.copy_cut_line;
        self.double_click_close = value.double_click_close;
        self.exit_last_tab = value.exit_last_tab;
        self.lock_tab_drag = value.lock_tab_drag;
        self.multiline_tabs = value.multiline_tabs;
        self.content_tab_names = value.content_tab_names;
        self.caret_width = value.caret_width.min(5);
        self.current_line_mode = value.current_line_mode.min(2);
        self.current_line_frame = value.current_line_frame.clamp(1, 6);
        self.fold_symbol_style = value.fold_symbols.min(4);
        self.edge_columns = value
            .edge_columns
            .iter()
            .copied()
            .filter(|&column| column <= 8192)
            .collect();
        self.edge_columns.sort_unstable();
        self.edge_columns.dedup();
        self.edge_background = value.edge_background;
        self.short_title = value.short_title;
        self.save_all_confirm = value.save_all_confirm;
        unsafe {
            SetPropW(
                self.window,
                w!("DistractionDivisor"),
                value.distraction_divisor.clamp(3, 9) as HANDLE,
            );
        }
        if let Some((left, right, _)) = self.distraction_free.as_mut() {
            *left = value.padding_left.min(9) as isize;
            *right = value.padding_right.min(9) as isize;
        }
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETMULTIPLESELECTION,
            value.multi_edit as usize,
            0,
        );
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETVIRTUALSPACEOPTIONS,
            if value.virtual_space { 7 } else { 1 },
            0,
        );
        unsafe {
            if value.multi_edit && value.column_to_multi {
                SetPropW(self.edit, w!("ColumnToMulti"), 1usize as HANDLE);
            } else {
                RemovePropW(self.edit, w!("ColumnToMulti"));
            }
        }
        unsafe {
            if value.toggle_fold_commands {
                SetPropW(self.edit, w!("ToggleFoldCommands"), 1usize as HANDLE);
            } else {
                RemovePropW(self.edit, w!("ToggleFoldCommands"));
            }
        }
        {
            use crate::scintilla::{ids::*, send};
            send(self.edit, SCI_MULTIEDGECLEARALL, 0, 0);
            for &column in &self.edge_columns {
                send(self.edit, SCI_MULTIEDGEADDLINE, column, 0xc0c0c0);
            }
            send(self.edit, SCI_SETEDGECOLOUR, 0xc0c0c0, 0);
            let mode = if self.edge_columns.is_empty() {
                0
            } else if self.edge_columns.len() == 1 && self.edge_background {
                send(self.edit, SCI_SETEDGECOLUMN, self.edge_columns[0], 0);
                2 // EDGE_BACKGROUND
            } else {
                3
            }; // EDGE_MULTILINE
            send(self.edit, SCI_SETEDGEMODE, mode, 0);
        }

        crate::scintilla::fold_symbols(self.edit, self.fold_symbol_style);
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETMARGINLEFT,
            0,
            value.padding_left.min(9) as isize,
        );
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETMARGINRIGHT,
            0,
            value.padding_right.min(9) as isize,
        );
        crate::scintilla::update_symbol_margin(self.edit, value.bookmark_margin);
        unsafe {
            if value.dynamic_line_numbers {
                RemovePropW(self.edit, w!("ConstantLineNumberWidth"));
            } else {
                SetPropW(self.edit, w!("ConstantLineNumberWidth"), 1usize as HANDLE);
            }
        }
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETDRAGDROPENABLED,
            (!value.disable_text_drag) as usize,
            0,
        );
        unsafe {
            if value.right_click_keeps_selection {
                SetPropW(self.edit, w!("KeepSelectionOnRightClick"), 1usize as HANDLE);
            } else {
                RemovePropW(self.edit, w!("KeepSelectionOnRightClick"));
            }
        }

        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETFONTQUALITY,
            if value.smooth_font { 3 } else { 0 },
            0,
        );
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETWRAPINDENTMODE,
            value.wrap_indent.min(2),
            0,
        );
        {
            use crate::scintilla::{ids::*, send};
            send(
                self.edit,
                SCI_SETCARETLINEVISIBLE,
                (self.current_line_mode != 0) as usize,
                0,
            );
            send(
                self.edit,
                SCI_SETCARETLINEFRAME,
                if self.current_line_mode == 2 {
                    self.current_line_frame
                } else {
                    0
                },
                0,
            );
        }
        {
            use crate::scintilla::{ids::*, send};
            // Scintilla CARETSTYLE_LINE=1, BLOCK=2, BLOCK_AFTER=0x100.
            let style = match self.caret_width {
                4 => 2,
                5 => 2 | 0x100,
                _ => 1,
            };
            send(self.edit, SCI_SETCARETSTYLE, style, 0);
            send(
                self.edit,
                SCI_SETCARETWIDTH,
                if self.caret_width < 4 {
                    self.caret_width
                } else {
                    1
                },
                0,
            );
            send(
                self.edit,
                SCI_SETCARETPERIOD,
                value.caret_blink.min(2000),
                0,
            );
        }

        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETENDATLASTLINE,
            (!value.scroll_beyond_last_line) as usize,
            0,
        );
        self.compact_tab_length = value.compact_tab_length.min(257);
        self.workspace.set_recent_limit(value.recent_file_limit);
        self.recent_label_length = value.recent_label_length.clamp(-1, 259);
        self.tab_close_buttons = value.tab_close_buttons;
        self.inactive_tab_buttons = value.inactive_tab_buttons;
        crate::tabs::close_buttons(
            self.tabs,
            value.tab_close_buttons,
            value.inactive_tab_buttons,
        );
        unsafe {
            let style = GetWindowLongPtrW(self.tabs, GWL_STYLE) as u32;
            let style = if value.multiline_tabs {
                style | TCS_MULTILINE
            } else {
                style & !TCS_MULTILINE
            };
            SetWindowLongPtrW(self.tabs, GWL_STYLE, style as isize);
            SetWindowPos(
                self.tabs,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
        unsafe {
            crate::tabs::lock_drag(GetDlgItem(self.window, TAB_ID as i32), value.lock_tab_drag);
        }
        self.search_engine = value.search_engine.clone();
        self.set_tab_width(value.tab_width.clamp(1, 32))?;
        self.indentation_style = if value.spaces {
            IndentationStyle::Spaces
        } else {
            IndentationStyle::Tabs
        };
        self.basic_auto_indent = value.auto_indent;
        self.advanced_indent = value.advanced_indent;
        self.find_files_prefer_disk = value.find_files_prefer_disk;
        self.rendering_mode = value.rendering_mode.min(4);
        self.date_first = value.date_first;
        self.date_format = value.date_format.clone();
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                DATE_FIRST,
                MF_BYCOMMAND
                    | if self.date_first {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        self.check_history_files = value.check_history_files;
        self.recent_in_submenu = value.recent_in_submenu;
        self.selected_text_foreground = value.selected_text_foreground;
        self.sql_backslash_escape = value.sql_backslash_escape;
        if self.compact_language_menu != value.compact_language_menu
            || self.excluded_languages != value.excluded_languages
        {
            self.excluded_languages = value.excluded_languages.clone();
            self.compact_language_menu = value.compact_language_menu;
            self.rebuild_language_layout();
        }
        if self.language().command == IDM_LANG_SQL {
            crate::scintilla::sql_backslash_escape(self.edit, self.sql_backslash_escape);
        }
        unsafe {
            if value.disable_advanced_scrolling {
                SetPropW(self.edit, w!("DisableAdvancedScrolling"), 1usize as HANDLE);
            } else {
                RemovePropW(self.edit, w!("DisableAdvancedScrolling"));
            }
        }
        crate::scintilla::selection_foreground(self.edit, value.selected_text_foreground);
        self.new_document_eol = value.new_document_eol.min(2);
        self.new_document_language = crate::languages::by_command(value.new_document_language)
            .map_or(IDM_LANG_TEXT, |language| language.command);
        self.new_document_encoding = value
            .new_document_encoding
            .min(crate::preferences::NEW_ENCODINGS.len() - 1);
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETBACKSPACEUNINDENTS,
            value.backspace_unindent as usize,
            0,
        );
        // Special views temporarily hide chrome; preferences describe the normal view.
        // Retain edits made while hidden for restoration when leaving the mode.
        if let Some((_, toolbar, tabs, status, _, _)) = self.post_it.as_mut() {
            *toolbar = value.toolbar;
            *tabs = value.tab_bar;
            *status = value.status_bar;
        }
        self.set_zoom(value.zoom)?;
        if value.word_wrap != self.word_wrap {
            self.toggle_word_wrap()?;
        }
        unsafe {
            let menu = crate::plugin_host::main_menu(self.window);
            for (id, command, show) in [
                (crate::editor_chrome::TOOLBAR, SHOW_TOOLBAR, value.toolbar),
                (
                    crate::editor_chrome::GUTTER,
                    SHOW_LINE_NUMBERS,
                    value.line_numbers,
                ),
                (STATUS_ID as i32, SHOW_STATUS_BAR, value.status_bar),
                (TAB_ID as i32, SHOW_TAB_BAR, value.tab_bar),
            ] {
                let show = show && (self.post_it.is_none() || id == crate::editor_chrome::GUTTER);
                ShowWindow(
                    GetDlgItem(self.window, id),
                    if show { SW_SHOWNA } else { SW_HIDE },
                );
                CheckMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if show { MF_CHECKED } else { MF_UNCHECKED },
                );
            }
            for (command, checked) in [
                (INDENT_WITH_SPACES, value.spaces),
                (BASIC_AUTO_INDENT, value.auto_indent),
            ] {
                CheckMenuItem(
                    menu,
                    command,
                    MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
                );
            }
        }
        self.refresh_recent_menu();
        self.refresh_tabs();
        self.resize();
        self.caption();
        Ok(())
    }
    fn toggle_word_wrap(&mut self) -> io::Result<()> {
        self.word_wrap = !self.word_wrap;
        crate::scintilla::send(
            self.edit,
            crate::scintilla::ids::SCI_SETWRAPMODE,
            self.word_wrap as usize,
            0,
        );
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                IDM_VIEW_WRAP,
                MF_BYCOMMAND
                    | if self.word_wrap {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        Ok(())
    }
    fn apply_symbols(&self, value: &crate::preferences::Preferences) {
        use crate::scintilla::{ids::*, send};
        self.show_non_printing.set(value.show_non_printing);
        self.show_controls.set(value.show_controls);
        let all = value.show_whitespace
            && value.show_eol
            && value.show_non_printing
            && value.show_controls;
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                IDM_VIEW_ALL_CHARACTERS,
                MF_BYCOMMAND | if all { MF_CHECKED } else { MF_UNCHECKED },
            );
        }
        crate::scintilla::control_characters(
            self.edit,
            value.show_controls,
            self.language().command,
        );
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                IDM_VIEW_NPC_CCUNIEOL,
                MF_BYCOMMAND
                    | if value.show_controls {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        crate::scintilla::non_printing(self.edit, value.show_non_printing);
        unsafe {
            CheckMenuItem(
                crate::plugin_host::main_menu(self.window),
                IDM_VIEW_NPC,
                MF_BYCOMMAND
                    | if value.show_non_printing {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
            );
        }
        for (command, message, show, mode) in [
            (
                IDM_VIEW_TAB_SPACE,
                SCI_SETVIEWWS,
                value.show_whitespace,
                SCWS_VISIBLEALWAYS,
            ),
            (IDM_VIEW_EOL, SCI_SETVIEWEOL, value.show_eol, 1),
            (
                IDM_VIEW_INDENT_GUIDE,
                SCI_SETINDENTATIONGUIDES,
                value.indent_guides,
                SC_IV_LOOKBOTH,
            ),
            (
                IDM_VIEW_WRAP_SYMBOL,
                SCI_SETWRAPVISUALFLAGS,
                value.wrap_symbols,
                SC_WRAPVISUALFLAG_END,
            ),
        ] {
            send(self.edit, message, if show { mode as usize } else { 0 }, 0);
            unsafe {
                CheckMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    command,
                    MF_BYCOMMAND | if show { MF_CHECKED } else { MF_UNCHECKED },
                );
            }
        }
    }
    fn hash_command(&mut self, command: u32) -> io::Result<()> {
        use sha2::Digest;
        let from_file = [
            IDM_TOOL_MD5_GENERATEFROMFILE,
            IDM_TOOL_SHA1_GENERATEFROMFILE,
            IDM_TOOL_SHA256_GENERATEFROMFILE,
            IDM_TOOL_SHA512_GENERATEFROMFILE,
        ]
        .contains(&command);
        let to_clipboard = [
            IDM_TOOL_MD5_GENERATEINTOCLIPBOARD,
            IDM_TOOL_SHA1_GENERATEINTOCLIPBOARD,
            IDM_TOOL_SHA256_GENERATEINTOCLIPBOARD,
            IDM_TOOL_SHA512_GENERATEINTOCLIPBOARD,
        ]
        .contains(&command);
        if from_file {
            let Some(paths) = file_dialog_paths(
                self.window,
                false,
                Some("Generate hashes from files"),
                false,
                true,
            )?
            else {
                return Ok(());
            };
            let mut results = String::new();
            for path in paths {
                let result = std::fs::read(&path).map(|bytes| match command {
                    IDM_TOOL_MD5_GENERATEFROMFILE => format!("{:x}", md5::Md5::digest(&bytes)),
                    IDM_TOOL_SHA1_GENERATEFROMFILE => format!("{:x}", sha1::Sha1::digest(&bytes)),
                    IDM_TOOL_SHA256_GENERATEFROMFILE => {
                        format!("{:x}", sha2::Sha256::digest(&bytes))
                    }
                    _ => format!("{:x}", sha2::Sha512::digest(&bytes)),
                });
                match result {
                    Ok(hash) => results.push_str(&format!("{hash}  {}\r\n", path.display())),
                    Err(error) => {
                        results.push_str(&format!("ERROR  {}: {error}\r\n", path.display()))
                    }
                }
            }
            return crate::input_dialog::results(
                self.window,
                "File Hash Results",
                "Hash and file path (select text to copy). Failed files are listed individually:",
                &results,
            );
        }
        let bytes = if to_clipboard {
            let (start, end) = control_selection(self.edit);
            let range = self.sync_native_selection(start, end).range();
            self.document().text()[range].as_bytes().to_vec()
        } else {
            let Some(value) = crate::input_dialog::ask(
                self.window,
                "Generate Hash",
                "Enter the text to hash (UTF-8):",
                "",
            )?
            else {
                return Ok(());
            };
            value.into_bytes()
        };
        let hash = match command {
            IDM_TOOL_MD5_GENERATE
            | IDM_TOOL_MD5_GENERATEFROMFILE
            | IDM_TOOL_MD5_GENERATEINTOCLIPBOARD => format!("{:x}", md5::Md5::digest(&bytes)),
            IDM_TOOL_SHA1_GENERATE
            | IDM_TOOL_SHA1_GENERATEFROMFILE
            | IDM_TOOL_SHA1_GENERATEINTOCLIPBOARD => format!("{:x}", sha1::Sha1::digest(&bytes)),
            IDM_TOOL_SHA256_GENERATE
            | IDM_TOOL_SHA256_GENERATEFROMFILE
            | IDM_TOOL_SHA256_GENERATEINTOCLIPBOARD => {
                format!("{:x}", sha2::Sha256::digest(&bytes))
            }
            _ => format!("{:x}", sha2::Sha512::digest(&bytes)),
        };
        if to_clipboard {
            crate::clipboard::write_text(self.window, &hash)?;
        } else {
            // An editable result makes copying a long digest straightforward.
            let _ = crate::input_dialog::ask(
                self.window,
                "Hash Result",
                "Hash of the supplied bytes:",
                &hash,
            )?;
        }
        Ok(())
    }
    fn command(&mut self, command: u32) {
        if crate::languages::by_command(command).is_some()
            || self.user_languages.iter().any(|l| l.command == command)
        {
            self.default_languages.remove(&self.active_id());
            self.languages.insert(self.active_id(), command);
            self.apply_language();
            self.caption();
            return;
        }
        if self.document().is_read_only() && EDIT_COMMANDS.contains(&command) {
            return;
        }
        {
            use crate::scintilla::{ids::*, send};
            let message = match command {
                IDM_EDIT_SPLIT_LINES => SCI_LINESSPLIT,
                IDM_EDIT_DELETE => SCI_CLEAR,
                DELETE_WORD_LEFT => SCI_DELWORDLEFT,
                DELETE_WORD_RIGHT => SCI_DELWORDRIGHT,
                DELETE_LINE_LEFT => SCI_DELLINELEFT,
                DELETE_LINE_RIGHT => SCI_DELLINERIGHT,
                _ => 0,
            };
            let split = command == IDM_EDIT_SPLIT_LINES;
            let selections = send(self.edit, SCI_GETSELECTIONS, 0, 0);
            if split && selections != 1 {
                return;
            }
            if message != 0
                && (split || selections > 1 || send(self.edit, SCI_SELECTIONISRECTANGLE, 0, 0) != 0)
            {
                let before = self
                    .macro_recording
                    .then(|| self.document().text().to_owned());
                let caret = self.document().selection().caret;
                UPDATING.set(true);
                let mut width = 0;
                if split {
                    let start = send(self.edit, SCI_GETSELECTIONSTART, 0, 0) as usize;
                    let end = send(self.edit, SCI_GETSELECTIONEND, 0, 0) as usize;
                    let first = send(self.edit, SCI_LINEFROMPOSITION, start, 0) as usize;
                    let mut last = send(self.edit, SCI_LINEFROMPOSITION, end, 0) as usize;
                    if last > first
                        && send(self.edit, SCI_POSITIONFROMLINE, last, 0) as usize == end
                    {
                        last -= 1;
                    }
                    let anchor = send(self.edit, SCI_POSITIONFROMLINE, first, 0);
                    let caret = send(self.edit, SCI_GETLINEENDPOSITION, last, 0);
                    send(self.edit, SCI_SETSELECTION, caret as usize, anchor);
                    send(self.edit, SCI_TARGETFROMSELECTION, 0, 0);
                    if send(self.edit, SCI_GETEDGEMODE, 0, 0) != 0 {
                        let column = self.edge_columns.last().copied().unwrap_or_else(|| {
                            send(self.edit, SCI_GETEDGECOLUMN, 0, 0).max(0) as usize
                        });
                        let character_width = send(
                            self.edit,
                            SCI_TEXTWIDTH,
                            STYLE_DEFAULT as usize,
                            c"P".as_ptr() as isize,
                        )
                        .max(1) as usize;
                        width = character_width.saturating_mul(column.saturating_add(1));
                    }
                }
                send(self.edit, message, width, 0);
                UPDATING.set(false);
                let text = crate::scintilla::text(self.edit);
                let id = self.active_id();
                if let Err(error) = self
                    .projection
                    .apply_exact(self.workspace.get_mut(id).unwrap(), &text)
                {
                    self.report(error);
                    self.refresh();
                    return;
                }
                self.projection = Projection::new(self.document().text());
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                if let Some(before) = before {
                    if let Some(step) =
                        crate::macros::difference(&before, self.document().text(), caret)
                    {
                        self.macro_steps.push(step);
                    }
                }
                self.caption();
                self.refresh_tabs();
                return;
            }
        }
        // Accelerators arrive before the editor's key handler. Preserve native
        // multiple/rectangular ranges and virtual columns for navigation.
        {
            use crate::scintilla::{ids::*, send};
            let native_navigation = send(self.edit, SCI_GETSELECTIONS, 0, 0) > 1
                || send(self.edit, SCI_SELECTIONISRECTANGLE, 0, 0) != 0
                || send(self.edit, SCI_GETVIRTUALSPACEOPTIONS, 0, 0) & 2 != 0;
            let rtl = self.rtl_buffers.contains(&self.active_id());
            let message = match command {
                SMART_HOME => SCI_VCHOME,
                SMART_HOME_EXTEND => SCI_VCHOMEEXTEND,
                LINE_END => SCI_LINEEND,
                LINE_END_EXTEND => SCI_LINEENDEXTEND,
                DOCUMENT_START => SCI_DOCUMENTSTART,
                DOCUMENT_START_EXTEND => SCI_DOCUMENTSTARTEXTEND,
                DOCUMENT_END => SCI_DOCUMENTEND,
                DOCUMENT_END_EXTEND => SCI_DOCUMENTENDEXTEND,
                WORD_LEFT => {
                    if rtl {
                        SCI_WORDRIGHT
                    } else {
                        SCI_WORDLEFT
                    }
                }
                WORD_RIGHT => {
                    if rtl {
                        SCI_WORDLEFT
                    } else {
                        SCI_WORDRIGHT
                    }
                }
                WORD_LEFT_EXTEND => {
                    if rtl {
                        SCI_WORDRIGHTEXTEND
                    } else {
                        SCI_WORDLEFTEXTEND
                    }
                }
                WORD_RIGHT_EXTEND => {
                    if rtl {
                        SCI_WORDLEFTEXTEND
                    } else {
                        SCI_WORDRIGHTEXTEND
                    }
                }
                WORD_PART_LEFT => SCI_WORDPARTLEFT,
                WORD_PART_RIGHT => SCI_WORDPARTRIGHT,
                WORD_PART_LEFT_EXTEND => SCI_WORDPARTLEFTEXTEND,
                WORD_PART_RIGHT_EXTEND => SCI_WORDPARTRIGHTEXTEND,
                PARAGRAPH_UP => SCI_PARAUP,
                PARAGRAPH_DOWN => SCI_PARADOWN,
                PARAGRAPH_UP_EXTEND => SCI_PARAUPEXTEND,
                PARAGRAPH_DOWN_EXTEND => SCI_PARADOWNEXTEND,
                _ => 0,
            };
            if native_navigation && message != 0 {
                let mode = send(self.edit, SCI_GETSELECTIONMODE, 0, 0) as u32;
                if matches!(mode, SC_SEL_RECTANGLE | SC_SEL_THIN)
                    && unsafe { !GetPropW(self.edit, w!("ColumnToMulti")).is_null() }
                {
                    // Upstream repeats the mode change to discard the extra
                    // rectangular range while retaining its constituent carets.
                    send(self.edit, SCI_SETSELECTIONMODE, SC_SEL_STREAM as usize, 0);
                    send(self.edit, SCI_SETSELECTIONMODE, SC_SEL_STREAM as usize, 0);
                }
                send(self.edit, message, 0, 0);
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                self.caption();
                return;
            }
        }
        if self.macro_recording
            && EDIT_COMMANDS.contains(&command)
            && ![
                IDM_EDIT_CUT,
                IDM_EDIT_CUT_BINARY,
                IDM_EDIT_PASTE_BINARY,
                IDM_EDIT_PASTE,
                IDM_EDIT_PASTE_AS_HTML,
                IDM_EDIT_PASTE_AS_RTF,
            ]
            .contains(&command)
        {
            self.macro_steps.push(crate::macros::Step::Command(command));
        }
        match command {
            IDM_VIEW_TAB_SPACE
            | IDM_VIEW_EOL
            | IDM_VIEW_INDENT_GUIDE
            | IDM_VIEW_WRAP_SYMBOL
            | IDM_VIEW_NPC
            | IDM_VIEW_NPC_CCUNIEOL
            | IDM_VIEW_ALL_CHARACTERS => {
                let mut value = self.current_preferences();
                match command {
                    IDM_VIEW_ALL_CHARACTERS => {
                        let show = !(value.show_whitespace
                            && value.show_eol
                            && value.show_non_printing
                            && value.show_controls);
                        value.show_whitespace = show;
                        value.show_eol = show;
                        value.show_non_printing = show;
                        value.show_controls = show;
                    }
                    IDM_VIEW_NPC => value.show_non_printing = !value.show_non_printing,
                    IDM_VIEW_NPC_CCUNIEOL => value.show_controls = !value.show_controls,
                    IDM_VIEW_TAB_SPACE => value.show_whitespace = !value.show_whitespace,
                    IDM_VIEW_EOL => value.show_eol = !value.show_eol,
                    IDM_VIEW_INDENT_GUIDE => value.indent_guides = !value.indent_guides,
                    _ => value.wrap_symbols = !value.wrap_symbols,
                }
                self.apply_symbols(&value);
                if let Err(error) = crate::preferences::save(&value) {
                    self.report(error);
                }
            }
            FORMAT_DOCUMENT | FORMAT_SELECTION => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                if command == FORMAT_SELECTION && selection.range().is_empty() {
                    return;
                }
                let source = self.document().text();
                let language = self.language().command;
                let mut range = if command == FORMAT_DOCUMENT {
                    0..source.len()
                } else {
                    selection.range()
                };
                if !matches!(language, IDM_LANG_JSON | IDM_LANG_XML) {
                    let starts = self.document().line_starts();
                    range.start = starts[starts
                        .partition_point(|&start| start <= range.start)
                        .saturating_sub(1)];
                    range.end = starts
                        .get(starts.partition_point(|&start| start < range.end))
                        .copied()
                        .unwrap_or(source.len());
                }
                let width = self.tab_width.get();
                let style = self.indentation_style;
                let ending = self.document().line_ending().as_str();
                let formatted = match language {
                    IDM_LANG_JSON => {
                        crate::formatting::json(&source[range.clone()], width, style, ending)
                    }
                    IDM_LANG_XML => {
                        crate::formatting::xml(&source[range.clone()], width, style, ending)
                    }
                    _ => Ok(crate::formatting::reindent(
                        self.edit,
                        source,
                        language,
                        range.clone(),
                        width,
                        style,
                    )),
                };
                match formatted {
                    Ok(text) => {
                        if let Err(error) = self.document_mut().replace(range, &text) {
                            self.report(error);
                        }
                        self.refresh();
                    }
                    Err(error) => self.report(error),
                }
            }
            IDM_VIEW_FOLDALL
            | IDM_VIEW_UNFOLDALL
            | IDM_VIEW_FOLD_CURRENT
            | IDM_VIEW_UNFOLD_CURRENT => {
                crate::scintilla::fold(
                    self.edit,
                    matches!(command, IDM_VIEW_UNFOLDALL | IDM_VIEW_UNFOLD_CURRENT),
                    None,
                    matches!(command, IDM_VIEW_FOLD_CURRENT | IDM_VIEW_UNFOLD_CURRENT),
                );
            }
            IDM_VIEW_FOLD_1..=IDM_VIEW_FOLD_8 => {
                crate::scintilla::fold(
                    self.edit,
                    false,
                    Some((command - IDM_VIEW_FOLD_1 + 1) as usize),
                    false,
                );
            }
            IDM_VIEW_UNFOLD_1..=IDM_VIEW_UNFOLD_8 => {
                crate::scintilla::fold(
                    self.edit,
                    true,
                    Some((command - IDM_VIEW_UNFOLD_1 + 1) as usize),
                    false,
                );
            }
            IDM_EDIT_EOL2WS | IDM_EDIT_TRIMALL => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let range = if selection.range().is_empty() {
                    0..self.document().text().len()
                } else {
                    selection.range()
                };
                let text = &self.document().text()[range.clone()];
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                let replacement = if command == IDM_EDIT_TRIMALL {
                    normalized
                        .split('\n')
                        .map(|line| line.trim_matches([' ', '\t']))
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    normalized.replace('\n', " ")
                };
                if let Err(error) = self.document_mut().replace(range, &replacement) {
                    self.report(error);
                }
                self.refresh();
            }
            IDM_LANG_USER_DLG | IDM_LANG_USER => {
                if command == IDM_LANG_USER && !self.user_languages.is_empty() {
                    self.languages
                        .insert(self.active_id(), self.user_languages[0].command);
                    self.apply_language();
                    self.caption();
                    return;
                }
                let result = (|| -> io::Result<()> {
                    let existing = self
                        .user_languages
                        .iter()
                        .find(|l| l.command == self.language().command);
                    let Some(mut language) = crate::languages::define(self.window, existing)?
                    else {
                        return Ok(());
                    };
                    let mut next = self.user_languages.clone();
                    if let Some(old) = next.iter_mut().find(|l| l.name == language.name) {
                        language.command = old.command;
                        *old = language.clone();
                    } else {
                        if next.len() >= crate::languages::USER_LANGUAGE_LIMIT {
                            return Err(io::Error::other(
                                "At most 500 user languages can be defined.",
                            ));
                        }
                        language.command =
                            crate::languages::USER_LANGUAGE_FIRST + next.len() as u32;
                        next.push(language.clone());
                    }
                    crate::languages::save_user(&next)?;
                    unsafe {
                        let menu = GetSubMenu(crate::plugin_host::main_menu(self.window), 5);
                        for old in &self.user_languages {
                            DeleteMenu(menu, old.command, MF_BYCOMMAND);
                        }
                        for item in &next {
                            crate::languages::append_language(menu, item);
                        }
                    }
                    self.user_languages = next;
                    self.default_languages.remove(&self.active_id());
                    self.languages.insert(self.active_id(), language.command);
                    self.applied_language = None;
                    self.apply_language();
                    self.caption();
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_LANG_OPENUDLDIR => {
                let directory = crate::languages::user_directory();
                let result = std::fs::create_dir_all(&directory).and_then(|_| {
                    let result = unsafe {
                        windows_sys::Win32::UI::Shell::ShellExecuteW(
                            self.window,
                            w!("open"),
                            wide(&directory.to_string_lossy()).as_ptr(),
                            null(),
                            null(),
                            SW_SHOWNORMAL,
                        )
                    };
                    if result as isize <= 32 {
                        Err(io::Error::other("Could not open the user language folder"))
                    } else {
                        Ok(())
                    }
                });
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_VIEW_DISTRACTIONFREE => {
                use crate::scintilla::{ids::*, send};
                if let Some((left, right, results)) = self.distraction_free.take() {
                    unsafe {
                        RemovePropW(self.window, w!("DistractionFree"));
                    }
                    self.command(IDM_VIEW_FULLSCREENTOGGLE);
                    self.command(IDM_VIEW_POSTIT);
                    send(self.edit, SCI_SETMARGINLEFT, 0, left);
                    send(self.edit, SCI_SETMARGINRIGHT, 0, right);
                    if let Some(search) = &self.find_files {
                        search.show_results(results);
                    }
                } else {
                    // Upstream only enters this combined mode from normal view.
                    if self.fullscreen.is_some() || self.post_it.is_some() {
                        return;
                    }
                    let left = send(self.edit, SCI_GETMARGINLEFT, 0, 0);
                    let right = send(self.edit, SCI_GETMARGINRIGHT, 0, 0);
                    let results =
                        !is_control_hidden(self.window, crate::find_files::RESULTS as usize);
                    self.command(IDM_VIEW_FULLSCREENTOGGLE);
                    if self.fullscreen.is_none() {
                        return;
                    }
                    self.command(IDM_VIEW_POSTIT);
                    if self.post_it.is_none() {
                        self.command(IDM_VIEW_FULLSCREENTOGGLE);
                        return;
                    }
                    if let Some(search) = &self.find_files {
                        search.show_results(false);
                    }
                    self.distraction_free = Some((left, right, results));
                    unsafe {
                        SetPropW(self.window, w!("DistractionFree"), 1usize as HANDLE);
                    }
                }
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        IDM_VIEW_DISTRACTIONFREE,
                        MF_BYCOMMAND
                            | if self.distraction_free.is_some() {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
                self.resize();
            }
            IDM_VIEW_POSTIT => unsafe {
                if self.distraction_free.is_some() {
                    self.command(IDM_VIEW_DISTRACTIONFREE);
                    return;
                }
                let mut restore = GetDlgItem(self.window, POST_IT_RESTORE);
                if restore.is_null() {
                    restore = CreateWindowExW(
                        0,
                        w!("BUTTON"),
                        w!("Restore (F12)"),
                        WS_CHILD | WS_TABSTOP | BS_PUSHBUTTON as u32,
                        0,
                        0,
                        110,
                        26,
                        self.window,
                        POST_IT_RESTORE as HMENU,
                        GetModuleHandleW(null()),
                        null(),
                    );
                    if restore.is_null() {
                        self.report(io::Error::last_os_error());
                        return;
                    }
                    SendMessageW(
                        restore,
                        WM_SETFONT,
                        GetStockObject(DEFAULT_GUI_FONT) as usize,
                        1,
                    );
                }
                let restoring = self.post_it.take();
                let entering = restoring.is_none();
                let (style, toolbar, tabs, status, menu, top) = restoring.unwrap_or_else(|| {
                    (
                        self.fullscreen
                            .as_ref()
                            .map_or(GetWindowLongPtrW(self.window, GWL_STYLE), |(style, _)| {
                                *style
                            }),
                        !is_control_hidden(self.window, crate::editor_chrome::TOOLBAR as usize),
                        !is_control_hidden(self.window, TAB_ID),
                        !is_status_hidden(self.window),
                        !GetMenu(self.window).is_null(),
                        GetWindowLongPtrW(self.window, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0,
                    )
                });
                if entering {
                    self.post_it = Some((style, toolbar, tabs, status, menu, top));
                }
                ShowWindow(restore, if entering { SW_SHOWNA } else { SW_HIDE });
                if self.fullscreen.is_none() {
                    SetWindowLongPtrW(
                        self.window,
                        GWL_STYLE,
                        if entering {
                            style & !(WS_OVERLAPPEDWINDOW as isize)
                        } else {
                            style
                        },
                    );
                } else if !entering {
                    if let Some((saved, _)) = self.fullscreen.as_mut() {
                        *saved = style;
                    }
                }
                ShowWindow(
                    GetDlgItem(self.window, crate::editor_chrome::TOOLBAR),
                    if !entering && toolbar {
                        SW_SHOWNA
                    } else {
                        SW_HIDE
                    },
                );
                SendMessageW(
                    self.window,
                    npp_compat::messages::NPPM_HIDETABBAR,
                    0,
                    (entering || !tabs) as isize,
                );
                SendMessageW(
                    self.window,
                    npp_compat::messages::NPPM_HIDESTATUSBAR,
                    0,
                    (entering || !status) as isize,
                );
                SendMessageW(
                    self.window,
                    npp_compat::messages::NPPM_HIDEMENU,
                    0,
                    (entering || !menu) as isize,
                );
                SetWindowPos(
                    self.window,
                    if entering || top {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
                );
                CheckMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    IDM_VIEW_POSTIT,
                    MF_BYCOMMAND | if entering { MF_CHECKED } else { MF_UNCHECKED },
                );
                CheckMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    IDM_VIEW_ALWAYSONTOP,
                    MF_BYCOMMAND
                        | if entering || top {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                );
                self.resize();
                SetFocus(self.edit);
            },
            IDM_VIEW_ALWAYSONTOP => unsafe {
                let top = GetWindowLongPtrW(self.window, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST == 0;
                SetWindowPos(
                    self.window,
                    if top { HWND_TOPMOST } else { HWND_NOTOPMOST },
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
                CheckMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    command,
                    MF_BYCOMMAND | if top { MF_CHECKED } else { MF_UNCHECKED },
                );
            },
            IDM_VIEW_FULLSCREENTOGGLE => unsafe {
                if self.distraction_free.is_some() {
                    self.command(IDM_VIEW_DISTRACTIONFREE);
                    return;
                }
                if let Some((style, bounds)) = self.fullscreen.take() {
                    let style = if self.post_it.is_some() {
                        style & !(WS_OVERLAPPEDWINDOW as isize)
                    } else {
                        style
                    };
                    SetWindowLongPtrW(self.window, GWL_STYLE, style);
                    SetWindowPos(
                        self.window,
                        std::ptr::null_mut(),
                        bounds.left,
                        bounds.top,
                        bounds.right - bounds.left,
                        bounds.bottom - bounds.top,
                        SWP_NOZORDER | SWP_FRAMECHANGED,
                    );
                } else {
                    let mut bounds = RECT::default();
                    GetWindowRect(self.window, &mut bounds);
                    let style = GetWindowLongPtrW(self.window, GWL_STYLE);
                    let mut monitor = MONITORINFO {
                        cbSize: size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    if GetMonitorInfoW(
                        MonitorFromWindow(self.window, MONITOR_DEFAULTTONEAREST),
                        &mut monitor,
                    ) != 0
                    {
                        self.fullscreen = Some((style, bounds));
                        SetWindowLongPtrW(
                            self.window,
                            GWL_STYLE,
                            style & !(WS_OVERLAPPEDWINDOW as isize),
                        );
                        let rect = monitor.rcMonitor;
                        SetWindowPos(
                            self.window,
                            std::ptr::null_mut(),
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_FRAMECHANGED,
                        );
                    }
                }
                CheckMenuItem(
                    crate::plugin_host::main_menu(self.window),
                    command,
                    MF_BYCOMMAND
                        | if self.fullscreen.is_some() {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                );
            },
            IDM_VIEW_TAB1..=IDM_VIEW_TAB9 => {
                let id = self
                    .workspace
                    .buffers()
                    .nth((command - IDM_VIEW_TAB1) as usize)
                    .map(|(id, _)| id);
                if let Some(id) = id {
                    self.workspace.activate(id);
                    self.refresh();
                }
            }
            IDM_VIEW_TAB_START | IDM_VIEW_TAB_END => {
                while self.workspace.move_active(command == IDM_VIEW_TAB_END) {}
                self.refresh_tabs();
            }
            IDM_EDIT_DELETE | IDM_EDIT_REDACT_SELECTION => {
                let (start, end) = control_selection(self.edit);
                let mut range = self.sync_native_selection(start, end).range();
                if command == IDM_EDIT_DELETE && range.is_empty() {
                    if self.document().text()[range.start..].starts_with("\r\n") {
                        range.end += 2;
                    } else if let Some(ch) = self.document().text()[range.start..].chars().next() {
                        range.end += ch.len_utf8();
                    }
                }
                let replacement = if command == IDM_EDIT_REDACT_SELECTION {
                    let bullet = unsafe { GetKeyState(VK_SHIFT as i32) < 0 };
                    let unicode = matches!(
                        self.document().encoding(),
                        Encoding::Utf8
                            | Encoding::Utf8Bom
                            | Encoding::Utf16Le
                            | Encoding::Utf16Be
                            | Encoding::Utf16LeNoBom
                            | Encoding::Utf16BeNoBom
                    );
                    let mask = match (unicode, bullet) {
                        (true, true) => '●',
                        (true, false) => '█',
                        (false, true) => '.',
                        _ => '#',
                    };
                    self.document().text()[range.clone()]
                        .chars()
                        .map(|ch| if ch == '\r' || ch == '\n' { ch } else { mask })
                        .collect::<String>()
                } else {
                    String::new()
                };
                if let Err(error) = self.document_mut().replace(range, &replacement) {
                    self.report(error);
                }
                self.refresh();
            }
            IDM_EDIT_AUTOCOMPLETE_CURRENTFILE | IDM_EDIT_AUTOCOMPLETE => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let text = self.document().text();
                let caret = selection.caret;
                let start = text[..caret]
                    .char_indices()
                    .rev()
                    .find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_')
                    .map_or(0, |(at, ch)| at + ch.len_utf8());
                let prefix = &text[start..caret];
                if prefix.is_empty() {
                    return;
                }
                let words: std::collections::BTreeSet<_> = text
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .chain(
                        if command == IDM_EDIT_AUTOCOMPLETE {
                            self.language().keywords.as_str()
                        } else {
                            ""
                        }
                        .split_whitespace(),
                    )
                    .filter(|word| word.starts_with(prefix) && word.len() > prefix.len())
                    .map(str::to_owned)
                    .collect();
                let words: Vec<_> = words.into_iter().take(50).collect();
                unsafe {
                    let menu = CreatePopupMenu();
                    for (index, word) in words.iter().enumerate() {
                        AppendMenuW(menu, MF_STRING, index + 1, wide(word).as_ptr());
                    }
                    let mut point = POINT::default();
                    GetCaretPos(&mut point);
                    ClientToScreen(self.edit, &mut point);
                    let choice = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD,
                        point.x,
                        point.y + 20,
                        0,
                        self.window,
                        null(),
                    );
                    DestroyMenu(menu);
                    if choice > 0 {
                        if let Some(word) = words.get(choice as usize - 1) {
                            if let Err(error) = self.document_mut().replace(start..caret, word) {
                                self.report(error);
                            }
                            self.refresh();
                        }
                    }
                }
            }
            IDM_EDIT_OPENSELECTEDFILETOEDIT | IDM_EDIT_OPENSELECTEDFILEFOLDERINEXPLORER => {
                let (start, end) = control_selection(self.edit);
                let range = self.sync_native_selection(start, end).range();
                let selected = self.document().text()[range].trim().trim_matches('"');
                if selected.is_empty() {
                    return;
                }
                let mut path = PathBuf::from(selected);
                if path.is_relative() {
                    if let Some(parent) = self.document().path().and_then(|p| p.parent()) {
                        path = parent.join(path);
                    }
                }
                let result = if command == IDM_EDIT_OPENSELECTEDFILETOEDIT {
                    self.open(&path)
                } else {
                    crate::file_launch::launch(
                        self.window,
                        &path,
                        crate::file_launch::Action::Reveal,
                    )
                };
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_FILE_RENAME => {
                let result = (|| -> io::Result<()> {
                    let Some(original) = self.document().path().map(Path::to_owned) else {
                        self.save(true);
                        return Ok(());
                    };
                    let Some(value) = crate::input_dialog::ask(
                        self.window,
                        "Rename",
                        "New file name:",
                        &original.file_name().unwrap_or_default().to_string_lossy(),
                    )?
                    else {
                        return Ok(());
                    };
                    let name = Path::new(&value);
                    if name.components().count() != 1
                        || value.trim().is_empty()
                        || value.contains(['/', '\\', ':'])
                        || value == "."
                        || value == ".."
                    {
                        return Err(io::Error::other("Enter a file name without a directory"));
                    }
                    let destination = original.with_file_name(name);
                    if destination == original {
                        return Ok(());
                    }
                    if self
                        .workspace
                        .buffers()
                        .any(|(_, doc)| doc.path() == Some(destination.as_path()))
                    {
                        return Err(io::Error::other("The destination is open in another tab"));
                    }
                    self.document_mut().rename_with(&destination, |from, to| {
                        use std::os::windows::ffi::OsStrExt;
                        let from: Vec<u16> =
                            from.as_os_str().encode_wide().chain(Some(0)).collect();
                        let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
                        if unsafe {
                            windows_sys::Win32::Storage::FileSystem::MoveFileW(
                                from.as_ptr(),
                                to.as_ptr(),
                            )
                        } == 0
                        {
                            Err(io::Error::last_os_error())
                        } else {
                            Ok(())
                        }
                    })?;
                    self.refresh();
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_FILE_DELETE => {
                if let Some(path) = self.document().path().map(Path::to_owned) {
                    unsafe {
                        if MessageBoxW(
                            self.window,
                            wide(&format!(
                                "Move {} to the Recycle Bin?\r\nThe open buffer will be retained.",
                                path.display()
                            ))
                            .as_ptr(),
                            w!("Delete File"),
                            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                        ) == IDYES
                        {
                            use std::os::windows::ffi::OsStrExt;
                            let path: Vec<u16> =
                                path.as_os_str().encode_wide().chain([0, 0]).collect();
                            let mut operation = windows_sys::Win32::UI::Shell::SHFILEOPSTRUCTW {
                                hwnd: self.window,
                                wFunc: windows_sys::Win32::UI::Shell::FO_DELETE,
                                pFrom: path.as_ptr(),
                                fFlags: (windows_sys::Win32::UI::Shell::FOF_ALLOWUNDO
                                    | windows_sys::Win32::UI::Shell::FOF_NOCONFIRMATION)
                                    as u16,
                                ..Default::default()
                            };
                            let result =
                                windows_sys::Win32::UI::Shell::SHFileOperationW(&mut operation);
                            if result == 0 && operation.fAnyOperationsAborted == 0 {
                                self.document_mut().mark_disk_deleted();
                                self.document_mut().set_file_read_only(false);
                                self.refresh();
                            } else if result != 0 {
                                self.report(format!("Could not recycle the file ({result})"));
                            }
                        }
                    }
                }
            }
            IDM_FILE_PRINT | IDM_FILE_PRINTNOW => {
                let (start, end) = control_selection(self.edit);
                let range = self.sync_native_selection(start, end).range();
                let title = self
                    .document()
                    .path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".to_owned());
                let text = self.document().text();
                if let Err(error) = crate::printing::print(
                    self.window,
                    &title,
                    text,
                    (!range.is_empty()).then(|| &text[range]),
                    command == IDM_FILE_PRINTNOW,
                    self.tab_width.get(),
                ) {
                    self.report(error);
                }
            }
            IDM_ABOUT | IDM_DEBUGINFO | IDM_CMDLINEARGUMENTS => {
                let message = match command {
                    IDM_ABOUT => "Notepad++ Rust\r\nAn independent GPL-3.0-or-later rewrite.\r\nBased on the supplied Notepad++ source.\r\nThis application is still under development.".to_owned(),
                    IDM_DEBUGINFO => format!("Notepad++ Rust {}\r\nOS: {}\r\nArchitecture: {}\r\nExecutable: {}\r\nOpen documents: {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, std::env::current_exe().unwrap_or_default().display(), self.workspace.buffers().count()),
                    _ => "Usage: npp-windows.exe [file ...]\r\nOpens each supplied file in a tab. Quote paths containing spaces.".to_owned(),
                };
                unsafe {
                    MessageBoxW(
                        self.window,
                        wide(&message).as_ptr(),
                        w!("Notepad++ Rust"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
            IDM_HOMESWEETHOME
            | IDM_PROJECTPAGE
            | IDM_ONLINEDOCUMENT
            | IDM_FORUM
            | IDM_LANG_UDLCOLLECTION_PROJECT_SITE => {
                let url = match command {
                    IDM_LANG_UDLCOLLECTION_PROJECT_SITE => {
                        "https://github.com/notepad-plus-plus/userDefinedLanguages"
                    }
                    IDM_HOMESWEETHOME => "https://notepad-plus-plus.org/",
                    IDM_PROJECTPAGE => "https://github.com/notepad-plus-plus/notepad-plus-plus",
                    IDM_ONLINEDOCUMENT => "https://npp-user-manual.org/",
                    _ => "https://community.notepad-plus-plus.org/",
                };
                unsafe {
                    let result = windows_sys::Win32::UI::Shell::ShellExecuteW(
                        self.window,
                        w!("open"),
                        wide(url).as_ptr(),
                        null(),
                        null(),
                        SW_SHOWNORMAL,
                    );
                    if result as usize <= 32 {
                        self.report("Could not open the browser");
                    }
                }
            }
            IDM_EDIT_SEARCHONINTERNET => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let mut range = selection.range();
                if range.is_empty() {
                    use crate::scintilla::{ids::*, send};
                    let position = send(self.edit, SCI_GETCURRENTPOS, 0, 0) as usize;
                    range = send(self.edit, SCI_WORDSTARTPOSITION, position, 1) as usize
                        ..send(self.edit, SCI_WORDENDPOSITION, position, 1) as usize;
                }
                if let Some(text) = self.document().text().get(range) {
                    let mut encoded = String::new();
                    for byte in text.bytes() {
                        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                            encoded.push(byte as char);
                        } else {
                            use std::fmt::Write;
                            let _ = write!(encoded, "%{byte:02X}");
                        }
                    }
                    let template = if self.search_engine.starts_with("https://")
                        || self.search_engine.starts_with("http://")
                    {
                        self.search_engine.clone()
                    } else {
                        crate::preferences::Preferences::default().search_engine
                    };
                    let url = template.replace("$(CURRENT_WORD)", &encoded);
                    unsafe {
                        let result = windows_sys::Win32::UI::Shell::ShellExecuteW(
                            self.window,
                            w!("open"),
                            wide(&url).as_ptr(),
                            null(),
                            null(),
                            SW_SHOWNORMAL,
                        );
                        if result as usize <= 32 {
                            self.report("Could not open the search URL");
                        }
                    }
                }
            }
            IDM_SETTING_PREFERENCE | IDM_EDIT_CHANGESEARCHENGINE => {
                let result = (|| -> io::Result<()> {
                    let mut current = self.current_preferences();
                    if command == IDM_EDIT_CHANGESEARCHENGINE {
                        current.initial_page = 4;
                    }
                    if let Some(value) = crate::preferences::show(self.window, current)? {
                        self.apply_preferences(&value)?;
                        crate::preferences::save(&value)?;
                        self.save_recent_files();
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            SHOW_TOOLBAR | SHOW_LINE_NUMBERS => {
                let id = if command == SHOW_TOOLBAR {
                    crate::editor_chrome::TOOLBAR
                } else {
                    crate::editor_chrome::GUTTER
                };
                let hidden = is_control_hidden(self.window, id as usize);
                unsafe {
                    ShowWindow(
                        GetDlgItem(self.window, id),
                        if hidden { SW_SHOWNA } else { SW_HIDE },
                    );
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        command,
                        MF_BYCOMMAND | if hidden { MF_CHECKED } else { MF_UNCHECKED },
                    );
                }
                self.resize();
            }
            IDM_VIEW_HIDELINES => {
                crate::scintilla::hide_selected_lines(self.edit);
            }
            IDM_VIEW_WRAP => {
                if let Err(error) = self.toggle_word_wrap() {
                    self.report(error);
                }
            }
            IDM_MACRO_STARTRECORDINGMACRO => {
                self.macro_steps.clear();
                self.macro_recording = true;
                self.caption();
            }
            IDM_MACRO_STOPRECORDINGMACRO => {
                self.macro_recording = false;
                self.caption();
            }
            IDM_MACRO_PLAYBACKRECORDEDMACRO | IDM_MACRO_RUNMULTIMACRODLG => {
                self.macro_recording = false;
                let count = if command == IDM_MACRO_RUNMULTIMACRODLG {
                    match crate::input_dialog::ask(
                        self.window,
                        "Run Macro",
                        "Number of repetitions (1–10000):",
                        "1",
                    ) {
                        Ok(Some(value)) => match value.parse::<usize>() {
                            Ok(n) if (1..=10000).contains(&n) => n,
                            _ => {
                                self.report("Enter a number from 1 to 10000");
                                return;
                            }
                        },
                        Ok(None) => return,
                        Err(error) => {
                            self.report(error);
                            return;
                        }
                    }
                } else {
                    1
                };
                let steps = self.macro_steps.clone();
                for _ in 0..count {
                    for step in &steps {
                        match step {
                            crate::macros::Step::Command(id) => {
                                if EDIT_COMMANDS.contains(id) {
                                    self.command(*id);
                                }
                            }
                            crate::macros::Step::Edit {
                                offset,
                                remove,
                                insert,
                            } => {
                                if let Err(error) = crate::macros::apply(
                                    self.document_mut(),
                                    *offset,
                                    *remove,
                                    insert,
                                ) {
                                    self.report(error);
                                    self.refresh();
                                    return;
                                }
                                self.refresh();
                            }
                        }
                    }
                }
            }
            IDM_MACRO_SAVECURRENTMACRO | LOAD_MACRO => {
                let save = command == IDM_MACRO_SAVECURRENTMACRO;
                let result = (|| -> io::Result<()> {
                    let Some(path) = file_dialog_titled(
                        self.window,
                        save,
                        Some(if save { "Save Macro" } else { "Load Macro" }),
                    )?
                    else {
                        return Ok(());
                    };
                    if save {
                        atomic_write(
                            &path,
                            &serde_json::to_vec_pretty(&self.macro_steps)
                                .map_err(io::Error::other)?,
                        )?;
                    } else {
                        let steps: Vec<crate::macros::Step> =
                            serde_json::from_slice(&std::fs::read(path)?)
                                .map_err(io::Error::other)?;

                        self.macro_steps = steps;
                        self.macro_recording = false;
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_EXECUTE => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let result = (|| -> io::Result<()> {
                    let Some(value) = crate::input_dialog::ask(
                        self.window,
                        "Run",
                        "Command (supports $(FULL_CURRENT_PATH), $(CURRENT_DIRECTORY), $(FILE_NAME), $(CURRENT_WORD)):",
                        &self.run_command,
                    )?
                    else {
                        return Ok(());
                    };
                    self.run_command = value.clone();
                    let text = self.document().text();
                    let mut word = self.document().selection().range();
                    if word.is_empty() {
                        word.start = text[..word.start]
                            .char_indices()
                            .rev()
                            .find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_')
                            .map_or(0, |(at, ch)| at + ch.len_utf8());
                        word.end += text[word.end..]
                            .chars()
                            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                            .map(char::len_utf8)
                            .sum::<usize>();
                    }
                    let path = self.document().path();
                    let directory = path
                        .and_then(|p| p.parent())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let value = value
                        .replace(
                            "$(FULL_CURRENT_PATH)",
                            &path
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        )
                        .replace("$(CURRENT_DIRECTORY)", &directory)
                        .replace(
                            "$(FILE_NAME)",
                            &path
                                .and_then(|p| p.file_name())
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        )
                        .replace("$(CURRENT_WORD)", &text[word]);
                    if value.trim().is_empty() {
                        return Ok(());
                    }
                    use std::os::windows::process::CommandExt;
                    let mut process = std::process::Command::new("cmd.exe");
                    process
                        .args(["/D", "/S", "/C", &value])
                        .creation_flags(0x08000000);
                    if !directory.is_empty() {
                        process.current_dir(directory);
                    }
                    process.spawn()?;
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_EDIT_BLOCK_COMMENT
            | IDM_EDIT_BLOCK_COMMENT_SET
            | IDM_EDIT_BLOCK_UNCOMMENT
            | IDM_EDIT_STREAM_COMMENT
            | IDM_EDIT_STREAM_UNCOMMENT => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let language = self.language().clone();
                let marker = language.line.as_str();
                let result =
                    if command == IDM_EDIT_STREAM_COMMENT || command == IDM_EDIT_STREAM_UNCOMMENT {
                        let (open, close) = (language.open.as_str(), language.close.as_str());
                        if open.is_empty() || close.is_empty() {
                            return;
                        }
                        self.document_mut().comment_selection(
                            open,
                            close,
                            command == IDM_EDIT_STREAM_UNCOMMENT,
                        )
                    } else {
                        if marker.is_empty() {
                            return;
                        }
                        self.document_mut().comment_lines(
                            marker,
                            match command {
                                IDM_EDIT_BLOCK_COMMENT_SET => Some(true),
                                IDM_EDIT_BLOCK_UNCOMMENT => Some(false),
                                _ => None,
                            },
                        )
                    };
                if let Err(error) = result {
                    self.report(error);
                }
                self.refresh();
            }
            IDM_TOOL_MD5_GENERATE
            | IDM_TOOL_MD5_GENERATEFROMFILE
            | IDM_TOOL_MD5_GENERATEINTOCLIPBOARD
            | IDM_TOOL_SHA1_GENERATE
            | IDM_TOOL_SHA1_GENERATEFROMFILE
            | IDM_TOOL_SHA1_GENERATEINTOCLIPBOARD
            | IDM_TOOL_SHA256_GENERATE
            | IDM_TOOL_SHA256_GENERATEFROMFILE
            | IDM_TOOL_SHA256_GENERATEINTOCLIPBOARD
            | IDM_TOOL_SHA512_GENERATE
            | IDM_TOOL_SHA512_GENERATEFROMFILE
            | IDM_TOOL_SHA512_GENERATEINTOCLIPBOARD => {
                if let Err(error) = self.hash_command(command) {
                    self.report(error);
                }
            }
            IDM_WINDOW_SORT_FN_ASC
            | IDM_WINDOW_SORT_FN_DSC
            | IDM_WINDOW_SORT_FP_ASC
            | IDM_WINDOW_SORT_FP_DSC
            | IDM_WINDOW_SORT_FT_ASC
            | IDM_WINDOW_SORT_FT_DSC
            | IDM_WINDOW_SORT_FS_ASC
            | IDM_WINDOW_SORT_FS_DSC
            | IDM_WINDOW_SORT_FD_ASC
            | IDM_WINDOW_SORT_FD_DSC => {
                let original = self.active_id();
                let mut entries: Vec<_> = self
                    .workspace
                    .buffers()
                    .map(|(id, doc)| {
                        let path = doc
                            .path()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|| {
                                self.workspace
                                    .untitled_name(id)
                                    .unwrap_or_default()
                                    .to_owned()
                            });
                        let name = Path::new(&path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase();
                        let extension = Path::new(&path)
                            .extension()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase();
                        let modified = doc
                            .path()
                            .and_then(|p| std::fs::metadata(p).ok())
                            .and_then(|m| m.modified().ok());
                        (
                            id,
                            name,
                            path.to_lowercase(),
                            extension,
                            doc.text().len(),
                            modified,
                        )
                    })
                    .collect();
                entries.sort_by(|a, b| match command {
                    IDM_WINDOW_SORT_FP_ASC | IDM_WINDOW_SORT_FP_DSC => a.2.cmp(&b.2),
                    IDM_WINDOW_SORT_FT_ASC | IDM_WINDOW_SORT_FT_DSC => a.3.cmp(&b.3),
                    IDM_WINDOW_SORT_FS_ASC | IDM_WINDOW_SORT_FS_DSC => a.4.cmp(&b.4),
                    IDM_WINDOW_SORT_FD_ASC | IDM_WINDOW_SORT_FD_DSC => a.5.cmp(&b.5),
                    _ => a.1.cmp(&b.1),
                });
                if [
                    IDM_WINDOW_SORT_FN_DSC,
                    IDM_WINDOW_SORT_FP_DSC,
                    IDM_WINDOW_SORT_FT_DSC,
                    IDM_WINDOW_SORT_FS_DSC,
                    IDM_WINDOW_SORT_FD_DSC,
                ]
                .contains(&command)
                {
                    entries.reverse();
                }
                for entry in entries.iter().rev() {
                    self.workspace.activate(entry.0);
                    while self.workspace.move_active(false) {}
                }
                self.workspace.activate(original);
                self.refresh_tabs();
            }
            IDM_VIEW_ZOOMIN | IDM_VIEW_ZOOMOUT | IDM_VIEW_ZOOMRESTORE => {
                let zoom = match command {
                    IDM_VIEW_ZOOMIN => self.zoom + 1,
                    IDM_VIEW_ZOOMOUT => self.zoom - 1,
                    _ => 0,
                };
                if let Err(error) = self.set_zoom(zoom) {
                    self.report(error);
                }
            }
            IDM_EDIT_TOGGLESYSTEMREADONLY => {
                if let Some(path) = self.document().path() {
                    match crate::file_io::toggle_file_read_only(path) {
                        Ok(read_only) => {
                            self.attribute_check = None;
                            self.document_mut().set_file_read_only(read_only);
                            self.refresh();
                        }
                        Err(error) => self.report(error),
                    }
                }
            }
            IDM_EDIT_TOGGLEREADONLY => {
                let value = !self.document().is_user_read_only();
                self.document_mut().set_read_only(value);
                self.refresh();
            }
            IDM_EDIT_SETREADONLYFORALLDOCS | IDM_EDIT_CLEARREADONLYFORALLDOCS => {
                let value = command == IDM_EDIT_SETREADONLYFORALLDOCS;
                let ids: Vec<_> = self.workspace.buffers().map(|(id, _)| id).collect();
                for id in ids {
                    self.workspace.get_mut(id).unwrap().set_read_only(value);
                }
                self.refresh();
            }
            BASIC_AUTO_INDENT => {
                self.basic_auto_indent = !self.basic_auto_indent;
                // SAFETY: The current UI thread owns this window and menu.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        BASIC_AUTO_INDENT,
                        MF_BYCOMMAND
                            | if self.basic_auto_indent {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
            }
            INSERT_NEWLINE => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let (width, style) = (self.tab_width, self.indentation_style);
                if self.basic_auto_indent && self.advanced_indent {
                    let (replacement, caret) = crate::formatting::newline(
                        self.edit,
                        self.document().text(),
                        selection.range(),
                        self.language().command,
                        width.get(),
                        style,
                        self.document().line_ending().as_str(),
                    );
                    if let Err(error) = self.document_mut().replace(selection.range(), &replacement)
                    {
                        self.report(error);
                    } else {
                        let _ = self.document_mut().select(npp_editor_core::Selection {
                            anchor: caret,
                            caret,
                        });
                    }
                } else if let Err(error) = {
                    let basic = self.basic_auto_indent;
                    self.document_mut().insert_newline(basic, width, style)
                } {
                    self.report(error);
                }
                self.refresh();
            }
            INDENT_WITH_SPACES => {
                self.indentation_style = if self.indentation_style == IndentationStyle::Tabs {
                    IndentationStyle::Spaces
                } else {
                    IndentationStyle::Tabs
                };
                // SAFETY: The main window owns this menu on the current UI thread.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        INDENT_WITH_SPACES,
                        MF_BYCOMMAND
                            | if self.indentation_style == IndentationStyle::Spaces {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
            }
            INSERT_TAB | BACK_TAB | IDM_EDIT_INS_TAB | IDM_EDIT_RMV_TAB => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let width = self.tab_width;
                let style = self.indentation_style;
                let result = self.document_mut().indent(
                    matches!(command, INSERT_TAB | IDM_EDIT_INS_TAB),
                    matches!(command, IDM_EDIT_INS_TAB | IDM_EDIT_RMV_TAB),
                    width,
                    style,
                );
                if let Err(error) = result {
                    if !matches!(error, npp_editor_core::EditError::ReadOnly) {
                        self.report(error);
                    }
                }
                self.refresh();
            }
            TAB_WIDTH_2 | TAB_WIDTH_4 | TAB_WIDTH_8 => {
                let width = match command {
                    TAB_WIDTH_2 => 2,
                    TAB_WIDTH_4 => 4,
                    _ => 8,
                };
                if let Err(error) = self.set_tab_width(width) {
                    self.report(error);
                }
            }
            SEARCH_WRAP => {
                self.search_options.wrap = !self.search_options.wrap;
                if let Some(dialog) = &self.search_dialog {
                    dialog.set_wrap(self.search_options.wrap);
                }
                // SAFETY: Update this window's retained menu.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        SEARCH_WRAP,
                        MF_BYCOMMAND
                            | if self.search_options.wrap {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
            }
            SEARCH_IN_SELECTION => {
                self.search_in_selection = !self.search_in_selection;
                // SAFETY: Live main window owns this menu, addressed by command ID.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        SEARCH_IN_SELECTION,
                        MF_BYCOMMAND
                            | if self.search_in_selection {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
            }
            SEARCH_COUNT | SEARCH_BOOKMARK_MATCHES => {
                if let Some(query) = self
                    .search_dialog
                    .as_ref()
                    .and_then(SearchDialog::current_query)
                {
                    self.search_text = query.needle;
                    self.search_options = SearchOptions {
                        mode: self.search_options.mode,
                        wrap: self.search_options.wrap,
                        ..query.options
                    };
                }
                if self.search_text.is_empty() {
                    self.show_search(false);
                    return;
                }
                let scope = self.search_scope();
                let result = if command == SEARCH_BOOKMARK_MATCHES {
                    let needle = self.search_text.clone();
                    let options = self.search_options;
                    self.document_mut()
                        .bookmark_matches_in(&needle, options, scope)
                } else {
                    self.document()
                        .count_matches_in(&self.search_text, self.search_options, scope)
                };
                match result {
                    Ok(count) => {
                        if command == SEARCH_BOOKMARK_MATCHES {
                            self.refresh_selection();
                        }
                        let label = if command == SEARCH_BOOKMARK_MATCHES {
                            "Bookmarked matches"
                        } else {
                            "Count"
                        };
                        // SAFETY: Live status control copies the temporary string.
                        unsafe {
                            SetWindowTextW(
                                self.status,
                                wide(&format!("{label}: {count} occurrences")).as_ptr(),
                            );
                        }
                    }
                    Err(error) => self.report(error),
                }
            }
            SEARCH_REGEX => {
                self.search_options.mode = if self.search_options.mode == SearchMode::Regex {
                    SearchMode::Normal
                } else {
                    SearchMode::Regex
                };
                unsafe {
                    let menu = crate::plugin_host::main_menu(self.window);
                    CheckMenuItem(
                        menu,
                        SEARCH_REGEX,
                        MF_BYCOMMAND
                            | if self.search_options.mode == SearchMode::Regex {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                    CheckMenuItem(menu, SEARCH_EXTENDED, MF_BYCOMMAND | MF_UNCHECKED);
                }
                if let Some(dialog) = &self.search_dialog {
                    dialog.set_mode(self.search_options.mode);
                }
            }
            SEARCH_EXTENDED => {
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        SEARCH_REGEX,
                        MF_BYCOMMAND | MF_UNCHECKED,
                    );
                }
                self.search_options.mode = if self.search_options.mode == SearchMode::Normal {
                    SearchMode::Extended
                } else {
                    SearchMode::Normal
                };
                // SAFETY: Live main window owns this menu; command is addressed by ID.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        SEARCH_EXTENDED,
                        MF_BYCOMMAND
                            | if self.search_options.mode == SearchMode::Extended {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
                if let Some(dialog) = &self.search_dialog {
                    dialog.set_mode(self.search_options.mode);
                }
            }
            IDM_SEARCH_GOTOMATCHINGBRACE | IDM_SEARCH_SELECTMATCHINGBRACES => {
                use crate::scintilla::{
                    ids::{SCI_CHOOSECARETX, SCI_GOTOPOS, SCI_SETSEL},
                    matching_braces, send,
                };
                let (brace, opposite) = matching_braces(self.edit);
                if opposite >= 0 {
                    if command == IDM_SEARCH_GOTOMATCHINGBRACE {
                        send(self.edit, SCI_GOTOPOS, opposite as usize, 0);
                    } else {
                        send(
                            self.edit,
                            SCI_SETSEL,
                            brace.min(opposite) as usize,
                            brace.max(opposite) + 1,
                        );
                    }
                    send(self.edit, SCI_CHOOSECARETX, 0, 0);
                    let (start, end) = control_selection(self.edit);
                    self.sync_native_selection(start, end);
                    self.caption();
                }
            }
            IDM_SEARCH_GOTOLINE => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                if self.goto_dialog.is_none() {
                    match crate::goto_dialog::GoToDialog::new(self.window) {
                        Ok(dialog) => self.goto_dialog = Some(dialog),
                        Err(error) => self.report(error),
                    }
                }
                if let Some(dialog) = &self.goto_dialog {
                    dialog.show(self.document());
                }
            }
            SHOW_STATUS_BAR => {
                // SAFETY: Native-only handler does not borrow App reentrantly.
                unsafe {
                    SendMessageW(
                        self.window,
                        npp_compat::messages::NPPM_HIDESTATUSBAR,
                        0,
                        (!is_status_hidden(self.window)) as isize,
                    );
                }
            }
            SHOW_MENU_BAR => {
                // SAFETY: The native handler retains the detached menu and
                // does not borrow App during synchronous dispatch.
                unsafe {
                    SendMessageW(
                        self.window,
                        npp_compat::messages::NPPM_HIDEMENU,
                        0,
                        (!GetMenu(self.window).is_null()) as isize,
                    );
                }
            }
            LINE_END
            | LINE_END_EXTEND
            | DOCUMENT_START
            | DOCUMENT_START_EXTEND
            | DOCUMENT_END
            | DOCUMENT_END_EXTEND => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let extend = matches!(
                    command,
                    LINE_END_EXTEND | DOCUMENT_START_EXTEND | DOCUMENT_END_EXTEND
                );
                if matches!(command, LINE_END | LINE_END_EXTEND) {
                    let _ = self.document_mut().move_line_end(extend);
                } else {
                    let end = matches!(command, DOCUMENT_END | DOCUMENT_END_EXTEND);
                    let _ = self.document_mut().move_document_boundary(end, extend);
                }
                self.refresh_selection();
            }
            PARAGRAPH_UP | PARAGRAPH_DOWN | PARAGRAPH_UP_EXTEND | PARAGRAPH_DOWN_EXTEND => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let down = matches!(command, PARAGRAPH_DOWN | PARAGRAPH_DOWN_EXTEND);
                let extend = matches!(command, PARAGRAPH_UP_EXTEND | PARAGRAPH_DOWN_EXTEND);
                let _ = self.document_mut().move_paragraph(down, extend);
                self.refresh_selection();
            }
            WORD_PART_LEFT | WORD_PART_RIGHT | WORD_PART_LEFT_EXTEND | WORD_PART_RIGHT_EXTEND => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let right = matches!(command, WORD_PART_RIGHT | WORD_PART_RIGHT_EXTEND);
                let extend = matches!(command, WORD_PART_LEFT_EXTEND | WORD_PART_RIGHT_EXTEND);
                let _ = self.document_mut().move_word_part(right, extend);
                self.refresh_selection();
            }
            SMART_HOME | SMART_HOME_EXTEND => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let _ = self.document_mut().move_home(command == SMART_HOME_EXTEND);
                self.refresh_selection();
            }
            WORD_LEFT | WORD_RIGHT | WORD_LEFT_EXTEND | WORD_RIGHT_EXTEND => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let rightwards = matches!(command, WORD_RIGHT | WORD_RIGHT_EXTEND)
                    ^ self.rtl_buffers.contains(&self.active_id());
                let extend = matches!(command, WORD_LEFT_EXTEND | WORD_RIGHT_EXTEND);
                let _ = self.document_mut().move_word(rightwards, extend);
                self.refresh_selection();
            }
            SHOW_TAB_BAR => {
                // SAFETY: Visibility is handled synchronously using native HWNDs.
                unsafe {
                    SendMessageW(
                        self.window,
                        npp_compat::messages::NPPM_HIDETABBAR,
                        0,
                        (!is_control_hidden(self.window, TAB_ID)) as isize,
                    );
                }
            }
            IDM_VIEW_IN_FIREFOX | IDM_VIEW_IN_CHROME | IDM_VIEW_IN_EDGE | IDM_VIEW_IN_IE => {
                if let Some(path) = self.document().path() {
                    let executable = match command {
                        IDM_VIEW_IN_FIREFOX => "firefox.exe",
                        IDM_VIEW_IN_CHROME => "chrome.exe",
                        IDM_VIEW_IN_EDGE => "msedge.exe",
                        _ => "IEXPLORE.EXE",
                    };
                    if let Err(error) = crate::file_launch::browser(self.window, path, executable) {
                        self.report(error);
                    }
                }
            }
            IDM_FILE_OPEN_FOLDER | IDM_FILE_OPEN_DEFAULT_VIEWER => {
                use crate::file_launch::{Action, launch};
                let action = if command == IDM_FILE_OPEN_FOLDER {
                    Action::Reveal
                } else {
                    Action::DefaultViewer
                };
                if let Err(error) =
                    self.launch_file_with(action, |path, action| launch(self.window, path, action))
                {
                    self.report(error);
                }
            }
            IDM_FILE_OPEN_CMD => {
                if let Some(directory) = self.document().path().and_then(Path::parent) {
                    let executable = std::env::var_os("COMSPEC")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("cmd.exe"));
                    if let Err(error) =
                        crate::file_launch::terminal(self.window, directory, &executable)
                    {
                        self.report(error);
                    }
                }
            }
            IDM_FILE_OPEN_POWERSHELL => {
                if let Some(directory) = self.document().path().and_then(Path::parent) {
                    let result = crate::file_launch::powershell_path().and_then(|executable| {
                        crate::file_launch::terminal(self.window, directory, &executable)
                    });
                    if let Err(error) = result {
                        self.report(error);
                    }
                }
            }
            IDM_VIEW_SUMMARY => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                match crate::summary::text(self.document()) {
                    Ok(text) => {
                        // SAFETY: Temporary terminated strings survive the modal call.
                        unsafe {
                            MessageBoxW(
                                self.window,
                                wide(&text).as_ptr(),
                                wide("Summary").as_ptr(),
                                MB_OK | MB_APPLMODAL,
                            );
                        }
                    }
                    Err(error) => self.report(error),
                }
            }
            IDM_EDIT_BEGINENDSELECT | IDM_EDIT_BEGINENDSELECT_COLUMNMODE => {
                let column = command == IDM_EDIT_BEGINENDSELECT_COLUMNMODE;
                let ending = self.selection_gesture.is_started();
                if ending && column != self.column_selection_gesture {
                    return;
                }
                if !ending {
                    self.column_selection_gesture = column;
                }
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                // The EDIT compatibility selection is sorted; use the native caret
                // so backwards selections mark the same endpoint as upstream.
                let native_caret = crate::scintilla::send(
                    self.edit,
                    crate::scintilla::ids::SCI_GETCURRENTPOS,
                    0,
                    0,
                ) as usize;
                let caret = self
                    .projection
                    .byte_offset(self.projection.text[..native_caret].encode_utf16().count());
                let mut selection = self.document().selection();
                selection.caret = caret;
                let _ = self.document_mut().select(selection);
                let gesture = self.selection_gesture.clone();
                if let Err(error) = gesture.toggle(self.document_mut()) {
                    self.report(error);
                    return;
                }
                if column && ending {
                    use crate::scintilla::{ids::*, send};
                    let selection = self.document().selection();
                    let anchor = Projection::new(&self.document().text()[..selection.anchor])
                        .text
                        .len();
                    let caret = Projection::new(&self.document().text()[..selection.caret])
                        .text
                        .len();
                    send(
                        self.edit,
                        SCI_CHANGESELECTIONMODE,
                        SC_SEL_RECTANGLE as usize,
                        0,
                    );
                    send(self.edit, SCI_SETRECTANGULARSELECTIONANCHOR, anchor, 0);
                    send(self.edit, SCI_SETRECTANGULARSELECTIONCARET, caret, 0);
                    self.caption();
                } else {
                    self.refresh_selection();
                }
            }
            OPEN_1252 | OPEN_1251 | OPEN_SHIFT_JIS | OPEN_UTF16LE_NO_BOM | OPEN_UTF16BE_NO_BOM => {
                let encoding = match command {
                    OPEN_1252 => Encoding::Windows1252,
                    OPEN_1251 => Encoding::Windows1251,
                    OPEN_UTF16LE_NO_BOM => Encoding::Utf16LeNoBom,
                    OPEN_UTF16BE_NO_BOM => Encoding::Utf16BeNoBom,
                    _ => Encoding::ShiftJis,
                };
                match file_dialog(self.window, false) {
                    Ok(Some(path)) => {
                        if let Err(e) = self.open_as(&path, Some(encoding)) {
                            self.report(e);
                        }
                    }
                    Ok(None) => (),
                    Err(e) => self.report(e),
                }
            }
            CONVERT_1252
            | CONVERT_1251
            | CONVERT_SHIFT_JIS
            | CONVERT_UTF16LE_NO_BOM
            | CONVERT_UTF16BE_NO_BOM => {
                let encoding = match command {
                    CONVERT_1252 => Encoding::Windows1252,
                    CONVERT_1251 => Encoding::Windows1251,
                    CONVERT_UTF16LE_NO_BOM => Encoding::Utf16LeNoBom,
                    CONVERT_UTF16BE_NO_BOM => Encoding::Utf16BeNoBom,
                    _ => Encoding::ShiftJis,
                };
                if let Err(e) = self.document_mut().set_encoding(encoding) {
                    self.report(e);
                }
                self.caption();
                self.refresh_tabs();
            }
            RECENT_FILE_FIRST..=RECENT_FILE_LAST | IDM_OPEN_ALL_RECENT_FILE => {
                let mut entries: Vec<_> = self.workspace.recent_files().cloned().collect();
                if command == IDM_OPEN_ALL_RECENT_FILE {
                    entries.reverse();
                } else {
                    entries = entries
                        .into_iter()
                        .nth((command - RECENT_FILE_FIRST) as usize)
                        .into_iter()
                        .collect();
                }
                let mut failures = Vec::new();
                for entry in entries {
                    let encoding = entry
                        .encoding
                        .requires_explicit_decode()
                        .then_some(entry.encoding);
                    if let Err(error) = self.open_as(&entry.path, encoding) {
                        failures.push(format!("{}: {error}", entry.path.display()));
                    }
                }
                if !failures.is_empty() {
                    self.report(failures.join("\n"));
                }
            }
            IDM_CLEAN_RECENT_FILE_LIST => {
                self.workspace.restore_recent_files(std::iter::empty());
                self.save_recent_files();
                self.caption();
            }
            IDM_FILE_RESTORELASTCLOSEDFILE => {
                if let Some(recent) = self.workspace.latest_closed().cloned() {
                    let encoding = match recent.encoding {
                        encoding if encoding.requires_explicit_decode() => Some(recent.encoding),
                        _ => None,
                    };
                    if let Err(e) = self.open_as(&recent.path, encoding) {
                        self.report(e);
                    }
                }
            }
            IDM_VIEW_TAB_NEXT
            | IDM_VIEW_TAB_PREV
            | IDM_VIEW_TAB_MOVEFORWARD
            | IDM_VIEW_TAB_MOVEBACKWARD => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                match command {
                    IDM_VIEW_TAB_NEXT | IDM_VIEW_TAB_PREV => {
                        self.workspace
                            .activate_adjacent(command == IDM_VIEW_TAB_NEXT);
                    }
                    _ => {
                        self.workspace
                            .move_active(command == IDM_VIEW_TAB_MOVEFORWARD);
                    }
                }
                self.refresh();
            }
            IDM_EDIT_MULTISELECTALL
            | IDM_EDIT_MULTISELECTALLMATCHCASE
            | IDM_EDIT_MULTISELECTALLWHOLEWORD
            | IDM_EDIT_MULTISELECTALLMATCHCASEWHOLEWORD
            | IDM_EDIT_MULTISELECTNEXT
            | IDM_EDIT_MULTISELECTNEXTMATCHCASE
            | IDM_EDIT_MULTISELECTNEXTWHOLEWORD
            | IDM_EDIT_MULTISELECTNEXTMATCHCASEWHOLEWORD
            | IDM_EDIT_MULTISELECTUNDO
            | IDM_EDIT_MULTISELECTSSKIP => {
                use crate::scintilla::{ids::*, send};
                if send(self.edit, SCI_GETMULTIPLESELECTION, 0, 0) == 0 {
                    return;
                }
                if command == IDM_EDIT_MULTISELECTUNDO {
                    let count = send(self.edit, SCI_GETSELECTIONS, 0, 0);
                    if count > 0 {
                        send(self.edit, SCI_DROPSELECTIONN, (count - 1) as usize, 0);
                    }
                } else {
                    if command != IDM_EDIT_MULTISELECTSSKIP {
                        self.multi_select_flags = match command {
                            IDM_EDIT_MULTISELECTALLMATCHCASE
                            | IDM_EDIT_MULTISELECTNEXTMATCHCASE => SCFIND_MATCHCASE as usize,
                            IDM_EDIT_MULTISELECTALLWHOLEWORD
                            | IDM_EDIT_MULTISELECTNEXTWHOLEWORD => SCFIND_WHOLEWORD as usize,
                            IDM_EDIT_MULTISELECTALLMATCHCASEWHOLEWORD
                            | IDM_EDIT_MULTISELECTNEXTMATCHCASEWHOLEWORD => {
                                (SCFIND_MATCHCASE | SCFIND_WHOLEWORD) as usize
                            }
                            _ => 0,
                        };
                    }
                    let all = matches!(
                        command,
                        IDM_EDIT_MULTISELECTALL
                            | IDM_EDIT_MULTISELECTALLMATCHCASE
                            | IDM_EDIT_MULTISELECTALLWHOLEWORD
                            | IDM_EDIT_MULTISELECTALLMATCHCASEWHOLEWORD
                    );
                    if all
                        && send(self.edit, SCI_GETSELECTIONSTART, 0, 0)
                            == send(self.edit, SCI_GETSELECTIONEND, 0, 0)
                    {
                        let caret = send(self.edit, SCI_GETCURRENTPOS, 0, 0) as usize;
                        let start = send(self.edit, SCI_WORDSTARTPOSITION, caret, 1);
                        let end = send(self.edit, SCI_WORDENDPOSITION, caret, 1);
                        if start == end {
                            return;
                        }
                        send(self.edit, SCI_SETSEL, start as usize, end);
                    }
                    send(self.edit, SCI_TARGETWHOLEDOCUMENT, 0, 0);
                    send(self.edit, SCI_SETSEARCHFLAGS, self.multi_select_flags, 0);
                    send(
                        self.edit,
                        if all {
                            SCI_MULTIPLESELECTADDEACH
                        } else {
                            SCI_MULTIPLESELECTADDNEXT
                        },
                        0,
                        0,
                    );
                    if command == IDM_EDIT_MULTISELECTSSKIP {
                        let count = send(self.edit, SCI_GETSELECTIONS, 0, 0);
                        if count > 1 {
                            send(self.edit, SCI_DROPSELECTIONN, (count - 2) as usize, 0);
                        }
                    }
                }
                self.caption();
            }
            IDM_EDIT_COLUMNMODETIP => {
                // SAFETY: Static text and a live owner remain valid for the modal dialog.
                unsafe {
                    MessageBoxW(
                        self.window,
                        w!(
                            "Column selection gestures:\r\n\r\n1. Hold Alt while dragging with the left mouse button.\r\n\r\n2. Hold Alt+Shift while using the arrow keys.\r\n\r\n3. Put the caret at one corner and choose Begin/End Select in Column Mode. Move to the opposite corner and choose the command again.\r\n\r\nColumn editing is incomplete: copy, cut, paste, undo and document refresh do not yet preserve rectangular selections."
                        ),
                        w!("Column Mode Tip"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
            IDM_EDIT_RTL | IDM_EDIT_LTR => {
                let rtl = command == IDM_EDIT_RTL;
                if rtl {
                    self.rtl_buffers.insert(self.active_id());
                } else {
                    self.rtl_buffers.remove(&self.active_id());
                }
                crate::scintilla::text_direction(self.edit, rtl);
                self.caption();
            }
            IDM_FILE_SAVEALL => {
                if self.save_all_confirm {
                    let buttons = [
                        TASKDIALOG_BUTTON {
                            nButtonID: IDYES,
                            pszButtonText: w!("&Yes"),
                        },
                        TASKDIALOG_BUTTON {
                            nButtonID: IDRETRY,
                            pszButtonText: w!("&Always yes"),
                        },
                        TASKDIALOG_BUTTON {
                            nButtonID: IDNO,
                            pszButtonText: w!("&No"),
                        },
                    ];
                    let config = TASKDIALOGCONFIG {
                        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
                        hwndParent: self.window,
                        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW,
                        pszWindowTitle: w!("Save"),
                        pszMainInstruction: w!("Save all modified documents?"),
                        pszContent: w!(
                            "Choose Always yes to save without this confirmation in future. You can enable confirmation again in Preferences."
                        ),
                        cButtons: buttons.len() as u32,
                        pButtons: buttons.as_ptr(),
                        nDefaultButton: IDYES,
                        ..Default::default()
                    };
                    let mut answer = IDCANCEL;
                    // SAFETY: The parent is live and all dialog strings and button storage
                    // remain valid throughout this synchronous modal call.
                    let result =
                        unsafe { TaskDialogIndirect(&config, &mut answer, null_mut(), null_mut()) };
                    if result < 0 {
                        self.report(io::Error::other(format!(
                            "Cannot display Save All confirmation (HRESULT {result:#x})"
                        )));
                        return;
                    }
                    if !matches!(answer, IDYES | IDRETRY) {
                        return;
                    }
                    if answer == IDRETRY {
                        self.save_all_confirm = false;
                        if let Err(error) = crate::preferences::save(&self.current_preferences()) {
                            self.report(error);
                        }
                    }
                }
                self.save_all_with(|app| app.save(false));
            }
            IDM_FILE_RELOAD => {
                if self.document().path().is_none() {
                    return;
                }
                // SAFETY: Modal prompt is owned by the live main window.
                if self.document().is_dirty()
                    && unsafe {
                        MessageBoxW(
                            self.window,
                            w!("Discard unsaved changes and reload from disk?"),
                            w!("Reload from Disk"),
                            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                        )
                    } != IDYES
                {
                    return;
                }
                if let Err(e) = self.reload() {
                    self.report(e);
                }
            }
            IDM_FILE_CLOSEALL
            | IDM_FILE_CLOSEALL_BUT_CURRENT
            | IDM_FILE_CLOSEALL_TOLEFT
            | IDM_FILE_CLOSEALL_TORIGHT
            | IDM_FILE_CLOSEALL_UNCHANGED => {
                let scope = match command {
                    IDM_FILE_CLOSEALL => CloseScope::All,
                    IDM_FILE_CLOSEALL_BUT_CURRENT => CloseScope::Others,
                    IDM_FILE_CLOSEALL_TOLEFT => CloseScope::Left,
                    IDM_FILE_CLOSEALL_TORIGHT => CloseScope::Right,
                    _ => CloseScope::Unchanged,
                };
                self.close_documents(self.workspace.close_targets(scope), |app| {
                    app.confirm_close()
                });
            }
            IDM_EDIT_UPPERCASE
            | IDM_EDIT_LOWERCASE
            | IDM_EDIT_INVERTCASE
            | IDM_EDIT_RANDOMCASE
            | IDM_EDIT_PROPERCASE_FORCE
            | IDM_EDIT_PROPERCASE_BLEND
            | IDM_EDIT_SENTENCECASE_FORCE
            | IDM_EDIT_SENTENCECASE_BLEND
            | IDM_EDIT_DUP_LINE
            | DELETE_LINE
            | TRANSPOSE_LINE
            | DELETE_LINE_LEFT
            | DELETE_LINE_RIGHT
            | DELETE_WORD_LEFT
            | DELETE_WORD_RIGHT
            | IDM_EDIT_TRIMTRAILING
            | IDM_EDIT_TRIMLINEHEAD
            | IDM_EDIT_TRIM_BOTH
            | IDM_EDIT_TAB2SW
            | IDM_EDIT_SW2TAB_LEADING
            | IDM_EDIT_SW2TAB_ALL
            | IDM_EDIT_REMOVE_CONSECUTIVE_DUP_LINES
            | IDM_EDIT_REMOVE_ANY_DUP_LINES
            | IDM_EDIT_REMOVEEMPTYLINES
            | IDM_EDIT_JOIN_LINES
            | IDM_EDIT_LINE_UP
            | IDM_EDIT_LINE_DOWN
            | IDM_EDIT_BLANKLINEABOVECURRENT
            | IDM_EDIT_BLANKLINEBELOWCURRENT
            | IDM_EDIT_REMOVEEMPTYLINESWITHBLANK
            | IDM_EDIT_SORTLINES_LEXICOGRAPHIC_ASCENDING
            | IDM_EDIT_SORTLINES_LEXICOGRAPHIC_DESCENDING
            | IDM_EDIT_SORTLINES_REVERSE_ORDER
            | IDM_EDIT_SORTLINES_RANDOMLY
            | IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_ASCENDING
            | IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_DESCENDING
            | IDM_EDIT_SORTLINES_LENGTH_ASCENDING
            | IDM_EDIT_SORTLINES_LENGTH_DESCENDING
            | IDM_EDIT_SORTLINES_INTEGER_ASCENDING
            | IDM_EDIT_SORTLINES_INTEGER_DESCENDING
            | IDM_EDIT_SORTLINES_DECIMALDOT_ASCENDING
            | IDM_EDIT_SORTLINES_DECIMALDOT_DESCENDING
            | IDM_EDIT_SORTLINES_DECIMALCOMMA_ASCENDING
            | IDM_EDIT_SORTLINES_DECIMALCOMMA_DESCENDING
            | IDM_EDIT_SORTLINES_LOCALE_ASCENDING
            | IDM_EDIT_SORTLINES_LOCALE_DESCENDING => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                let result = match command {
                    IDM_EDIT_LINE_UP | IDM_EDIT_LINE_DOWN => self
                        .document_mut()
                        .move_lines(command == IDM_EDIT_LINE_DOWN),
                    IDM_EDIT_BLANKLINEABOVECURRENT | IDM_EDIT_BLANKLINEBELOWCURRENT => self
                        .document_mut()
                        .insert_blank_line(command == IDM_EDIT_BLANKLINEBELOWCURRENT),
                    IDM_EDIT_JOIN_LINES => self.document_mut().join_lines(),
                    IDM_EDIT_REMOVEEMPTYLINES | IDM_EDIT_REMOVEEMPTYLINESWITHBLANK => self
                        .document_mut()
                        .remove_empty_lines(command == IDM_EDIT_REMOVEEMPTYLINESWITHBLANK),
                    IDM_EDIT_REMOVE_CONSECUTIVE_DUP_LINES => self
                        .document_mut()
                        .remove_duplicate_lines(DuplicateLines::Consecutive),
                    IDM_EDIT_REMOVE_ANY_DUP_LINES => self
                        .document_mut()
                        .remove_duplicate_lines(DuplicateLines::Any),
                    IDM_EDIT_TAB2SW | IDM_EDIT_SW2TAB_LEADING | IDM_EDIT_SW2TAB_ALL => {
                        let tab_width = self.tab_width;
                        let mode = match command {
                            IDM_EDIT_TAB2SW => WhitespaceConversion::TabsToSpaces,
                            IDM_EDIT_SW2TAB_LEADING => WhitespaceConversion::SpacesToTabsLeading,
                            _ => WhitespaceConversion::SpacesToTabsAll,
                        };
                        self.document_mut().convert_whitespace(mode, tab_width)
                    }
                    IDM_EDIT_SORTLINES_LEXICOGRAPHIC_ASCENDING => {
                        self.document_mut().order_lines(LineOrder::Ascending)
                    }
                    IDM_EDIT_SORTLINES_LEXICOGRAPHIC_DESCENDING => {
                        self.document_mut().order_lines(LineOrder::Descending)
                    }
                    IDM_EDIT_SORTLINES_REVERSE_ORDER => {
                        self.document_mut().order_lines(LineOrder::Reverse)
                    }
                    IDM_EDIT_SORTLINES_RANDOMLY => {
                        let seed = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        self.document_mut().order_lines(LineOrder::Random(seed))
                    }
                    IDM_EDIT_SORTLINES_LOCALE_ASCENDING | IDM_EDIT_SORTLINES_LOCALE_DESCENDING => {
                        self.document_mut().order_lines_locale(
                            command == IDM_EDIT_SORTLINES_LOCALE_DESCENDING,
                            crate::locale_sort::key,
                        )
                    }
                    IDM_EDIT_SORTLINES_DECIMALDOT_ASCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::DecimalDotAscending),
                    IDM_EDIT_SORTLINES_DECIMALDOT_DESCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::DecimalDotDescending),
                    IDM_EDIT_SORTLINES_DECIMALCOMMA_ASCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::DecimalCommaAscending),
                    IDM_EDIT_SORTLINES_DECIMALCOMMA_DESCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::DecimalCommaDescending),
                    IDM_EDIT_SORTLINES_INTEGER_ASCENDING => {
                        self.document_mut().order_lines(LineOrder::IntegerAscending)
                    }
                    IDM_EDIT_SORTLINES_INTEGER_DESCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::IntegerDescending),
                    IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_ASCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::CaseInsensitiveAscending),
                    IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_DESCENDING => self
                        .document_mut()
                        .order_lines(LineOrder::CaseInsensitiveDescending),
                    IDM_EDIT_SORTLINES_LENGTH_ASCENDING => {
                        self.document_mut().order_lines(LineOrder::LengthAscending)
                    }
                    IDM_EDIT_SORTLINES_LENGTH_DESCENDING => {
                        self.document_mut().order_lines(LineOrder::LengthDescending)
                    }
                    IDM_EDIT_UPPERCASE
                    | IDM_EDIT_LOWERCASE
                    | IDM_EDIT_INVERTCASE
                    | IDM_EDIT_RANDOMCASE
                    | IDM_EDIT_PROPERCASE_FORCE
                    | IDM_EDIT_PROPERCASE_BLEND
                    | IDM_EDIT_SENTENCECASE_FORCE
                    | IDM_EDIT_SENTENCECASE_BLEND => {
                        use crate::case_conversion::{Mode, convert};
                        let mode = match command {
                            IDM_EDIT_UPPERCASE => Mode::Upper,
                            IDM_EDIT_LOWERCASE => Mode::Lower,
                            IDM_EDIT_INVERTCASE => Mode::Invert,
                            IDM_EDIT_RANDOMCASE => Mode::Random,
                            IDM_EDIT_PROPERCASE_FORCE => Mode::ProperForce,
                            IDM_EDIT_PROPERCASE_BLEND => Mode::ProperBlend,
                            IDM_EDIT_SENTENCECASE_FORCE => Mode::SentenceForce,
                            _ => Mode::SentenceBlend,
                        };
                        self.document_mut()
                            .transform_selection(|text| convert(text, mode))
                    }
                    IDM_EDIT_DUP_LINE => self.document_mut().duplicate_line(),
                    DELETE_LINE => self.document_mut().delete_line(),
                    TRANSPOSE_LINE => self.document_mut().transpose_line(),
                    DELETE_WORD_LEFT | DELETE_WORD_RIGHT => self
                        .document_mut()
                        .delete_to_word_start(command == DELETE_WORD_RIGHT),
                    DELETE_LINE_LEFT | DELETE_LINE_RIGHT => self
                        .document_mut()
                        .delete_to_line_boundary(command == DELETE_LINE_RIGHT),
                    IDM_EDIT_TRIMTRAILING => self.document_mut().trim_lines(TrimMode::Trailing),
                    IDM_EDIT_TRIMLINEHEAD => self.document_mut().trim_lines(TrimMode::Leading),
                    _ => self.document_mut().trim_lines(TrimMode::Both),
                };
                if let Err(e) = result {
                    self.report(e);
                }
                self.refresh();
            }
            IDM_SEARCH_FINDINFILES => {
                if let Err(error) = self.show_find_files() {
                    self.report(error);
                }
            }
            IDM_FOCUS_ON_FOUND_RESULTS => {
                if let Some(search) = &self.find_files {
                    search.show_results(true);
                    resize_controls(self.window);
                    unsafe {
                        SetFocus(search.list);
                    }
                }
            }
            IDM_SEARCH_GOTONEXTFOUND | IDM_SEARCH_GOTOPREVFOUND => {
                if let Some(index) = self
                    .find_files
                    .as_ref()
                    .and_then(|search| search.adjacent(command == IDM_SEARCH_GOTONEXTFOUND))
                {
                    if let Err(error) = self.open_find_result(index) {
                        self.report(error);
                    }
                }
            }
            IDM_SEARCH_FIND => self.show_search(false),
            IDM_SEARCH_REPLACE => self.show_search(true),
            IDM_SEARCH_FINDNEXT => self.repeat_find(SearchDirection::Forward),
            IDM_SEARCH_FINDPREV => self.repeat_find(SearchDirection::Backward),
            IDM_SEARCH_TOGGLE_BOOKMARK
            | IDM_SEARCH_NEXT_BOOKMARK
            | IDM_SEARCH_PREV_BOOKMARK
            | IDM_SEARCH_INVERSEMARKS
            | CLEAR_BOOKMARKS_IN_SELECTION
            | IDM_SEARCH_CLEAR_BOOKMARKS => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                match command {
                    IDM_SEARCH_TOGGLE_BOOKMARK => {
                        let line = self
                            .document()
                            .line_starts()
                            .partition_point(|&p| p <= selection.caret)
                            - 1;
                        let _ = self.document_mut().toggle_bookmark(line);
                    }
                    IDM_SEARCH_CLEAR_BOOKMARKS => self.document_mut().clear_bookmarks(),
                    CLEAR_BOOKMARKS_IN_SELECTION => {
                        let _ = self.document_mut().clear_bookmarks_in(selection.range());
                    }
                    IDM_SEARCH_INVERSEMARKS => self.document_mut().invert_bookmarks(),
                    _ => {
                        let _ = self
                            .document_mut()
                            .goto_bookmark(command == IDM_SEARCH_NEXT_BOOKMARK);
                    }
                }
                self.refresh_selection();
            }
            IDM_SEARCH_SETANDFINDNEXT => self.set_and_find(SearchDirection::Forward),
            IDM_SEARCH_SETANDFINDPREV => self.set_and_find(SearchDirection::Backward),
            IDM_SEARCH_VOLATILE_FINDNEXT => self.find_selected(SearchDirection::Forward),
            IDM_SEARCH_VOLATILE_FINDPREV => self.find_selected(SearchDirection::Backward),
            IDM_FILE_NEW => {
                self.add_new_document();
                self.refresh();
            }
            IDM_FILE_OPEN => match file_dialog_paths(self.window, false, None, false, true) {
                Ok(Some(paths)) => {
                    let mut failures = Vec::new();
                    for path in paths {
                        if let Err(error) = self.open(&path) {
                            failures.push(format!("{}: {error}", path.display()));
                        }
                    }
                    if !failures.is_empty() {
                        self.report(failures.join("\n"));
                    }
                }
                Ok(None) => (),
                Err(e) => self.report(e),
            },
            IDM_FILE_SAVE => {
                self.save(false);
            }
            IDM_FILE_SAVEAS => {
                self.save(true);
            }
            IDM_FILE_LOADSESSION => {
                match file_dialog_options(self.window, false, Some("Load Session"), true) {
                    Ok(Some(path)) => {
                        if let Err(error) = self.load_session_from(&path) {
                            self.report(error);
                        }
                    }
                    Ok(None) => (),
                    Err(error) => self.report(error),
                }
            }
            IDM_FILE_SAVESESSION => {
                match file_dialog_options(self.window, true, Some("Save Session"), true) {
                    Ok(Some(path)) => {
                        if let Err(error) = self.save_session_to(&path, atomic_write) {
                            self.report(error);
                        }
                    }
                    Ok(None) => (),
                    Err(error) => self.report(error),
                }
            }
            IDM_FILE_SAVECOPYAS => {
                self.save_mode(true, true);
            }
            DATE_FIRST => {
                self.date_first = !self.date_first;
                if let Err(error) = crate::preferences::save(&self.current_preferences()) {
                    self.report(error);
                }
                // SAFETY: Scalar menu state update for the owned main menu.
                unsafe {
                    CheckMenuItem(
                        crate::plugin_host::main_menu(self.window),
                        DATE_FIRST,
                        MF_BYCOMMAND
                            | if self.date_first {
                                MF_CHECKED
                            } else {
                                MF_UNCHECKED
                            },
                    );
                }
            }
            IDM_EDIT_INSERT_DATETIME_CUSTOMIZED => {
                let text = crate::date_time::current_custom(&self.date_format);
                if let Err(error) = self.insert_date_time_text(&text) {
                    self.report(error);
                }
            }
            IDM_EDIT_INSERT_DATETIME_SHORT | IDM_EDIT_INSERT_DATETIME_LONG => {
                match crate::date_time::current(
                    command == IDM_EDIT_INSERT_DATETIME_LONG,
                    self.date_first,
                ) {
                    Ok(text) => {
                        if let Err(error) = self.insert_date_time_text(&text) {
                            self.report(error);
                        }
                    }
                    Err(error) => self.report(error),
                }
            }
            IDM_FILE_CLOSE if self.confirm_close() => {
                let _ = self.workspace.close(self.active_id(), true);
                if self.workspace.active().is_none() {
                    self.add_new_document();
                }
                self.refresh();
            }
            IDM_FILE_EXIT => enqueue(Event::Close),
            IDM_EDIT_UNDO | IDM_EDIT_REDO => {
                let result = if command == IDM_EDIT_UNDO {
                    self.document_mut().undo()
                } else {
                    self.document_mut().redo()
                };
                if let Err(e) = result {
                    self.report(e);
                }
                self.refresh();
            }
            IDM_SEARCH_COPYMARKEDLINES => {
                if let Err(error) = self
                    .copy_bookmarked_lines(|text| crate::clipboard::write_text(self.window, text))
                {
                    self.report(error);
                }
            }
            IDM_SEARCH_PASTEMARKEDLINES => {
                let window = self.window;
                if let Err(error) =
                    self.paste_bookmarked_lines(|| crate::clipboard::read_text(window))
                {
                    self.report(error);
                }
            }
            IDM_SEARCH_CUTMARKEDLINES => {
                let window = self.window;
                if let Err(error) =
                    self.cut_bookmarked_lines(|text| crate::clipboard::write_text(window, text))
                {
                    self.report(error);
                }
            }
            IDM_SEARCH_DELETEMARKEDLINES | IDM_SEARCH_DELETEUNMARKEDLINES => {
                let (start, end) = control_selection(self.edit);
                self.sync_native_selection(start, end);
                if let Err(error) = self
                    .document_mut()
                    .delete_bookmarked_lines(command == IDM_SEARCH_DELETEMARKEDLINES)
                {
                    self.report(error);
                }
                self.refresh();
            }
            CUT_LINES | COPY_LINES => {
                let window = self.window;
                if let Err(error) = self.line_clipboard(command == CUT_LINES, |text| {
                    crate::clipboard::write_text(window, text)
                }) {
                    self.report(error);
                }
            }
            IDM_EDIT_FULLPATHTOCLIP | IDM_EDIT_FILENAMETOCLIP | IDM_EDIT_CURRENTDIRTOCLIP => {
                use crate::path_clipboard::Part;
                let part = match command {
                    IDM_EDIT_FULLPATHTOCLIP => Part::Full,
                    IDM_EDIT_FILENAMETOCLIP => Part::Name,
                    _ => Part::Directory,
                };
                if let Err(error) = self.copy_file_path(part, |text| {
                    crate::clipboard::write_os_text(self.window, text)
                }) {
                    self.report(error);
                }
            }
            IDM_EDIT_COPY_ALL_NAMES | IDM_EDIT_COPY_ALL_PATHS => {
                let list = self.workspace.file_list(command == IDM_EDIT_COPY_ALL_PATHS);
                if !list.is_empty() {
                    if let Err(error) = crate::clipboard::write_os_text(self.window, &list) {
                        self.report(error);
                    }
                }
            }
            IDM_EDIT_SELECTALL => unsafe {
                SendMessageW(self.edit, EM_SETSEL, 0, -1);
            },
            IDM_EDIT_COPY_BINARY | IDM_EDIT_CUT_BINARY | IDM_EDIT_PASTE_BINARY => {
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let range = selection.range();
                let result = (|| -> io::Result<()> {
                    let replacement = if command == IDM_EDIT_PASTE_BINARY {
                        let Some(bytes) = crate::clipboard::read_binary(self.window)? else {
                            return Ok(());
                        };
                        String::from_utf8(bytes).map_err(|_| io::Error::other("Binary clipboard data is not valid UTF-8. The text document cannot yet represent arbitrary byte sequences."))?
                    } else {
                        if range.is_empty() {
                            return Ok(());
                        }
                        crate::clipboard::write_binary(
                            self.window,
                            self.document().text()[range.clone()].as_bytes(),
                        )?;
                        if command == IDM_EDIT_COPY_BINARY {
                            return Ok(());
                        }
                        String::new()
                    };
                    let before = self
                        .macro_recording
                        .then(|| self.document().text().to_owned());
                    self.document_mut()
                        .replace(range, &replacement)
                        .map_err(io::Error::other)?;
                    if let Some(before) = before {
                        if let Some(step) = crate::macros::difference(
                            &before,
                            self.document().text(),
                            selection.caret,
                        ) {
                            self.macro_steps.push(step);
                        }
                    }
                    self.refresh();
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_EDIT_CUT
            | IDM_EDIT_COPY
            | IDM_EDIT_PASTE
            | IDM_EDIT_PASTE_AS_HTML
            | IDM_EDIT_PASTE_AS_RTF => {
                use crate::scintilla::{ids::*, send};
                if command == IDM_EDIT_PASTE {
                    let result = (|| -> io::Result<bool> {
                        let column = crate::clipboard::is_column(self.window)?;
                        if !column && send(self.edit, SCI_GETSELECTIONS, 0, 0) <= 1 {
                            return Ok(false);
                        }
                        let Some(text) = crate::clipboard::read_text(self.window)? else {
                            return Ok(true);
                        };
                        let eol = match self.document().line_ending() {
                            LineEnding::CrLf => SC_EOL_CRLF,
                            LineEnding::Lf => SC_EOL_LF,
                            LineEnding::Cr => SC_EOL_CR,
                        };
                        let before = self
                            .macro_recording
                            .then(|| self.document().text().to_owned());
                        let caret = self.document().selection().caret;
                        send(self.edit, SCI_SETEOLMODE, eol as usize, 0);
                        UPDATING.set(true);
                        if column {
                            send(
                                self.edit,
                                SCI_REPLACERECTANGULAR,
                                text.len(),
                                text.as_ptr() as isize,
                            );
                        } else {
                            let text = self.document().line_ending().convert(&text);
                            let count = send(self.edit, SCI_GETSELECTIONS, 0, 0) as usize;
                            let mut selections: Vec<_> = (0..count)
                                .map(|index| {
                                    (
                                        send(self.edit, SCI_GETSELECTIONNSTART, index, 0) as usize,
                                        send(self.edit, SCI_GETSELECTIONNEND, index, 0) as usize,
                                        send(
                                            self.edit,
                                            SCI_GETSELECTIONNSTARTVIRTUALSPACE,
                                            index,
                                            0,
                                        ) as usize,
                                        index,
                                    )
                                })
                                .collect();
                            selections.sort_by_key(|range| std::cmp::Reverse(range.0));
                            for (start, end, padding, index) in selections {
                                let inserted = format!("{}{}", " ".repeat(padding), text);
                                send(self.edit, SCI_SETTARGETSTART, start, 0);
                                send(self.edit, SCI_SETTARGETEND, end, 0);
                                send(
                                    self.edit,
                                    SCI_REPLACETARGET,
                                    inserted.len(),
                                    inserted.as_ptr() as isize,
                                );
                                let caret = start + inserted.len();
                                send(self.edit, SCI_SETSELECTIONNCARET, index, caret as isize);
                                send(self.edit, SCI_SETSELECTIONNANCHOR, index, caret as isize);
                                send(self.edit, SCI_SETSELECTIONNCARETVIRTUALSPACE, index, 0);
                                send(self.edit, SCI_SETSELECTIONNANCHORVIRTUALSPACE, index, 0);
                            }
                        }
                        UPDATING.set(false);
                        let edited = crate::scintilla::text(self.edit);
                        let id = self.active_id();
                        if let Err(error) = self
                            .projection
                            .apply_exact(self.workspace.get_mut(id).unwrap(), &edited)
                        {
                            self.refresh();
                            return Err(io::Error::other(error));
                        }
                        self.projection = Projection::new(self.document().text());
                        let (start, end) = control_selection(self.edit);
                        self.sync_native_selection(start, end);
                        if let Some(before) = before {
                            if let Some(step) =
                                crate::macros::difference(&before, self.document().text(), caret)
                            {
                                self.macro_steps.push(step);
                            }
                        }
                        self.caption();
                        self.refresh_tabs();
                        Ok(true)
                    })();
                    match result {
                        Ok(true) => return,
                        Err(error) => {
                            self.report(error);
                            return;
                        }
                        Ok(false) => {}
                    }
                }
                let count = send(self.edit, SCI_GETSELECTIONS, 0, 0) as usize;
                let rectangular = send(self.edit, SCI_SELECTIONISRECTANGLE, 0, 0) != 0;
                if matches!(command, IDM_EDIT_COPY | IDM_EDIT_CUT) && (count > 1 || rectangular) {
                    let result = (|| -> io::Result<()> {
                        if send(self.edit, SCI_GETSELECTIONEMPTY, 0, 0) != 0 {
                            return Ok(());
                        }
                        let length = send(self.edit, SCI_GETSELTEXT, 0, 0) as usize;
                        let mut bytes = vec![0; length + 1];
                        send(self.edit, SCI_GETSELTEXT, 0, bytes.as_mut_ptr() as isize);
                        bytes.truncate(length);
                        let text = String::from_utf8(bytes).map_err(io::Error::other)?;
                        // Bundled Scintilla marks every multi-range copy as
                        // column-shaped, including disjoint stream selections.
                        if count > 1 {
                            crate::clipboard::write_column(self.window, &text)?;
                        } else {
                            crate::clipboard::write_text(self.window, &text)?;
                        }
                        if command == IDM_EDIT_CUT {
                            let mut ranges: Vec<_> = (0..count)
                                .map(|index| {
                                    let a =
                                        send(self.edit, SCI_GETSELECTIONNSTART, index, 0) as usize;
                                    let b =
                                        send(self.edit, SCI_GETSELECTIONNEND, index, 0) as usize;
                                    a..b
                                })
                                .filter(|range| !range.is_empty())
                                .collect();
                            ranges.sort_by_key(|range| range.start);
                            if let (Some(first), Some(last)) = (ranges.first(), ranges.last()) {
                                let span = first.start..last.end;
                                let mut replacement = String::new();
                                let mut cursor = span.start;
                                for range in &ranges {
                                    if range.start > cursor {
                                        replacement
                                            .push_str(&self.document().text()[cursor..range.start]);
                                    }
                                    cursor = cursor.max(range.end);
                                }
                                let before = self
                                    .macro_recording
                                    .then(|| self.document().text().to_owned());
                                let caret = self.document().selection().caret;
                                self.document_mut()
                                    .replace(span, &replacement)
                                    .map_err(io::Error::other)?;
                                if let Some(before) = before {
                                    if let Some(step) = crate::macros::difference(
                                        &before,
                                        self.document().text(),
                                        caret,
                                    ) {
                                        self.macro_steps.push(step);
                                    }
                                }
                            }
                            send(self.edit, SCI_CLEAR, 0, 0);
                            self.projection = Projection::new(self.document().text());
                            self.caption();
                            self.refresh_tabs();
                        }
                        Ok(())
                    })();
                    if let Err(error) = result {
                        self.report(error);
                    }
                    return;
                }
                let (start, end) = control_selection(self.edit);
                let selection = self.sync_native_selection(start, end);
                let mut range = selection.range();
                let result = (|| -> io::Result<()> {
                    if command == IDM_EDIT_COPY || command == IDM_EDIT_CUT {
                        if range.is_empty() {
                            if !self.copy_cut_line {
                                return Ok(());
                            }
                            range = self.document().selected_line_range();
                            let line = self.document().text()[range.clone()]
                                .trim_end_matches(['\r', '\n']);
                            let copied =
                                format!("{}{}", line, self.document().line_ending().as_str());
                            crate::clipboard::write_line(self.window, &copied)?;
                        } else {
                            crate::clipboard::write_text(
                                self.window,
                                &self.document().text()[range.clone()],
                            )?;
                        }
                        if command == IDM_EDIT_COPY {
                            return Ok(());
                        }
                    }
                    let replacement = if command == IDM_EDIT_CUT {
                        String::new()
                    } else if command == IDM_EDIT_PASTE {
                        let Some(text) = crate::clipboard::read_text(self.window)? else {
                            return Ok(());
                        };
                        if range.is_empty() && crate::clipboard::is_line() {
                            let start = self.document().selected_line_range().start;
                            range = start..start;
                        }
                        self.document().line_ending().convert(&text)
                    } else {
                        let format = if command == IDM_EDIT_PASTE_AS_HTML {
                            "HTML Format"
                        } else {
                            "Rich Text Format"
                        };
                        let Some(bytes) = crate::clipboard::read_registered(self.window, format)?
                        else {
                            return Ok(());
                        };
                        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                        String::from_utf8(bytes[..end].to_vec()).map_err(io::Error::other)?
                    };
                    let before = self
                        .macro_recording
                        .then(|| self.document().text().to_owned());
                    self.document_mut()
                        .replace(range, &replacement)
                        .map_err(io::Error::other)?;
                    if let Some(before) = before {
                        if let Some(step) = crate::macros::difference(
                            &before,
                            self.document().text(),
                            selection.caret,
                        ) {
                            self.macro_steps.push(step);
                        }
                    }
                    self.refresh();
                    Ok(())
                })();
                if let Err(error) = result {
                    self.report(error);
                }
            }
            IDM_FORMAT_TODOS | IDM_FORMAT_TOUNIX | IDM_FORMAT_TOMAC => {
                let ending = match command {
                    IDM_FORMAT_TODOS => LineEnding::CrLf,
                    IDM_FORMAT_TOUNIX => LineEnding::Lf,
                    _ => LineEnding::Cr,
                };
                if let Err(e) = self.document_mut().convert_line_endings(ending) {
                    self.report(e);
                }
                self.refresh();
            }
            IDM_FORMAT_CONV2_AS_UTF_8
            | IDM_FORMAT_CONV2_UTF_8
            | IDM_FORMAT_CONV2_UTF_16LE
            | IDM_FORMAT_CONV2_UTF_16BE => {
                let encoding = match command {
                    IDM_FORMAT_CONV2_AS_UTF_8 => Encoding::Utf8,
                    IDM_FORMAT_CONV2_UTF_8 => Encoding::Utf8Bom,
                    IDM_FORMAT_CONV2_UTF_16LE => Encoding::Utf16Le,
                    _ => Encoding::Utf16Be,
                };
                if let Err(e) = self.document_mut().set_encoding(encoding) {
                    self.report(e);
                }
                self.caption();
                self.refresh_tabs();
            }
            _ => {
                if let Ok(command) = u16::try_from(command) {
                    // SAFETY: Only host-compatible plugins may be registered in
                    // this private registry. UI callbacks queue events rather
                    // than taking a reentrant mutable reference to App.
                    unsafe {
                        self.plugins.dispatch(command);
                    }
                }
            }
        }
    }
    fn drain(&mut self) -> bool {
        loop {
            let Some(event) = EVENTS.with_borrow_mut(|q| q.pop_front()) else {
                return true;
            };
            match event {
                Event::LiteralCharacter(ch) => {
                    if self.document().is_read_only() {
                        continue;
                    }
                    let (start, end) = control_selection(self.edit);
                    let selection = self.sync_native_selection(start, end);
                    let before = self
                        .macro_recording
                        .then(|| self.document().text().to_owned());
                    if let Err(error) = self
                        .document_mut()
                        .replace(selection.range(), &ch.to_string())
                    {
                        self.report(error);
                    } else if let Some(before) = before {
                        if let Some(step) = crate::macros::difference(
                            &before,
                            self.document().text(),
                            selection.caret,
                        ) {
                            self.macro_steps.push(step);
                        }
                    }
                    self.refresh();
                }
                Event::HorizontalWheel(delta, horizontal) => {
                    let mut characters = 0u32;
                    // SAFETY: SPI writes one UINT, and EM_LINESCROLL takes a
                    // signed character count encoded in the scalar WPARAM.
                    unsafe {
                        SystemParametersInfoW(
                            SPI_GETWHEELSCROLLCHARS,
                            0,
                            &mut characters as *mut _ as _,
                            0,
                        );
                        if characters != 0 {
                            let remainder = if horizontal {
                                &mut self.horizontal_wheel_remainder
                            } else {
                                &mut self.wheel_remainder
                            };
                            *remainder += delta;
                            let actions = *remainder / WHEEL_DELTA as i32;
                            *remainder %= WHEEL_DELTA as i32;
                            let direction = if horizontal { 1 } else { -1 };
                            let count = (i64::from(actions) * i64::from(characters) * direction)
                                .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                                as i32;
                            if count != 0 {
                                SendMessageW(self.edit, EM_LINESCROLL, count as isize as usize, 0);
                            }
                        }
                    }
                }
                Event::Wheel(delta, zoom) => {
                    let mut lines = 0u32;
                    // SAFETY: SPI writes a UINT into this live stack variable.
                    unsafe {
                        SystemParametersInfoW(
                            SPI_GETWHEELSCROLLLINES,
                            0,
                            &mut lines as *mut _ as _,
                            0,
                        );
                    }
                    if lines != 0 {
                        self.wheel_remainder += delta;
                        let actions = self.wheel_remainder / WHEEL_DELTA as i32;
                        self.wheel_remainder %= WHEEL_DELTA as i32;
                        // Upstream zooms once per qualifying message, regardless
                        // of the number of notches carried by that message.
                        if zoom && actions != 0 {
                            if let Err(error) = self.set_zoom(self.zoom + actions.signum()) {
                                self.report(error);
                            }
                        }
                    }
                }
                Event::CheckFileAttributes => {
                    if self.attribute_check.is_none() {
                        let paths = self
                            .workspace
                            .buffers()
                            .filter_map(|(id, doc)| {
                                doc.path()
                                    .map(|path| (id, path.to_owned(), doc.saved_fingerprint()))
                            })
                            .collect();
                        self.attribute_check =
                            Some(crate::file_attributes::start(self.window, paths));
                    }
                }
                Event::FileAttributesReady => {
                    if let Some(receiver) = &self.attribute_check {
                        match receiver.try_recv() {
                            Ok(states) => {
                                self.attribute_check = None;
                                self.apply_file_attributes(states);
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                self.attribute_check = None;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => (),
                        }
                    }
                }
                Event::FindFiles(query) => {
                    let (start, end) = control_selection(self.edit);
                    self.sync_native_selection(start, end);
                    self.search_text = query.needle.clone();
                    let open_files = self
                        .workspace
                        .buffers()
                        .filter(|_| !self.find_files_prefer_disk)
                        .filter_map(|(_, document)| {
                            document.path().map(|path| {
                                (
                                    path.to_owned(),
                                    (document.text().to_owned(), document.encoding()),
                                )
                            })
                        })
                        .collect();
                    if let Some(search) = &mut self.find_files {
                        if let Err(error) = search.start(query, open_files) {
                            self.report(error);
                        }
                    }
                    resize_controls(self.window);
                }
                Event::CancelFindFiles => {
                    if let Some(search) = &self.find_files {
                        search.stop();
                    }
                }
                Event::ClearFindFilesResults => {
                    if let Some(search) = &mut self.find_files {
                        search.clear();
                    }
                }
                Event::CopyFindFilesResults(all) => {
                    if let Some(search) = &self.find_files {
                        if let Err(error) = search.copy(all) {
                            self.report(error);
                        }
                    }
                }
                Event::PollFindFiles => {
                    if let Some(search) = &mut self.find_files {
                        search.poll();
                    }
                }
                Event::FindFilesDirectory => {
                    if let (Some(search), Some(directory)) = (
                        &self.find_files,
                        self.document().path().and_then(Path::parent),
                    ) {
                        search.set_directory(directory);
                    }
                }
                Event::OpenFindFileResult(index) => {
                    if let Err(error) = self.open_find_result(index) {
                        self.report(error);
                    }
                }
                Event::HideFindFilesResults => {
                    if let Some(search) = &self.find_files {
                        search.show_results(false);
                        resize_controls(self.window);
                    }
                }
                Event::LoadSession(path) => {
                    if let Err(error) = self.load_session_from(&path) {
                        self.report(error);
                    }
                }
                Event::OpenPaths(paths) => {
                    for path in paths {
                        if let Err(error) = self.open(&path) {
                            self.report(format!("{}: {error}", path.display()));
                        }
                    }
                }
                Event::Search(request) => self.search_request(request),
                Event::SearchMode(mode) => {
                    self.search_options.mode = mode;
                    if let Some(dialog) = &self.search_dialog {
                        dialog.set_mode(mode);
                    }
                    unsafe {
                        for (id, checked) in [
                            (SEARCH_REGEX, mode == SearchMode::Regex),
                            (SEARCH_EXTENDED, mode == SearchMode::Extended),
                        ] {
                            CheckMenuItem(
                                crate::plugin_host::main_menu(self.window),
                                id,
                                MF_BYCOMMAND | if checked { MF_CHECKED } else { MF_UNCHECKED },
                            );
                        }
                    }
                }
                Event::SearchTab(command, needle) => {
                    self.search_text = needle.clone();
                    self.command(command);
                    if command == IDM_SEARCH_FINDINFILES {
                        if let Some(dialog) = &self.find_files {
                            dialog.set_needle(&needle);
                        }
                    } else if let Some(dialog) = &self.search_dialog {
                        dialog.set_query_text(&needle);
                    }
                }
                Event::GoToRefresh => {
                    if let Some(dialog) = &self.goto_dialog {
                        dialog.update(self.document());
                    }
                }
                Event::GoTo(value, offset) => {
                    if offset {
                        self.document_mut().go_to_offset(value);
                    } else {
                        self.document_mut().go_to_line(value);
                    }
                    self.refresh_selection();
                    // SAFETY: Return keyboard focus to the owned editor after navigation.
                    unsafe {
                        SetFocus(self.edit);
                    }
                }
                Event::Command(command) => self.command(command),
                Event::Resize => self.resize(),
                Event::Selection(start, end) => {
                    self.sync_native_selection(start, end);
                    self.caption();
                }
                Event::BookmarkMargin(line) => {
                    if let Err(error) = self.document_mut().toggle_bookmark(line) {
                        self.report(error);
                    }
                    self.caption();
                }
                Event::RehideMarkedLines => {
                    // Run after the native edit/fold notification stack has unwound.
                    crate::scintilla::rehide_marked_lines(self.edit);
                }
                Event::NativeChanged => {
                    let (start, end) = control_selection(self.edit);
                    EVENTS.with_borrow_mut(|events| {
                        events.push_front(Event::Changed(
                            crate::scintilla::text(self.edit),
                            start,
                            end,
                        ))
                    });
                }
                Event::Changed(text, start, end) => {
                    let recording = self.macro_recording.then(|| {
                        (
                            self.document().text().to_owned(),
                            self.document().selection().caret,
                        )
                    });
                    let id = self.active_id();
                    if let Err(e) = self
                        .projection
                        .apply_exact(self.workspace.get_mut(id).unwrap(), &text)
                    {
                        self.report(e);
                        self.refresh();
                        continue;
                    }
                    self.projection = Projection::new(self.document().text());
                    if let Some((before, caret)) = recording {
                        if let Some(step) =
                            crate::macros::difference(&before, self.document().text(), caret)
                        {
                            self.macro_steps.push(step);
                        }
                    }
                    let selection = self.projection.selection(start, end);
                    let _ = self.document_mut().select(selection);
                    if self.projection.text != text {
                        self.refresh();
                        continue;
                    }
                    if self.content_tab_names {
                        self.workspace.name_from_first_line(self.active_id());
                    }
                    self.caption();
                    self.refresh_tabs();
                }
                Event::Tab(index) => {
                    let (start, end) = control_selection(self.edit);
                    self.sync_native_selection(start, end);
                    let id = self.workspace.buffers().nth(index).map(|(id, _)| id);
                    if let Some(id) = id {
                        self.workspace.activate(id);
                        self.refresh();
                        unsafe {
                            SetFocus(self.edit);
                        }
                    }
                }
                Event::DoubleClickTab(index) => {
                    if self.double_click_close {
                        enqueue(Event::CloseTab(index));
                    }
                }
                Event::CloseTab(index) => {
                    let id = self.workspace.buffers().nth(index).map(|(id, _)| id);
                    if let Some(id) = id {
                        self.close_documents(vec![id], |app| app.confirm_close());
                        unsafe {
                            SetFocus(self.edit);
                        }
                    }
                }
                Event::MoveTab(from, to) => {
                    if self.lock_tab_drag {
                        continue;
                    }
                    let id = self.workspace.buffers().nth(from).map(|(id, _)| id);
                    if let Some(id) = id {
                        self.workspace.activate(id);
                        for _ in 0..from.abs_diff(to) {
                            self.workspace.move_active(to > from);
                        }
                        self.refresh();
                        unsafe {
                            SetFocus(self.edit);
                        }
                    }
                }
                Event::Close => {
                    if self.close_documents(self.workspace.close_targets(CloseScope::All), |app| {
                        app.confirm_close()
                    }) {
                        return false;
                    }
                }
            }
        }
    }
    fn apply_file_attributes(&mut self, states: Vec<crate::file_attributes::AttributeState>) {
        self.launch_availability.borrow_mut().take();
        self.disk_states
            .retain(|id, _| self.workspace.get(*id).is_some());
        for state in states {
            if let Some(doc) = self.workspace.get_mut(state.id) {
                if doc.path() == Some(state.path.as_path()) {
                    if state.disk == crate::file_attributes::DiskState::Deleted {
                        doc.mark_disk_deleted();
                    }
                    if let Some(read_only) = state
                        .read_only
                        .filter(|_| state.disk != crate::file_attributes::DiskState::Deleted)
                    {
                        doc.set_file_read_only(read_only);
                    }
                    self.disk_states.insert(state.id, state.disk);
                }
            }
        }
        self.refresh_tabs();
        // Update protection without rebuilding text, selection, or scroll state.
        // SAFETY: The owning UI thread sends a scalar flag to its live control.
        unsafe {
            SendMessageW(
                self.edit,
                EM_SETREADONLY,
                self.document().is_read_only() as usize,
                0,
            );
        }
        self.caption();
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.attribute_check.take();
        self.search_dialog.take();
        self.find_files.take();
        // NPPN_SHUTDOWN from the supplied Notepad_plus_msgs.h. Send while host
        // windows are still valid, then unload DLLs before destroying controls.
        let plugins = std::mem::take(&mut self.plugins);
        let mut notification = ScNotification {
            header: NotifyHeader {
                hwnd_from: self.window,
                id_from: 0,
                code: 1009,
            },
            ..Default::default()
        };
        // SAFETY: Registered plugins must conform to this host's shutdown and
        // lifetime contract; notification has no borrowed text pointers.
        unsafe {
            plugins.notify_all(&mut notification);
        }
        drop(plugins);
        // A detached menu is not destroyed along with its former owner window.
        // Keep it through shutdown notifications, which may still query it.
        let detached_menu = unsafe { GetMenu(self.window).is_null() }
            .then(|| crate::plugin_host::main_menu(self.window));
        crate::plugin_host::remove(self.window);
        // SAFETY: No raw App pointer is stored; parent owns its controls and menu.
        unsafe {
            DestroyWindow(self.window);
            if let Some(menu) = detached_menu {
                DestroyMenu(menu);
            }
            if !self.zoom_font.is_null() {
                DeleteObject(self.zoom_font);
            }
        }
    }
}
fn file_dialog(owner: HWND, save: bool) -> io::Result<Option<PathBuf>> {
    file_dialog_titled(owner, save, None)
}
fn file_dialog_titled(owner: HWND, save: bool, title: Option<&str>) -> io::Result<Option<PathBuf>> {
    file_dialog_options(owner, save, title, false)
}
fn file_dialog_options(
    owner: HWND,
    save: bool,
    title: Option<&str>,
    session: bool,
) -> io::Result<Option<PathBuf>> {
    Ok(file_dialog_paths(owner, save, title, session, false)?
        .and_then(|paths| paths.into_iter().next()))
}
fn file_dialog_paths(
    owner: HWND,
    save: bool,
    title: Option<&str>,
    session: bool,
    multiple: bool,
) -> io::Result<Option<Vec<PathBuf>>> {
    let title = title.map(wide);
    let mut path = vec![0u16; if multiple { 1024 * 1024 } else { 32768 }];
    let mut dialog = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrTitle: title.as_ref().map_or(null(), |text| text.as_ptr()),
        lpstrFilter: if session {
            w!("Session files\0*.xml\0All files\0*.*\0")
        } else {
            w!("All files\0*.*\0Text files\0*.txt\0")
        },
        lpstrDefExt: if session { w!("xml") } else { null() },
        lpstrFile: path.as_mut_ptr(),
        nMaxFile: path.len() as u32,
        Flags: OFN_EXPLORER
            | OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if multiple && !save {
                OFN_ALLOWMULTISELECT
            } else {
                0
            }
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
        ..Default::default()
    };
    // SAFETY: Dialog buffers and owner outlive the synchronous dialog call.
    unsafe {
        let accepted = if save {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        };
        if accepted == 0 {
            let error = CommDlgExtendedError();
            return if error == 0 {
                Ok(None)
            } else {
                Err(io::Error::other(format!("File dialog error {error:#x}")))
            };
        }
    }
    // Explorer format: one full path, or directory followed by file names,
    // terminated by an empty string. Preserve the dialog's returned order.
    let mut parts = path
        .split(|&ch| ch == 0)
        .take_while(|part| !part.is_empty());
    let first = PathBuf::from(std::ffi::OsString::from_wide(
        parts.next().unwrap_or_default(),
    ));
    let names: Vec<_> = parts.map(std::ffi::OsString::from_wide).collect();
    Ok(Some(if names.is_empty() {
        vec![first]
    } else {
        names.into_iter().map(|name| first.join(name)).collect()
    }))
}
// Source menu grouping follows Notepad_plus.rc; only implemented commands appear.
enum MenuEntry {
    Command(u32, &'static str),
    Separator,
    Popup(&'static str, &'static [MenuEntry]),
}
unsafe fn append_menu_entries(menu: HMENU, entries: &[MenuEntry]) {
    // SAFETY: Submenu ownership transfers to the parent native menu.
    unsafe {
        for entry in entries {
            match entry {
                MenuEntry::Command(id, label) => {
                    AppendMenuW(menu, MF_STRING, *id as usize, wide(label).as_ptr());
                }
                MenuEntry::Separator => {
                    AppendMenuW(menu, MF_SEPARATOR, 0, null());
                }
                MenuEntry::Popup(label, children) => {
                    let popup = CreatePopupMenu();
                    append_menu_entries(popup, children);
                    AppendMenuW(menu, MF_POPUP, popup as usize, wide(label).as_ptr());
                }
            }
        }
    }
}
unsafe fn create_menu() -> HMENU {
    let entries = &[
        MenuEntry::Popup(
            "&File",
            &[
                MenuEntry::Command(IDM_FILE_NEW, "&New\tCtrl+N"),
                MenuEntry::Command(IDM_FILE_OPEN, "&Open...\tCtrl+O"),
                MenuEntry::Popup(
                    "Open with Encoding",
                    &[
                        MenuEntry::Command(OPEN_1252, "Open as Windows-1252…"),
                        MenuEntry::Command(OPEN_1251, "Open as Windows-1251…"),
                        MenuEntry::Command(OPEN_SHIFT_JIS, "Open as Shift-JIS…"),
                        MenuEntry::Command(OPEN_UTF16LE_NO_BOM, "Open as UTF-16 LE without BOM…"),
                        MenuEntry::Command(OPEN_UTF16BE_NO_BOM, "Open as UTF-16 BE without BOM…"),
                    ],
                ),
                MenuEntry::Popup(
                    "Open Containing &Folder",
                    &[
                        MenuEntry::Command(IDM_FILE_OPEN_FOLDER, "Explorer"),
                        MenuEntry::Command(IDM_FILE_OPEN_CMD, "cmd"),
                        MenuEntry::Command(IDM_FILE_OPEN_POWERSHELL, "PowerShell"),
                    ],
                ),
                MenuEntry::Command(IDM_FILE_OPEN_DEFAULT_VIEWER, "Open in &Default Viewer"),
                MenuEntry::Command(IDM_FILE_RELOAD, "Re&load from Disk"),
                MenuEntry::Command(IDM_FILE_SAVE, "&Save\tCtrl+S"),
                MenuEntry::Command(IDM_FILE_SAVEAS, "Save &As...\tCtrl+Shift+S"),
                MenuEntry::Command(IDM_FILE_SAVECOPYAS, "Save a Cop&y As..."),
                MenuEntry::Command(IDM_FILE_SAVEALL, "Sa&ve All"),
                MenuEntry::Command(IDM_FILE_RENAME, "Rename…"),
                MenuEntry::Command(IDM_FILE_DELETE, "Delete from Disk…"),
                MenuEntry::Command(IDM_FILE_CLOSE, "&Close\tCtrl+W"),
                MenuEntry::Command(IDM_FILE_CLOSEALL, "Clos&e All"),
                MenuEntry::Popup(
                    "Close &Multiple Documents",
                    &[
                        MenuEntry::Command(
                            IDM_FILE_CLOSEALL_BUT_CURRENT,
                            "Close All but Active Document",
                        ),
                        MenuEntry::Command(IDM_FILE_CLOSEALL_TOLEFT, "Close All to the Left"),
                        MenuEntry::Command(IDM_FILE_CLOSEALL_TORIGHT, "Close All to the Right"),
                        MenuEntry::Command(IDM_FILE_CLOSEALL_UNCHANGED, "Close All Unchanged"),
                    ],
                ),
                MenuEntry::Separator,
                MenuEntry::Popup(
                    "Recent Files",
                    &[
                        MenuEntry::Command(IDM_OPEN_ALL_RECENT_FILE, "Open All Recent Files"),
                        MenuEntry::Command(IDM_CLEAN_RECENT_FILE_LIST, "Clear Recent Files List"),
                    ],
                ),
                MenuEntry::Command(
                    IDM_FILE_RESTORELASTCLOSEDFILE,
                    "Restore Last Closed File\tCtrl+Shift+T",
                ),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_FILE_LOADSESSION, "Load Sess&ion..."),
                MenuEntry::Command(IDM_FILE_SAVESESSION, "Save Sess&ion..."),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_FILE_PRINT, "&Print…\tCtrl+P"),
                MenuEntry::Command(IDM_FILE_PRINTNOW, "Print Now"),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_FILE_EXIT, "E&xit"),
            ],
        ),
        MenuEntry::Popup(
            "&Edit",
            &[
                MenuEntry::Command(IDM_EDIT_UNDO, "&Undo\tCtrl+Z"),
                MenuEntry::Command(IDM_EDIT_REDO, "&Redo\tCtrl+Y"),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_EDIT_CUT, "Cu&t\tCtrl+X"),
                MenuEntry::Command(IDM_EDIT_COPY, "&Copy\tCtrl+C"),
                MenuEntry::Command(IDM_EDIT_PASTE, "&Paste\tCtrl+V"),
                MenuEntry::Popup(
                    "Paste Special",
                    &[
                        MenuEntry::Command(IDM_EDIT_COPY_BINARY, "Copy Binary Content"),
                        MenuEntry::Command(IDM_EDIT_CUT_BINARY, "Cut Binary Content"),
                        MenuEntry::Command(IDM_EDIT_PASTE_BINARY, "Paste Binary Content"),
                        MenuEntry::Command(IDM_EDIT_PASTE_AS_HTML, "Paste HTML Content"),
                        MenuEntry::Command(IDM_EDIT_PASTE_AS_RTF, "Paste RTF Content"),
                    ],
                ),
                MenuEntry::Popup(
                    "Multi-select All",
                    &[
                        MenuEntry::Command(IDM_EDIT_MULTISELECTALL, "Ignore Case && Whole Word"),
                        MenuEntry::Command(IDM_EDIT_MULTISELECTALLMATCHCASE, "Match Case Only"),
                        MenuEntry::Command(
                            IDM_EDIT_MULTISELECTALLWHOLEWORD,
                            "Match Whole Word Only",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_MULTISELECTALLMATCHCASEWHOLEWORD,
                            "Match Case && Whole Word",
                        ),
                    ],
                ),
                MenuEntry::Popup(
                    "Multi-select Next",
                    &[
                        MenuEntry::Command(IDM_EDIT_MULTISELECTNEXT, "Ignore Case && Whole Word"),
                        MenuEntry::Command(IDM_EDIT_MULTISELECTNEXTMATCHCASE, "Match Case Only"),
                        MenuEntry::Command(
                            IDM_EDIT_MULTISELECTNEXTWHOLEWORD,
                            "Match Whole Word Only",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_MULTISELECTNEXTMATCHCASEWHOLEWORD,
                            "Match Case && Whole Word",
                        ),
                    ],
                ),
                MenuEntry::Command(
                    IDM_EDIT_MULTISELECTUNDO,
                    "Undo the Latest Added Multi-Select",
                ),
                MenuEntry::Command(
                    IDM_EDIT_MULTISELECTSSKIP,
                    "Skip Current && Go to Next Multi-select",
                ),
                MenuEntry::Command(IDM_EDIT_COLUMNMODETIP, "Column Mode..."),
                MenuEntry::Command(IDM_EDIT_SELECTALL, "&Select All\tCtrl+A"),
                MenuEntry::Command(IDM_EDIT_DELETE, "Delete"),
                MenuEntry::Popup(
                    "Formatting",
                    &[
                        MenuEntry::Command(FORMAT_DOCUMENT, "Format Document\tCtrl+Shift+I"),
                        MenuEntry::Command(FORMAT_SELECTION, "Format Selection"),
                    ],
                ),
                MenuEntry::Popup(
                    "Auto-Completion",
                    &[
                        MenuEntry::Command(IDM_EDIT_AUTOCOMPLETE, "Function / Keyword Completion"),
                        MenuEntry::Command(
                            IDM_EDIT_AUTOCOMPLETE_CURRENTFILE,
                            "Word Completion\tCtrl+Space",
                        ),
                    ],
                ),
                MenuEntry::Popup(
                    "On Selection",
                    &[
                        MenuEntry::Command(IDM_EDIT_OPENSELECTEDFILETOEDIT, "Open File"),
                        MenuEntry::Command(IDM_EDIT_SEARCHONINTERNET, "Search on Internet"),
                        MenuEntry::Command(IDM_EDIT_CHANGESEARCHENGINE, "Change Search Engine..."),
                        MenuEntry::Command(
                            IDM_EDIT_OPENSELECTEDFILEFOLDERINEXPLORER,
                            "Open Containing Folder in Explorer",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_REDACT_SELECTION,
                            "Redact Selection █ (Shift: ●)",
                        ),
                    ],
                ),
                MenuEntry::Command(IDM_EDIT_BEGINENDSELECT, "Begin/End &Select\tCtrl+Shift+B"),
                MenuEntry::Command(
                    IDM_EDIT_BEGINENDSELECT_COLUMNMODE,
                    "Begin/End Select in Column Mode",
                ),
                MenuEntry::Separator,
                MenuEntry::Popup(
                    "Insert",
                    &[
                        MenuEntry::Command(IDM_EDIT_INSERT_DATETIME_SHORT, "Date Time (short)"),
                        MenuEntry::Command(IDM_EDIT_INSERT_DATETIME_LONG, "Date Time (long)"),
                        MenuEntry::Command(
                            IDM_EDIT_INSERT_DATETIME_CUSTOMIZED,
                            "Date Time (customized)",
                        ),
                        MenuEntry::Separator,
                        MenuEntry::Command(DATE_FIRST, "Date Before Time"),
                    ],
                ),
                MenuEntry::Popup(
                    "Cop&y to Clipboard",
                    &[
                        MenuEntry::Command(IDM_EDIT_FULLPATHTOCLIP, "Copy Current Full File path"),
                        MenuEntry::Command(IDM_EDIT_FILENAMETOCLIP, "Copy Current Filename"),
                        MenuEntry::Command(IDM_EDIT_CURRENTDIRTOCLIP, "Copy Current Dir. Path"),
                        MenuEntry::Separator,
                        MenuEntry::Command(IDM_EDIT_COPY_ALL_NAMES, "Copy All Filenames"),
                        MenuEntry::Command(IDM_EDIT_COPY_ALL_PATHS, "Copy All File Paths"),
                    ],
                ),
                MenuEntry::Popup(
                    "&Indent",
                    &[
                        MenuEntry::Command(IDM_EDIT_INS_TAB, "Increase Line Indent"),
                        MenuEntry::Command(IDM_EDIT_RMV_TAB, "Decrease Line Indent"),
                    ],
                ),
                MenuEntry::Popup(
                    "Comment / Uncomment",
                    &[
                        MenuEntry::Command(
                            IDM_EDIT_BLOCK_COMMENT,
                            "Toggle Single Line Comment\tCtrl+Q",
                        ),
                        MenuEntry::Command(IDM_EDIT_BLOCK_COMMENT_SET, "Single Line Comment"),
                        MenuEntry::Command(IDM_EDIT_BLOCK_UNCOMMENT, "Single Line Uncomment"),
                        MenuEntry::Command(IDM_EDIT_STREAM_COMMENT, "Block Comment"),
                        MenuEntry::Command(IDM_EDIT_STREAM_UNCOMMENT, "Block Uncomment"),
                    ],
                ),
                MenuEntry::Popup(
                    "Con&vert Case to",
                    &[
                        MenuEntry::Command(IDM_EDIT_UPPERCASE, "&UPPERCASE\tCtrl+Shift+U"),
                        MenuEntry::Command(IDM_EDIT_LOWERCASE, "&lowercase\tCtrl+U"),
                        MenuEntry::Command(IDM_EDIT_PROPERCASE_FORCE, "&Proper Case"),
                        MenuEntry::Command(IDM_EDIT_PROPERCASE_BLEND, "Proper Case (blend)"),
                        MenuEntry::Command(IDM_EDIT_SENTENCECASE_FORCE, "&Sentence case"),
                        MenuEntry::Command(IDM_EDIT_SENTENCECASE_BLEND, "Sentence case (blend)"),
                        MenuEntry::Command(IDM_EDIT_INVERTCASE, "&iNVERT cASE"),
                        MenuEntry::Command(IDM_EDIT_RANDOMCASE, "&ranDOm CasE"),
                    ],
                ),
                MenuEntry::Popup(
                    "&Line Operations",
                    &[
                        MenuEntry::Command(IDM_EDIT_DUP_LINE, "Duplicate Current Line\tCtrl+D"),
                        MenuEntry::Command(IDM_EDIT_REMOVE_ANY_DUP_LINES, "Remove Duplicate Lines"),
                        MenuEntry::Command(
                            IDM_EDIT_REMOVE_CONSECUTIVE_DUP_LINES,
                            "Remove Consecutive Duplicate Lines",
                        ),
                        MenuEntry::Command(IDM_EDIT_SPLIT_LINES, "Split Lines\tCtrl+I"),
                        MenuEntry::Command(IDM_EDIT_JOIN_LINES, "Join Lines\tCtrl+J"),
                        MenuEntry::Command(IDM_EDIT_LINE_UP, "Move Up Current Line\tCtrl+Shift+Up"),
                        MenuEntry::Command(
                            IDM_EDIT_LINE_DOWN,
                            "Move Down Current Line\tCtrl+Shift+Down",
                        ),
                        MenuEntry::Command(IDM_EDIT_REMOVEEMPTYLINES, "Remove Empty Lines"),
                        MenuEntry::Command(
                            IDM_EDIT_REMOVEEMPTYLINESWITHBLANK,
                            "Remove Empty Lines (Containing Blank characters)",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_BLANKLINEABOVECURRENT,
                            "Insert Blank Line Above Current\tCtrl+Alt+Enter",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_BLANKLINEBELOWCURRENT,
                            "Insert Blank Line Below Current\tCtrl+Alt+Shift+Enter",
                        ),
                        MenuEntry::Command(IDM_EDIT_SORTLINES_REVERSE_ORDER, "Reverse Line Order"),
                        MenuEntry::Command(IDM_EDIT_SORTLINES_RANDOMLY, "Randomize Line Order"),
                        MenuEntry::Separator,
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LEXICOGRAPHIC_ASCENDING,
                            "Sort Lines Lexicographically Ascending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_ASCENDING,
                            "Sort Lines Lex. Ascending Ignoring Case",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LOCALE_ASCENDING,
                            "Sort Lines In Locale Order Ascending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_INTEGER_ASCENDING,
                            "Sort Lines As Integers Ascending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_DECIMALCOMMA_ASCENDING,
                            "Sort Lines As Decimals (Comma) Ascending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_DECIMALDOT_ASCENDING,
                            "Sort Lines As Decimals (Dot) Ascending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LENGTH_ASCENDING,
                            "Sort Lines By Length Ascending",
                        ),
                        MenuEntry::Separator,
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LEXICOGRAPHIC_DESCENDING,
                            "Sort Lines Lexicographically Descending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_DESCENDING,
                            "Sort Lines Lex. Descending Ignoring Case",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LOCALE_DESCENDING,
                            "Sort Lines In Locale Order Descending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_INTEGER_DESCENDING,
                            "Sort Lines As Integers Descending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_DECIMALCOMMA_DESCENDING,
                            "Sort Lines As Decimals (Comma) Descending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_DECIMALDOT_DESCENDING,
                            "Sort Lines As Decimals (Dot) Descending",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SORTLINES_LENGTH_DESCENDING,
                            "Sort Lines By Length Descending",
                        ),
                        MenuEntry::Separator,
                        MenuEntry::Command(CUT_LINES, "Cut Lines\tCtrl+L"),
                        MenuEntry::Command(COPY_LINES, "Copy Lines\tCtrl+Shift+X"),
                        MenuEntry::Command(DELETE_LINE, "Delete Line\tCtrl+Shift+L"),
                        MenuEntry::Command(TRANSPOSE_LINE, "Transpose Line\tCtrl+T"),
                    ],
                ),
                MenuEntry::Popup(
                    "&EOL Conversion",
                    &[
                        MenuEntry::Command(IDM_FORMAT_TODOS, "Windows (CR LF)"),
                        MenuEntry::Command(IDM_FORMAT_TOUNIX, "Unix (LF)"),
                        MenuEntry::Command(IDM_FORMAT_TOMAC, "Macintosh (CR)"),
                    ],
                ),
                MenuEntry::Popup(
                    "&Blank Operations",
                    &[
                        MenuEntry::Command(IDM_EDIT_TRIMTRAILING, "Trim Trailing Space"),
                        MenuEntry::Command(IDM_EDIT_TRIMLINEHEAD, "Trim Leading Space"),
                        MenuEntry::Command(IDM_EDIT_TRIM_BOTH, "Trim Leading and Trailing Space"),
                        MenuEntry::Command(IDM_EDIT_EOL2WS, "EOL to Space"),
                        MenuEntry::Command(IDM_EDIT_TRIMALL, "Trim both and EOL to Space"),
                        MenuEntry::Separator,
                        MenuEntry::Command(IDM_EDIT_TAB2SW, "TAB to Space"),
                        MenuEntry::Command(IDM_EDIT_SW2TAB_ALL, "Space to TAB (All)"),
                        MenuEntry::Command(IDM_EDIT_SW2TAB_LEADING, "Space to TAB (Leading)"),
                    ],
                ),
                MenuEntry::Separator,
                MenuEntry::Popup(
                    "Read-&Only in Notepad++",
                    &[
                        MenuEntry::Command(
                            IDM_EDIT_TOGGLEREADONLY,
                            "Read-Only on Current Document",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_SETREADONLYFORALLDOCS,
                            "Read-Only for All Documents",
                        ),
                        MenuEntry::Command(
                            IDM_EDIT_CLEARREADONLYFORALLDOCS,
                            "Clear Read-Only for All Documents",
                        ),
                    ],
                ),
                MenuEntry::Command(
                    IDM_EDIT_TOGGLESYSTEMREADONLY,
                    "Read-Only Attribute in Windows",
                ),
                MenuEntry::Popup(
                    "Delete",
                    &[
                        MenuEntry::Command(DELETE_WORD_LEFT, "Delete Word Left\tCtrl+Backspace"),
                        MenuEntry::Command(DELETE_WORD_RIGHT, "Delete Word Right\tCtrl+Delete"),
                        MenuEntry::Command(
                            DELETE_LINE_LEFT,
                            "Delete to Start of Line\tCtrl+Shift+Backspace",
                        ),
                        MenuEntry::Command(
                            DELETE_LINE_RIGHT,
                            "Delete to End of Line\tCtrl+Shift+Delete",
                        ),
                    ],
                ),
            ],
        ),
        MenuEntry::Popup(
            "&Search",
            &[
                MenuEntry::Command(IDM_SEARCH_FIND, "&Find...\tCtrl+F"),
                MenuEntry::Command(IDM_SEARCH_FINDINFILES, "Find in Fi&les...\tCtrl+Shift+F"),
                MenuEntry::Command(IDM_SEARCH_FINDNEXT, "Find &Next\tF3"),
                MenuEntry::Command(IDM_SEARCH_FINDPREV, "Find &Previous\tShift+F3"),
                MenuEntry::Command(IDM_SEARCH_SETANDFINDNEXT, "&Select and Find Next\tCtrl+F3"),
                MenuEntry::Command(
                    IDM_SEARCH_SETANDFINDPREV,
                    "&Select and Find Previous\tCtrl+Shift+F3",
                ),
                MenuEntry::Command(
                    IDM_SEARCH_VOLATILE_FINDNEXT,
                    "Find (&Volatile) Next\tCtrl+Alt+F3",
                ),
                MenuEntry::Command(
                    IDM_SEARCH_VOLATILE_FINDPREV,
                    "Find (&Volatile) Previous\tCtrl+Alt+Shift+F3",
                ),
                MenuEntry::Command(IDM_SEARCH_REPLACE, "&Replace...\tCtrl+H"),
                MenuEntry::Command(IDM_SEARCH_GOTOLINE, "&Go to...\tCtrl+G"),
                MenuEntry::Command(IDM_FOCUS_ON_FOUND_RESULTS, "Search Results &Window"),
                MenuEntry::Command(IDM_SEARCH_GOTONEXTFOUND, "Next Search Resul&t\tF4"),
                MenuEntry::Command(
                    IDM_SEARCH_GOTOPREVFOUND,
                    "Previous Search Resul&t\tShift+F4",
                ),
                MenuEntry::Command(IDM_SEARCH_GOTOMATCHINGBRACE, "Go to &Matching Brace"),
                MenuEntry::Command(
                    IDM_SEARCH_SELECTMATCHINGBRACES,
                    "Select All In-betw&een {} [] or ()",
                ),
                MenuEntry::Separator,
                MenuEntry::Popup(
                    "&Bookmark",
                    &[
                        MenuEntry::Command(IDM_SEARCH_TOGGLE_BOOKMARK, "Toggle Bookmark\tCtrl+F2"),
                        MenuEntry::Command(IDM_SEARCH_NEXT_BOOKMARK, "Next Bookmark\tF2"),
                        MenuEntry::Command(IDM_SEARCH_PREV_BOOKMARK, "Previous Bookmark\tShift+F2"),
                        MenuEntry::Command(IDM_SEARCH_CLEAR_BOOKMARKS, "Clear All Bookmarks"),
                        MenuEntry::Command(IDM_SEARCH_CUTMARKEDLINES, "Cut Bookmarked Lines"),
                        MenuEntry::Command(IDM_SEARCH_COPYMARKEDLINES, "Copy Bookmarked Lines"),
                        MenuEntry::Command(
                            IDM_SEARCH_PASTEMARKEDLINES,
                            "Paste to (Replace) Bookmarked Lines",
                        ),
                        MenuEntry::Command(IDM_SEARCH_DELETEMARKEDLINES, "Remove Bookmarked Lines"),
                        MenuEntry::Command(
                            IDM_SEARCH_DELETEUNMARKEDLINES,
                            "Remove Non-Bookmarked Lines",
                        ),
                        MenuEntry::Command(IDM_SEARCH_INVERSEMARKS, "Inverse Bookmarks"),
                        MenuEntry::Separator,
                        MenuEntry::Command(
                            CLEAR_BOOKMARKS_IN_SELECTION,
                            "Clear Bookmarks in Selection",
                        ),
                        MenuEntry::Command(SEARCH_BOOKMARK_MATCHES, "Bookmark Matching Lines"),
                    ],
                ),
                MenuEntry::Popup(
                    "Search Options",
                    &[
                        MenuEntry::Command(SEARCH_EXTENDED, "Extended Search Mode"),
                        MenuEntry::Command(SEARCH_REGEX, "Regular Expression Search Mode"),
                        MenuEntry::Command(SEARCH_COUNT, "Count Occurrences"),
                        MenuEntry::Command(
                            SEARCH_IN_SELECTION,
                            "Count / Replace All / Bookmark in Selection",
                        ),
                        MenuEntry::Command(SEARCH_WRAP, "Wrap Around"),
                    ],
                ),
            ],
        ),
        MenuEntry::Popup(
            "&View",
            &[
                MenuEntry::Popup(
                    "View Current File in",
                    &[
                        MenuEntry::Command(IDM_VIEW_IN_FIREFOX, "Firefox"),
                        MenuEntry::Command(IDM_VIEW_IN_CHROME, "Chrome"),
                        MenuEntry::Command(IDM_VIEW_IN_EDGE, "Edge"),
                        MenuEntry::Command(IDM_VIEW_IN_IE, "Internet Explorer"),
                    ],
                ),
                MenuEntry::Command(IDM_VIEW_WRAP, "Word Wrap"),
                MenuEntry::Command(IDM_VIEW_HIDELINES, "Hide Lines\tAlt+H"),
                MenuEntry::Command(IDM_EDIT_RTL, "Text Direction RTL\tCtrl+Alt+R"),
                MenuEntry::Command(IDM_EDIT_LTR, "Text Direction LTR\tCtrl+Alt+L"),
                MenuEntry::Popup(
                    "Show Symbol",
                    &[
                        MenuEntry::Command(IDM_VIEW_NPC, "Show Non-Printing Characters"),
                        MenuEntry::Command(
                            IDM_VIEW_NPC_CCUNIEOL,
                            "Show Control Characters && Unicode EOL",
                        ),
                        MenuEntry::Command(IDM_VIEW_ALL_CHARACTERS, "Show All Characters"),
                        MenuEntry::Command(IDM_VIEW_TAB_SPACE, "Show Space and Tab"),
                        MenuEntry::Command(IDM_VIEW_EOL, "Show End of Line"),
                        MenuEntry::Command(IDM_VIEW_INDENT_GUIDE, "Show Indent Guide"),
                        MenuEntry::Command(IDM_VIEW_WRAP_SYMBOL, "Show Wrap Symbol"),
                    ],
                ),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_VIEW_FOLDALL, "Fold All\tAlt+0"),
                MenuEntry::Command(IDM_VIEW_UNFOLDALL, "Unfold All\tAlt+Shift+0"),
                MenuEntry::Command(IDM_VIEW_FOLD_CURRENT, "Fold Current Level"),
                MenuEntry::Command(IDM_VIEW_UNFOLD_CURRENT, "Unfold Current Level"),
                MenuEntry::Popup(
                    "Fold Level",
                    &[
                        MenuEntry::Command(IDM_VIEW_FOLD_1, "1"),
                        MenuEntry::Command(IDM_VIEW_FOLD_2, "2"),
                        MenuEntry::Command(IDM_VIEW_FOLD_3, "3"),
                        MenuEntry::Command(IDM_VIEW_FOLD_4, "4"),
                        MenuEntry::Command(IDM_VIEW_FOLD_5, "5"),
                        MenuEntry::Command(IDM_VIEW_FOLD_6, "6"),
                        MenuEntry::Command(IDM_VIEW_FOLD_7, "7"),
                        MenuEntry::Command(IDM_VIEW_FOLD_8, "8"),
                    ],
                ),
                MenuEntry::Popup(
                    "Unfold Level",
                    &[
                        MenuEntry::Command(IDM_VIEW_UNFOLD_1, "1"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_2, "2"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_3, "3"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_4, "4"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_5, "5"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_6, "6"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_7, "7"),
                        MenuEntry::Command(IDM_VIEW_UNFOLD_8, "8"),
                    ],
                ),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_VIEW_POSTIT, "Post-It\tF12"),
                MenuEntry::Command(IDM_VIEW_DISTRACTIONFREE, "Distraction Free Mode"),
                MenuEntry::Command(IDM_VIEW_ALWAYSONTOP, "Always on Top"),
                MenuEntry::Command(IDM_VIEW_FULLSCREENTOGGLE, "Toggle Full Screen Mode\tF11"),
                MenuEntry::Popup(
                    "&Zoom",
                    &[
                        MenuEntry::Command(
                            IDM_VIEW_ZOOMIN,
                            "Zoom &In (Ctrl+Mouse Wheel Up)\tCtrl+Num +",
                        ),
                        MenuEntry::Command(
                            IDM_VIEW_ZOOMOUT,
                            "Zoom &Out (Ctrl+Mouse Wheel Down)\tCtrl+Num -",
                        ),
                        MenuEntry::Command(
                            IDM_VIEW_ZOOMRESTORE,
                            "&Restore Default Zoom\tCtrl+Num /",
                        ),
                    ],
                ),
                MenuEntry::Popup(
                    "Ta&b",
                    &[
                        MenuEntry::Command(IDM_VIEW_TAB_NEXT, "Next Tab\tCtrl+Page Down"),
                        MenuEntry::Command(IDM_VIEW_TAB_PREV, "Previous Tab\tCtrl+Page Up"),
                        MenuEntry::Separator,
                        MenuEntry::Command(IDM_VIEW_TAB_MOVEFORWARD, "Move Tab Forward"),
                        MenuEntry::Command(IDM_VIEW_TAB_MOVEBACKWARD, "Move Tab Backward"),
                        MenuEntry::Command(IDM_VIEW_TAB_START, "Move Tab to Start"),
                        MenuEntry::Command(IDM_VIEW_TAB_END, "Move Tab to End"),
                    ],
                ),
                MenuEntry::Separator,
                MenuEntry::Command(IDM_VIEW_SUMMARY, "&Summary..."),
                MenuEntry::Popup(
                    "Interface",
                    &[
                        MenuEntry::Command(SHOW_TOOLBAR, "Show Toolbar"),
                        MenuEntry::Command(SHOW_LINE_NUMBERS, "Show Line Numbers"),
                        MenuEntry::Command(SHOW_STATUS_BAR, "Show Status Bar"),
                        MenuEntry::Command(SHOW_TAB_BAR, "Show Tab Bar"),
                        MenuEntry::Command(SHOW_MENU_BAR, "Show Menu Bar\tCtrl+Alt+M"),
                    ],
                ),
                MenuEntry::Popup(
                    "Indentation",
                    &[
                        MenuEntry::Command(TAB_WIDTH_2, "Tab Width: 2"),
                        MenuEntry::Command(TAB_WIDTH_4, "Tab Width: 4"),
                        MenuEntry::Command(TAB_WIDTH_8, "Tab Width: 8"),
                        MenuEntry::Command(INDENT_WITH_SPACES, "Replace Tab with Spaces"),
                        MenuEntry::Command(BASIC_AUTO_INDENT, "Language-aware Auto-Indent"),
                    ],
                ),
            ],
        ),
        MenuEntry::Popup(
            "E&ncoding",
            &[
                MenuEntry::Command(IDM_FORMAT_CONV2_AS_UTF_8, "Convert to UTF-8"),
                MenuEntry::Command(IDM_FORMAT_CONV2_UTF_8, "Convert to UTF-8-BOM"),
                MenuEntry::Command(IDM_FORMAT_CONV2_UTF_16BE, "Convert to UTF-16 BE BOM"),
                MenuEntry::Command(IDM_FORMAT_CONV2_UTF_16LE, "Convert to UTF-16 LE BOM"),
                MenuEntry::Popup(
                    "Convert to Character Set",
                    &[
                        MenuEntry::Command(CONVERT_1252, "Convert to Windows-1252"),
                        MenuEntry::Command(CONVERT_1251, "Convert to Windows-1251"),
                        MenuEntry::Command(CONVERT_SHIFT_JIS, "Convert to Shift-JIS"),
                        MenuEntry::Command(
                            CONVERT_UTF16LE_NO_BOM,
                            "Convert to UTF-16 LE without BOM",
                        ),
                        MenuEntry::Command(
                            CONVERT_UTF16BE_NO_BOM,
                            "Convert to UTF-16 BE without BOM",
                        ),
                    ],
                ),
            ],
        ),
    ];
    // SAFETY: The resulting native menu is owned by the main window.
    unsafe {
        let menu = CreateMenu();
        append_menu_entries(menu, entries);
        append_menu_entries(
            menu,
            &[
                MenuEntry::Popup(
                    "Se&ttings",
                    &[MenuEntry::Command(IDM_SETTING_PREFERENCE, "Preferences…")],
                ),
                MenuEntry::Popup(
                    "T&ools",
                    &[
                        MenuEntry::Popup(
                            "MD5",
                            &[
                                MenuEntry::Command(IDM_TOOL_MD5_GENERATE, "Generate…"),
                                MenuEntry::Command(
                                    IDM_TOOL_MD5_GENERATEFROMFILE,
                                    "Generate from file…",
                                ),
                                MenuEntry::Command(
                                    IDM_TOOL_MD5_GENERATEINTOCLIPBOARD,
                                    "Generate from selection into clipboard",
                                ),
                            ],
                        ),
                        MenuEntry::Popup(
                            "SHA-1",
                            &[
                                MenuEntry::Command(IDM_TOOL_SHA1_GENERATE, "Generate…"),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA1_GENERATEFROMFILE,
                                    "Generate from file…",
                                ),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA1_GENERATEINTOCLIPBOARD,
                                    "Generate from selection into clipboard",
                                ),
                            ],
                        ),
                        MenuEntry::Popup(
                            "SHA-256",
                            &[
                                MenuEntry::Command(IDM_TOOL_SHA256_GENERATE, "Generate…"),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA256_GENERATEFROMFILE,
                                    "Generate from file…",
                                ),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA256_GENERATEINTOCLIPBOARD,
                                    "Generate from selection into clipboard",
                                ),
                            ],
                        ),
                        MenuEntry::Popup(
                            "SHA-512",
                            &[
                                MenuEntry::Command(IDM_TOOL_SHA512_GENERATE, "Generate…"),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA512_GENERATEFROMFILE,
                                    "Generate from file…",
                                ),
                                MenuEntry::Command(
                                    IDM_TOOL_SHA512_GENERATEINTOCLIPBOARD,
                                    "Generate from selection into clipboard",
                                ),
                            ],
                        ),
                    ],
                ),
                MenuEntry::Popup(
                    "&Macro",
                    &[
                        MenuEntry::Command(IDM_MACRO_STARTRECORDINGMACRO, "Start Recording"),
                        MenuEntry::Command(IDM_MACRO_STOPRECORDINGMACRO, "Stop Recording"),
                        MenuEntry::Command(IDM_MACRO_PLAYBACKRECORDEDMACRO, "Playback"),
                        MenuEntry::Command(
                            IDM_MACRO_SAVECURRENTMACRO,
                            "Save Current Recorded Macro…",
                        ),
                        MenuEntry::Command(LOAD_MACRO, "Load Recorded Macro…"),
                        MenuEntry::Command(
                            IDM_MACRO_RUNMULTIMACRODLG,
                            "Run a Macro Multiple Times…",
                        ),
                    ],
                ),
                MenuEntry::Popup("&Run", &[MenuEntry::Command(IDM_EXECUTE, "Run…")]),
                MenuEntry::Popup(
                    "&Window",
                    &[
                        MenuEntry::Command(IDM_WINDOW_SORT_FN_ASC, "Name A to Z"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FN_DSC, "Name Z to A"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FP_ASC, "Path A to Z"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FP_DSC, "Path Z to A"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FT_ASC, "Type A to Z"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FT_DSC, "Type Z to A"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FS_ASC, "Content Length Ascending"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FS_DSC, "Content Length Descending"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FD_ASC, "Modified Time Ascending"),
                        MenuEntry::Command(IDM_WINDOW_SORT_FD_DSC, "Modified Time Descending"),
                    ],
                ),
                MenuEntry::Popup(
                    "&?",
                    &[
                        MenuEntry::Command(IDM_CMDLINEARGUMENTS, "Command Line Arguments…"),
                        MenuEntry::Separator,
                        MenuEntry::Command(IDM_HOMESWEETHOME, "Notepad++ Home"),
                        MenuEntry::Command(IDM_PROJECTPAGE, "Notepad++ Project Page"),
                        MenuEntry::Command(IDM_ONLINEDOCUMENT, "Notepad++ Online User Manual"),
                        MenuEntry::Command(IDM_FORUM, "Notepad++ Community (Forum)"),
                        MenuEntry::Separator,
                        MenuEntry::Command(IDM_DEBUGINFO, "Debug Info…"),
                        MenuEntry::Command(IDM_ABOUT, "About Notepad++ Rust"),
                    ],
                ),
            ],
        );
        let languages = CreatePopupMenu();
        AppendMenuW(
            languages,
            MF_STRING,
            IDM_LANG_TEXT as usize,
            w!("None (Normal Text)"),
        );
        AppendMenuW(languages, MF_SEPARATOR, 0, null());
        let mut groups = std::collections::BTreeMap::new();
        for language in crate::languages::LANGUAGES.iter().skip(1) {
            let initial = language.name.chars().next().unwrap().to_ascii_uppercase();
            let group = *groups.entry(initial).or_insert_with(|| {
                let popup = CreatePopupMenu();
                AppendMenuW(
                    languages,
                    MF_POPUP,
                    popup as usize,
                    wide(&initial.to_string()).as_ptr(),
                );
                popup
            });
            AppendMenuW(
                group,
                MF_STRING,
                language.command as usize,
                wide(&language.name).as_ptr(),
            );
        }
        crate::languages::append_user_menu(languages);
        InsertMenuW(
            menu,
            5,
            MF_BYPOSITION | MF_POPUP,
            languages as usize,
            w!("&Language"),
        );

        menu
    }
}

pub fn run() -> io::Result<()> {
    let mut app = App::create(true)?;
    for argument in std::env::args_os().skip(1) {
        if let Err(e) = app.open(Path::new(&argument)) {
            app.report(e);
        }
    }
    let keys = [
        (b' ', IDM_EDIT_AUTOCOMPLETE_CURRENTFILE),
        (b'P', IDM_FILE_PRINT),
        (b'Q', IDM_EDIT_BLOCK_COMMENT),
        (b'D', IDM_EDIT_DUP_LINE),
        (b'T', TRANSPOSE_LINE),
        (b'J', IDM_EDIT_JOIN_LINES),
        (b'I', IDM_EDIT_SPLIT_LINES),
        (b'U', IDM_EDIT_LOWERCASE),
        (b'F', IDM_SEARCH_FIND),
        (b'G', IDM_SEARCH_GOTOLINE),
        (b'L', CUT_LINES),
        (b'H', IDM_SEARCH_REPLACE),
        (b'N', IDM_FILE_NEW),
        (b'O', IDM_FILE_OPEN),
        (b'S', IDM_FILE_SAVE),
        (b'W', IDM_FILE_CLOSE),
        (b'C', IDM_EDIT_COPY),
        (b'X', IDM_EDIT_CUT),
        (b'V', IDM_EDIT_PASTE),
        (b'Z', IDM_EDIT_UNDO),
        (b'Y', IDM_EDIT_REDO),
        (b'A', IDM_EDIT_SELECTALL),
    ];
    let mut bindings: Vec<_> = keys
        .into_iter()
        .map(|(key, cmd)| ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: key as u16,
            cmd: cmd as u16,
        })
        .collect();
    for (modifiers, key, command) in [
        (FALT, b'H' as u16, IDM_VIEW_HIDELINES),
        (FCONTROL, VK_INSERT, IDM_EDIT_COPY),
        (FSHIFT, VK_INSERT, IDM_EDIT_PASTE),
        (FSHIFT, VK_DELETE, IDM_EDIT_CUT),
    ] {
        bindings.push(ACCEL {
            fVirt: FVIRTKEY | modifiers,
            key,
            cmd: command as u16,
        });
    }
    bindings.push(ACCEL {
        fVirt: FVIRTKEY,
        key: VK_F12,
        cmd: IDM_VIEW_POSTIT as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY,
        key: VK_F11,
        cmd: IDM_VIEW_FULLSCREENTOGGLE as u16,
    });
    for index in 0..9 {
        bindings.push(ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: b'1' as u16 + index,
            cmd: IDM_VIEW_TAB1 as u16 + index,
        });
    }
    for (key, command) in [(b'R', IDM_EDIT_RTL), (b'L', IDM_EDIT_LTR)] {
        bindings.push(ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FALT,
            key: key as u16,
            cmd: command as u16,
        });
    }
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL,
        key: VK_TAB,
        cmd: IDM_VIEW_TAB_NEXT as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FSHIFT,
        key: VK_TAB,
        cmd: IDM_VIEW_TAB_PREV as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FSHIFT,
        key: b'F' as u16,
        cmd: IDM_SEARCH_FINDINFILES as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY,
        key: VK_F4,
        cmd: IDM_SEARCH_GOTONEXTFOUND as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FSHIFT,
        key: VK_F4,
        cmd: IDM_SEARCH_GOTOPREVFOUND as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FSHIFT,
        key: b'S' as u16,
        cmd: IDM_FILE_SAVEAS as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FSHIFT,
        key: b'U' as u16,
        cmd: IDM_EDIT_UPPERCASE as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FSHIFT,
        key: b'B' as u16,
        cmd: IDM_EDIT_BEGINENDSELECT as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FCONTROL | FALT,
        key: b'M' as u16,
        cmd: SHOW_MENU_BAR as u16,
    });
    bindings.extend([
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: b'I' as u16,
            cmd: FORMAT_DOCUMENT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT,
            key: b'0' as u16,
            cmd: IDM_VIEW_FOLDALL as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FALT | FSHIFT,
            key: b'0' as u16,
            cmd: IDM_VIEW_UNFOLDALL as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_F2,
            cmd: IDM_SEARCH_TOGGLE_BOOKMARK as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_F2,
            cmd: IDM_SEARCH_NEXT_BOOKMARK as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FSHIFT,
            key: VK_F2,
            cmd: IDM_SEARCH_PREV_BOOKMARK as u16,
        },
    ]);
    bindings.push(ACCEL {
        fVirt: FVIRTKEY,
        key: VK_F3,
        cmd: IDM_SEARCH_FINDNEXT as u16,
    });
    bindings.push(ACCEL {
        fVirt: FVIRTKEY | FSHIFT,
        key: VK_F3,
        cmd: IDM_SEARCH_FINDPREV as u16,
    });
    bindings.extend([
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_F3,
            cmd: IDM_SEARCH_SETANDFINDNEXT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_F3,
            cmd: IDM_SEARCH_SETANDFINDPREV as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FALT,
            key: VK_F3,
            cmd: IDM_SEARCH_VOLATILE_FINDNEXT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FALT | FSHIFT,
            key: VK_F3,
            cmd: IDM_SEARCH_VOLATILE_FINDPREV as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: b'X' as u16,
            cmd: COPY_LINES as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_ADD,
            cmd: IDM_VIEW_ZOOMIN as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_SUBTRACT,
            cmd: IDM_VIEW_ZOOMOUT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_DIVIDE,
            cmd: IDM_VIEW_ZOOMRESTORE as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_UP,
            cmd: IDM_EDIT_LINE_UP as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_DOWN,
            cmd: IDM_EDIT_LINE_DOWN as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FALT,
            key: VK_RETURN,
            cmd: IDM_EDIT_BLANKLINEABOVECURRENT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FALT | FSHIFT,
            key: VK_RETURN,
            cmd: IDM_EDIT_BLANKLINEBELOWCURRENT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_BACK,
            cmd: DELETE_LINE_LEFT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_BACK,
            cmd: DELETE_WORD_LEFT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_LEFT,
            cmd: WORD_LEFT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_OEM_2,
            cmd: WORD_PART_LEFT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_OEM_5,
            cmd: WORD_PART_RIGHT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_OEM_2,
            cmd: WORD_PART_LEFT_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_OEM_5,
            cmd: WORD_PART_RIGHT_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_OEM_4,
            cmd: PARAGRAPH_UP as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_OEM_6,
            cmd: PARAGRAPH_DOWN as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_OEM_4,
            cmd: PARAGRAPH_UP_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_OEM_6,
            cmd: PARAGRAPH_DOWN_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_HOME,
            cmd: SMART_HOME as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FSHIFT,
            key: VK_HOME,
            cmd: SMART_HOME_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY,
            key: VK_END,
            cmd: LINE_END as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FSHIFT,
            key: VK_END,
            cmd: LINE_END_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_HOME,
            cmd: DOCUMENT_START as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_HOME,
            cmd: DOCUMENT_START_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_END,
            cmd: DOCUMENT_END as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_END,
            cmd: DOCUMENT_END_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_RIGHT,
            cmd: WORD_RIGHT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_LEFT,
            cmd: WORD_LEFT_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_RIGHT,
            cmd: WORD_RIGHT_EXTEND as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_DELETE,
            cmd: DELETE_WORD_RIGHT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: VK_DELETE,
            cmd: DELETE_LINE_RIGHT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: b'L' as u16,
            cmd: DELETE_LINE as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL | FSHIFT,
            key: b'T' as u16,
            cmd: IDM_FILE_RESTORELASTCLOSEDFILE as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_NEXT,
            cmd: IDM_VIEW_TAB_NEXT as u16,
        },
        ACCEL {
            fVirt: FVIRTKEY | FCONTROL,
            key: VK_PRIOR,
            cmd: IDM_VIEW_TAB_PREV as u16,
        },
    ]);
    // SAFETY: Windows copies accelerators; MSG is live throughout dispatch. All
    // model changes happen outside callbacks, avoiding reentrant mutable access.
    unsafe {
        let accelerators = CreateAcceleratorTableW(bindings.as_ptr(), bindings.len() as i32);
        if accelerators.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut message = MSG::default();
        let result = loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == -1 {
                break Err(io::Error::last_os_error());
            }
            if result == 0 {
                break Ok(());
            }
            let dialog_handled = app
                .search_dialog
                .as_ref()
                .is_some_and(|dialog| dialog.translate(&message));
            let dialog_handled = dialog_handled
                || app
                    .goto_dialog
                    .as_ref()
                    .is_some_and(|dialog| dialog.translate(&message));
            let dialog_handled = dialog_handled
                || app
                    .find_files
                    .as_ref()
                    .is_some_and(|dialog| dialog.translate(&message));
            let dialog_handled =
                dialog_handled || crate::plugin_host::translate_dialogs(app.window, &message);
            if !dialog_handled
                && TranslateAcceleratorW(app.window, accelerators, &message) == 0
                && !app.plugin_accelerators.translate(app.window, &message)
            {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if !app.drain() {
                break Ok(());
            }
        };
        DestroyAcceleratorTable(accelerators);
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_brace_commands_update_native_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀(x)").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 4,
                caret: 4,
            })
            .unwrap();
        app.refresh();
        app.command(IDM_SEARCH_SELECTMATCHINGBRACES);
        assert_eq!(control_selection(app.edit), (2, 5));
        app.command(IDM_SEARCH_GOTOMATCHINGBRACE);
        assert_eq!(control_selection(app.edit), (2, 2));
    }
    #[test]
    fn tab_commands_switch_visible_content_and_reorder_without_changing_identity() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let first = app.active_id();
        app.document_mut().insert("first").unwrap();
        let second = app.workspace.add(Document::new());
        app.document_mut().insert("second").unwrap();
        app.refresh();
        app.command(IDM_VIEW_TAB_NEXT);
        assert_eq!(app.active_id(), first);
        assert_eq!(control_text(app.edit), "first");
        app.command(IDM_VIEW_TAB_PREV);
        assert_eq!(app.active_id(), second);
        app.command(IDM_VIEW_TAB_MOVEBACKWARD);
        assert_eq!(app.active_id(), second);
        assert_eq!(
            app.workspace
                .buffers()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec![second, first]
        );
        assert_eq!(control_text(app.edit), "second");
        // SAFETY: Read selection from this test's native tab control.
        assert_eq!(unsafe { SendMessageW(app.tabs, TCM_GETCURSEL, 0, 0) }, 0);
    }
    #[test]
    fn batch_save_and_close_stop_on_cancel_and_preserve_remaining_tabs() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let a = app.active_id();
        app.document_mut().insert("a").unwrap();
        let b = app.workspace.add(Document::new());
        app.document_mut().insert("b").unwrap();
        let c = app.workspace.add(Document::new());
        app.refresh();
        let mut visited = Vec::new();
        assert!(!app.save_all_with(|app| {
            visited.push(app.active_id());
            app.active_id() != b
        }));
        assert_eq!(visited, vec![a, b]);
        assert_eq!(app.active_id(), b);
        app.workspace.activate(c);
        assert!(app.save_all_with(|_| true));
        assert_eq!(app.active_id(), c);
        let ids = app.workspace.close_targets(CloseScope::All);
        assert!(!app.close_documents(ids, |app| app.active_id() != b));
        assert!(app.workspace.get(a).is_none());
        assert_eq!(app.workspace.get(b).unwrap().text(), "b");
        assert!(app.workspace.get(c).is_some());
        assert_eq!(app.active_id(), b);
        let ids = app.workspace.close_targets(CloseScope::All);
        assert!(app.close_documents(ids, |_| true));
        assert_eq!(app.workspace.buffers().count(), 1);
        assert_eq!(app.document().text(), "");
    }
    #[test]
    fn plugin_modeless_dialog_registration_allows_reentrant_removal() {
        use npp_compat::messages::*;
        unsafe extern "system" fn unregister_on_command(
            hwnd: HWND,
            message: u32,
            wp: WPARAM,
            lp: LPARAM,
            _: usize,
            host: usize,
        ) -> LRESULT {
            // SAFETY: Subclass belongs to the live test dialog; host is its owner.
            unsafe {
                if message == WM_COMMAND {
                    assert_eq!(SendMessageW(host as HWND, NPPM_GETNBOPENFILES, 0, 0), 1);
                    assert_eq!(
                        SendMessageW(
                            host as HWND,
                            NPPM_MODELESSDIALOG,
                            MODELESSDIALOGREMOVE,
                            hwnd as isize
                        ),
                        0
                    );
                }
                DefSubclassProc(hwnd, message, wp, lp)
            }
        }
        let mut app = App::create(false).unwrap();
        app.drain();
        let dialog = crate::goto_dialog::GoToDialog::new(app.window).unwrap();
        let hwnd = dialog.handle();
        // SAFETY: All handles are owned on this thread and messages are scalar.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_MODELESSDIALOG,
                    MODELESSDIALOGADD,
                    hwnd as isize
                ),
                hwnd as isize
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_MODELESSDIALOG,
                    MODELESSDIALOGADD,
                    hwnd as isize
                ),
                0
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_MODELESSDIALOG, 99, hwnd as isize),
                1
            );
            assert_ne!(
                SetWindowSubclass(hwnd, Some(unregister_on_command), 91, app.window as usize),
                0
            );
        }
        // Publishing a document snapshot must retain plugin registrations.
        app.refresh();
        let message = MSG {
            hwnd: unsafe { GetDlgItem(hwnd, 204) },
            message: WM_KEYDOWN,
            wParam: VK_ESCAPE as usize,
            ..MSG::default()
        };
        assert!(crate::plugin_host::translate_dialogs(app.window, &message));
        assert!(!crate::plugin_host::translate_dialogs(app.window, &message));
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_MODELESSDIALOG,
                    MODELESSDIALOGREMOVE,
                    hwnd as isize
                ),
                hwnd as isize
            );
            assert_ne!(IsWindow(hwnd), 0);
            SendMessageW(
                app.window,
                NPPM_MODELESSDIALOG,
                MODELESSDIALOGADD,
                hwnd as isize,
            );
        }
        drop(dialog);
        assert!(!crate::plugin_host::translate_dialogs(app.window, &message));
    }

    #[test]
    fn plugin_session_queries_report_validity_and_both_view_file_order() {
        use npp_compat::messages::*;
        let path =
            std::env::temp_dir().join(format!("npp-plugin-session-{}.xml", std::process::id()));
        std::fs::write(&path, "<NotepadPlus><Session><mainView><File filename='C:\\missing\\日本語🦀.txt'/><File filename='second'/></mainView><subView><File filename='second'/></subView></Session></NotepadPlus>").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        let filename = wide(path.to_str().unwrap());
        let mut validity = [9i32, 0x12345678];
        let mut first = [0xccccu16; 128];
        let mut second = [0xccccu16; 128];
        let mut third = [0xccccu16; 128];
        let pointers = [first.as_mut_ptr(), second.as_mut_ptr(), third.as_mut_ptr()];
        // SAFETY: Each synchronous message receives readable terminated input
        // and writable output with capacity for every session filename and NUL.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    validity.as_mut_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                3
            );
            assert_eq!(validity, [1, 0x12345678]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    0,
                    filename.as_ptr() as isize
                ),
                3
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETSESSIONFILES,
                    pointers.as_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                1
            );
        }
        let expected = wide("C:\\missing\\日本語🦀.txt");
        assert_eq!(&first[..expected.len()], expected);
        assert_eq!(first[expected.len()], 0xcccc);
        assert_eq!(&second[..7], wide("second"));
        assert_eq!(&third[..7], wide("second"));
        assert_eq!(app.workspace.buffers().count(), 1);
        assert!(app.document().text().is_empty());
        first.fill(0xcccc);
        let invalid_pointers = [first.as_mut_ptr(), null_mut(), third.as_mut_ptr()];
        // SAFETY: The first output and input are valid; null outputs are rejected.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETSESSIONFILES,
                    invalid_pointers.as_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                0
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETSESSIONFILES,
                    0,
                    filename.as_ptr() as isize
                ),
                0
            );
        }
        assert_eq!(first, [0xcccc; 128]);
        std::fs::write(&path, b"<NotepadPlus><Session>").unwrap();
        // SAFETY: Same valid caller-owned input/output allocations as above.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    validity.as_mut_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                0
            );
            assert_eq!(validity, [0, 0x12345678]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETSESSIONFILES,
                    pointers.as_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                0
            );
        }
        std::fs::write(&path, b"<NotepadPlus><Session/></NotepadPlus>").unwrap();
        // SAFETY: Same valid caller-owned input/output allocations as above.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    validity.as_mut_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                0
            );
            assert_eq!(validity, [1, 0x12345678]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETSESSIONFILES,
                    pointers.as_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                1
            );
        }
        std::fs::remove_file(&path).unwrap();
        // SAFETY: Optional BOOL remains writable for null/missing input failures.
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    validity.as_mut_ptr() as usize,
                    filename.as_ptr() as isize
                ),
                0
            );
            assert_eq!(validity[0], 0);
            validity[0] = 9;
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETNBSESSIONFILES,
                    validity.as_mut_ptr() as usize,
                    0
                ),
                0
            );
            assert_eq!(validity[0], 0);
        }
    }

    #[test]
    fn plugin_open_file_enumeration_obeys_capacity_and_tab_order() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        let mut named = Document::new();
        let path = Path::new(r"C:\folder\résumé😀.txt");
        named.save_with(path, |_, _| Ok(())).unwrap();
        app.workspace.add(named);
        app.refresh();
        let mut first = [0xccccu16; 128];
        let mut second = [0xccccu16; 128];
        let mut untouched = [0xccccu16; 128];
        let pointers = [
            first.as_mut_ptr(),
            second.as_mut_ptr(),
            untouched.as_mut_ptr(),
        ];
        // SAFETY: All three array entries point to writable storage larger than
        // the fixture paths. Native messages are synchronous and retain nothing.
        unsafe {
            assert_eq!(
                SendMessageW(app.window, NPPM_GETOPENFILENAMES_DEPRECATED, 0, 3),
                0
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETOPENFILENAMES_DEPRECATED,
                    pointers.as_ptr() as usize,
                    0
                ),
                0
            );
            assert_eq!(first, [0xcccc; 128]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETOPENFILENAMES_DEPRECATED,
                    pointers.as_ptr() as usize,
                    1
                ),
                1
            );
            assert_eq!(&first[..6], wide("new 1"));
            assert_eq!(first[6], 0xcccc);
            assert_eq!(second, [0xcccc; 128]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETOPENFILENAMESPRIMARY_DEPRECATED,
                    pointers.as_ptr() as usize,
                    3
                ),
                2
            );
            let expected = wide(path.to_str().unwrap());
            assert_eq!(&second[..expected.len()], expected);
            assert_eq!(second[expected.len()], 0xcccc);
            assert_eq!(untouched, [0xcccc; 128]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETOPENFILENAMESSECOND_DEPRECATED,
                    pointers.as_ptr() as usize,
                    3
                ),
                0
            );
        }
        app.workspace.move_active(false);
        app.refresh();
        unsafe {
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETOPENFILENAMES_DEPRECATED,
                    pointers.as_ptr() as usize,
                    1
                ),
                1
            );
        }
        let expected = wide(path.to_str().unwrap());
        assert_eq!(&first[..expected.len()], expected);
        assert_eq!(app.document().path(), Some(path));
        assert!(!app.document().is_dirty());
    }

    #[test]
    fn date_time_replaces_selection_in_one_undo_and_respects_read_only() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a😀b").unwrap();
        app.refresh();
        // SAFETY: Select the supplementary character in the owned native edit.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 1, 3);
        }
        app.insert_date_time_text("13:04 12/10/2026").unwrap();
        assert_eq!(app.document().text(), "a13:04 12/10/2026b");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a😀b");
        assert_eq!(control_selection(app.edit), (1, 3));
        app.command(DATE_FIRST);
        assert!(app.date_first);
        unsafe {
            assert_ne!(
                GetMenuState(GetMenu(app.window), DATE_FIRST, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
        }
        for command in [
            IDM_EDIT_INSERT_DATETIME_SHORT,
            IDM_EDIT_INSERT_DATETIME_LONG,
            IDM_EDIT_INSERT_DATETIME_CUSTOMIZED,
        ] {
            app.command(command);
            assert!(app.document().text().starts_with('a'));
            assert!(app.document().text().ends_with('b'));
            assert!(!app.document().text().contains('😀'));
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), "a😀b");
            app.document_mut().set_read_only(true);
            app.command(command);
            assert_eq!(app.document().text(), "a😀b");
            app.document_mut().set_read_only(false);
        }
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn decimal_range_handling_agrees_with_source_windows_runtime() {
        use std::ffi::{c_char, c_void};
        unsafe extern "C" {
            fn _create_locale(category: i32, locale: *const c_char) -> *mut c_void;
            fn _free_locale(locale: *mut c_void);
            fn _wcstod_l(text: *const u16, end: *mut *mut u16, locale: *mut c_void) -> f64;
            fn _errno() -> *mut i32;
        }
        // SAFETY: The test owns the CRT locale and frees it after all comparisons.
        let locale = unsafe { _create_locale(4, c"en-US".as_ptr()) };
        assert!(!locale.is_null());
        let mut inputs = vec![
            "-".to_owned(),
            ".".to_owned(),
            "-0".to_owned(),
            "9".repeat(400),
            format!("0.{}1", "0".repeat(400)),
        ];
        for value in [
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(f64::MIN_POSITIVE.to_bits() - 1),
            f64::MIN_POSITIVE / 2.0,
            f64::from_bits(1),
            0.1,
            -0.1,
        ] {
            inputs.push(format!("{value:.400}"));
        }
        let mut outcomes = Vec::new();
        let mut numeric = Vec::new();
        for input in &inputs {
            let text = wide(input);
            let mut end = null_mut();
            // SAFETY: Live terminated input, valid end-pointer storage, and owned locale.
            let (failed, number) = unsafe {
                *_errno() = 0;
                let number = _wcstod_l(text.as_ptr(), &mut end, locale);
                (std::ptr::eq(text.as_ptr(), end) || *_errno() == 34, number)
            };
            if !failed {
                numeric.push((input.as_str(), number));
            }
            let mut doc = Document::new();
            doc.convert_line_endings(LineEnding::Lf).unwrap();
            doc.insert(&format!("0\n{input}")).unwrap();
            outcomes.push((
                input,
                failed,
                doc.order_lines(LineOrder::DecimalDotAscending).is_err(),
            ));
        }
        unsafe {
            _free_locale(locale);
        }
        for (input, expected, actual) in outcomes {
            assert_eq!(actual, expected, "range handling for {input}");
        }
        let original = numeric
            .iter()
            .map(|(text, _)| *text)
            .collect::<Vec<_>>()
            .join("\n");
        for (order, descending) in [
            (LineOrder::DecimalDotAscending, false),
            (LineOrder::DecimalDotDescending, true),
        ] {
            let mut expected = numeric.clone();
            expected.sort_by(|a, b| {
                let compared = a.1.partial_cmp(&b.1).unwrap();
                if descending {
                    compared.reverse()
                } else {
                    compared
                }
            });
            let expected = expected
                .iter()
                .map(|(text, _)| *text)
                .collect::<Vec<_>>()
                .join("\n");
            let mut doc = Document::new();
            doc.convert_line_endings(LineEnding::Lf).unwrap();
            doc.insert(&original).unwrap();
            doc.order_lines(order).unwrap();
            assert_eq!(doc.text(), expected);
        }
    }

    #[test]
    fn hidden_menu_is_temporarily_attached_only_for_main_menu_navigation() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        // SAFETY: Synchronous menu lifecycle messages target this test's own window.
        unsafe {
            let menu = GetMenu(app.window);
            SendMessageW(app.window, NPPM_HIDEMENU, 0, 1);
            app.refresh();
            for popup in [1, 0] {
                if popup == 0 {
                    crate::plugin_host::entering_system_menu(app.window);
                }
                SendMessageW(app.window, WM_ENTERMENULOOP, popup, 0);
                assert!(GetMenu(app.window).is_null());
                SendMessageW(app.window, WM_EXITMENULOOP, popup, 0);
                assert!(GetMenu(app.window).is_null());
            }
            assert_eq!(SendMessageW(app.window, WM_ENTERMENULOOP, 0, 0), 1);
            assert_eq!(GetMenu(app.window), menu);
            assert_eq!(SendMessageW(app.window, NPPM_ISMENUHIDDEN, 0, 0), 0);
            assert_eq!(
                GetMenuState(menu, SHOW_MENU_BAR, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
            // Source treats a show request during temporary display as a no-op.
            assert_eq!(SendMessageW(app.window, NPPM_HIDEMENU, 0, 0), 0);
            app.refresh();
            assert_eq!(SendMessageW(app.window, WM_EXITMENULOOP, 0, 0), 0);
            assert!(GetMenu(app.window).is_null());
            SendMessageW(app.window, NPPM_HIDEMENU, 0, 0);
            SendMessageW(app.window, WM_ENTERMENULOOP, 0, 0);
            SendMessageW(app.window, WM_EXITMENULOOP, 0, 0);
            assert_eq!(GetMenu(app.window), menu);
        }
    }

    #[test]
    fn hidden_menu_retains_plugin_access_and_releases_detached_menu_on_drop() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        // SAFETY: Test queries and toggles only the menu of its own live window.
        let menu = unsafe { GetMenu(app.window) };
        assert!(!menu.is_null());
        unsafe {
            assert_eq!(SendMessageW(app.window, NPPM_ISMENUHIDDEN, 0, 0), 0);
            assert_eq!(SendMessageW(app.window, NPPM_HIDEMENU, 0, 1), 0);
            assert!(GetMenu(app.window).is_null());
            assert_eq!(SendMessageW(app.window, NPPM_HIDEMENU, 0, 1), 1);
            assert_eq!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPMAINMENU, 0),
                menu as isize
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_SETMENUITEMCHECK, DATE_FIRST as usize, 1),
                1
            );
            assert_ne!(GetMenuState(menu, DATE_FIRST, MF_BYCOMMAND) & MF_CHECKED, 0);
            app.rebuild_plugin_menu().unwrap();
            assert!(GetMenu(app.window).is_null());
            assert_ne!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPPLUGINMENU, 0),
                0
            );
            app.document_mut().set_read_only(true);
            app.refresh();
            assert_ne!(
                GetMenuState(menu, IDM_EDIT_DUP_LINE, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
            // Only exactly TRUE hides the menu, unlike the tab-bar API.
            assert_eq!(SendMessageW(app.window, NPPM_HIDEMENU, 0, 2), 1);
            assert_eq!(GetMenu(app.window), menu);
            assert_ne!(
                GetMenuState(menu, SHOW_MENU_BAR, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
            app.command(SHOW_MENU_BAR);
            assert!(GetMenu(app.window).is_null());
            app.command(SHOW_MENU_BAR);
            assert_eq!(GetMenu(app.window), menu);
            app.command(SHOW_MENU_BAR);
            assert_ne!(IsMenu(menu), 0);
        }
        drop(app);
        // SAFETY: IsMenu accepts a former handle and does not dereference it.
        unsafe {
            assert_eq!(IsMenu(menu), 0);
        }
    }

    #[test]
    fn tab_visibility_returns_previous_hidden_state_and_composes_with_status_bar() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("first tab").unwrap();
        app.refresh();
        // SAFETY: The test owns all queried handles and output rectangles.
        unsafe {
            let mut original = RECT::default();
            GetClientRect(app.edit, &mut original);
            assert_eq!(SendMessageW(app.window, NPPM_ISTABBARHIDDEN, 0, 0), 0);
            assert_eq!(SendMessageW(app.window, NPPM_HIDETABBAR, 0, 2), 0);
            assert_eq!(SendMessageW(app.window, NPPM_ISTABBARHIDDEN, 0, 0), 1);
            assert_eq!(SendMessageW(app.window, NPPM_HIDETABBAR, 0, -1), 1);
            let mut hidden = RECT::default();
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, original.bottom + 28);
            SendMessageW(app.window, NPPM_HIDESTATUSBAR, 0, 1);
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, original.bottom + 52);
            app.command(IDM_FILE_NEW);
            app.command(IDM_VIEW_TAB_PREV);
            assert!(is_control_hidden(app.window, TAB_ID));
            assert_eq!(app.document().text(), "first tab");
            app.command(SHOW_TAB_BAR);
            assert_eq!(SendMessageW(app.window, NPPM_ISTABBARHIDDEN, 0, 0), 0);
            assert_ne!(
                GetMenuState(GetMenu(app.window), SHOW_TAB_BAR, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, original.bottom + 24);
            assert_eq!(SendMessageW(app.window, NPPM_HIDETABBAR, 0, 0), 0);
            app.command(SHOW_STATUS_BAR);
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, original.bottom);
        }
        assert!(app.document().is_dirty());
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn status_visibility_preserves_document_and_resizes_synchronously() {
        use npp_compat::messages::{NPPM_HIDESTATUSBAR, NPPM_ISSTATUSBARHIDDEN};
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("unchanged").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 2,
                caret: 5,
            })
            .unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        // SAFETY: All handles and rectangles belong to this hidden test window.
        unsafe {
            let mut shown = RECT::default();
            ShowWindow(app.window, SW_HIDE);
            GetClientRect(app.edit, &mut shown);
            assert_eq!(SendMessageW(app.window, NPPM_ISSTATUSBARHIDDEN, 0, 0), 0);
            assert_eq!(SendMessageW(app.window, NPPM_HIDESTATUSBAR, 0, 1), 1);
            assert_eq!(SendMessageW(app.window, NPPM_ISSTATUSBARHIDDEN, 0, 0), 1);
            let mut hidden = RECT::default();
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, shown.bottom + 24);
            assert_eq!(SendMessageW(app.window, NPPM_HIDESTATUSBAR, 0, 1), 0);
            // Only exactly TRUE hides; other values show, matching the source.
            assert_eq!(SendMessageW(app.window, NPPM_HIDESTATUSBAR, 0, 2), 0);
            assert_eq!(SendMessageW(app.window, NPPM_ISSTATUSBARHIDDEN, 0, 0), 0);
            app.command(SHOW_STATUS_BAR);
            assert!(is_status_hidden(app.window));
            assert_eq!(
                GetMenuState(GetMenu(app.window), SHOW_STATUS_BAR, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
            app.command(SHOW_STATUS_BAR);
            assert!(!is_status_hidden(app.window));
            GetClientRect(app.edit, &mut hidden);
            assert_eq!(hidden.bottom, shown.bottom);
        }
        assert_eq!(app.document().text(), "unchanged");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 2,
                caret: 5
            }
        );
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn status_reports_selection_characters_lines_and_unselected_byte_position() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("é🦀\r\nx\n").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: 8,
            })
            .unwrap();
        app.refresh();
        let status = control_text(app.status);
        assert!(status.contains("Sel: 4 | 1"), "{status}");
        assert!(status.contains("10 bytes, 3 lines"), "{status}");
        app.document_mut()
            .select(Selection {
                anchor: 8,
                caret: 8,
            })
            .unwrap();
        app.caption();
        let status = control_text(app.status);
        assert!(status.starts_with("Ln 2, Col 1"), "{status}");
        assert!(status.contains("Pos: 9"), "{status}");
        assert!(!status.contains("Sel:"));
    }

    #[test]
    fn begin_end_selection_tracks_edits_and_updates_menu_in_read_only_documents() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("0123456789").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 7,
                caret: 7,
            })
            .unwrap();
        app.refresh();
        app.command(IDM_EDIT_BEGINENDSELECT);
        assert!(app.selection_gesture.is_started());
        app.document_mut().replace(0..0, "abc").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 2,
                caret: 2,
            })
            .unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        // SAFETY: Read the menu state of the test's live window.
        unsafe {
            let state = GetMenuState(GetMenu(app.window), IDM_EDIT_BEGINENDSELECT, MF_BYCOMMAND);
            assert_ne!(state & MF_CHECKED, 0);
            assert_eq!(state & MF_GRAYED, 0);
        }
        app.command(IDM_EDIT_BEGINENDSELECT);
        assert!(!app.selection_gesture.is_started());
        assert_eq!(app.document().selection().range(), 2..10);
        assert_eq!(control_selection(app.edit), (2, 10));
        assert_eq!(app.document().text(), "abc0123456789");
        // SAFETY: Read the menu state of the test's live window.
        unsafe {
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_BEGINENDSELECT, MF_BYCOMMAND)
                    & MF_CHECKED,
                0
            );
        }
    }

    #[test]
    fn upper_and_lower_commands_use_native_mapping_and_restore_selection_on_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        for (command, original, expected) in [
            (IDM_EDIT_UPPERCASE, "[ıßﬃé🦀𐐨]", "[ıßﬃÉ🦀𐐨]"),
            (IDM_EDIT_LOWERCASE, "[AİÉΟΣ🦀𐐀]", "[aİéοσ🦀𐐀]"),
        ] {
            app.command(IDM_FILE_NEW);
            app.document_mut().insert(original).unwrap();
            app.document_mut()
                .select(Selection {
                    anchor: 1,
                    caret: original.len() - 1,
                })
                .unwrap();
            app.refresh();
            app.document_mut().set_read_only(true);
            app.command(command);
            assert_eq!(app.document().text(), original);
            app.document_mut().set_read_only(false);
            app.command(command);
            assert_eq!(app.document().text(), expected);
            assert_eq!(app.document().selection().range(), 1..expected.len() - 1);
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), original);
            assert_eq!(app.document().selection().range(), 1..original.len() - 1);
            app.document_mut().select(Selection::default()).unwrap();
            app.refresh();
            app.command(command);
            assert_eq!(app.document().text(), original);
        }
    }

    #[test]
    fn random_and_invert_commands_use_native_case_mapping() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let original = "aBéÉß🦀𐐀";
        app.document_mut().insert(original).unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: original.len(),
            })
            .unwrap();
        app.refresh();
        app.command(IDM_EDIT_INVERTCASE);
        assert_eq!(app.document().text(), "AbÉéß🦀𐐀");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), original);
        app.document_mut().set_read_only(true);
        app.command(IDM_EDIT_RANDOMCASE);
        assert_eq!(app.document().text(), original);
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_RANDOMCASE);
        let randomized = app.document().text().to_owned();
        assert_eq!(
            randomized.encode_utf16().count(),
            original.encode_utf16().count()
        );
        assert_eq!(randomized.to_lowercase(), original.to_lowercase());
        assert!(randomized.ends_with("ß🦀𐐀"));
        if randomized != original {
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), original);
        }
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(IDM_EDIT_RANDOMCASE);
        assert_eq!(app.document().text(), original);
    }

    #[test]
    fn proper_and_sentence_commands_preserve_history_and_read_only_guards() {
        let mut app = App::create(false).unwrap();
        app.drain();
        for (command, expected) in [
            (IDM_EDIT_PROPERCASE_FORCE, "Hello World. Next"),
            (IDM_EDIT_PROPERCASE_BLEND, "HELLO WORLD. NEXT"),
            (IDM_EDIT_SENTENCECASE_FORCE, "Hello world. Next"),
            (IDM_EDIT_SENTENCECASE_BLEND, "HELLO wORLD. NEXT"),
        ] {
            app.command(IDM_FILE_NEW);
            let original = "hELLO wORLD. nEXT";
            app.document_mut().insert(original).unwrap();
            app.document_mut()
                .select(Selection {
                    anchor: original.len(),
                    caret: 0,
                })
                .unwrap();
            app.refresh();
            app.document_mut().set_read_only(true);
            app.command(command);
            assert_eq!(app.document().text(), original);
            app.document_mut().set_read_only(false);
            app.command(command);
            assert_eq!(app.document().text(), expected);
            assert_eq!(
                app.document().selection(),
                Selection {
                    anchor: 0,
                    caret: expected.len()
                }
            );
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), original);
            assert_eq!(
                app.document().selection(),
                Selection {
                    anchor: original.len(),
                    caret: 0
                }
            );
        }
    }

    #[test]
    fn random_sort_command_preserves_lines_and_respects_read_only() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut()
            .convert_line_endings(LineEnding::Lf)
            .unwrap();
        let original = "one\ntwo\nthree\nfour\nfive\nsix\n";
        app.document_mut().insert(original).unwrap();
        app.refresh();
        app.command(IDM_EDIT_SORTLINES_RANDOMLY);
        let shuffled = app.document().text().to_owned();
        let mut actual: Vec<_> = shuffled.split('\n').collect();
        let mut expected: Vec<_> = original.split('\n').collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        app.document_mut().set_read_only(true);
        app.command(IDM_EDIT_SORTLINES_RANDOMLY);
        assert_eq!(app.document().text(), shuffled);
        app.document_mut().set_read_only(false);
        if shuffled != original {
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), original);
        }
    }

    #[test]
    fn locale_sort_commands_use_numeric_caseless_keys_and_keep_final_ending() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("file10\nfile2\nFILE2\n").unwrap();
        app.refresh();
        app.command(IDM_EDIT_SORTLINES_LOCALE_ASCENDING);
        assert_eq!(app.document().text(), "file2\nFILE2\nfile10\n");
        app.document_mut().set_read_only(true);
        app.command(IDM_EDIT_SORTLINES_LOCALE_DESCENDING);
        assert_eq!(app.document().text(), "file2\nFILE2\nfile10\n");
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_SORTLINES_LOCALE_DESCENDING);
        assert_eq!(app.document().text(), "file10\nfile2\nFILE2\n");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "file2\nFILE2\nfile10\n");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "file10\nfile2\nFILE2\n");
    }

    #[test]
    fn decimal_sort_commands_handle_both_separators_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        for (text, ascending, descending, expected) in [
            (
                "10.5\n2.25",
                IDM_EDIT_SORTLINES_DECIMALDOT_ASCENDING,
                IDM_EDIT_SORTLINES_DECIMALDOT_DESCENDING,
                "2.25\n10.5",
            ),
            (
                "10,5\n2,25",
                IDM_EDIT_SORTLINES_DECIMALCOMMA_ASCENDING,
                IDM_EDIT_SORTLINES_DECIMALCOMMA_DESCENDING,
                "2,25\n10,5",
            ),
        ] {
            app.command(IDM_FILE_NEW);
            app.document_mut()
                .convert_line_endings(LineEnding::Lf)
                .unwrap();
            app.document_mut().insert(text).unwrap();
            app.refresh();
            app.command(ascending);
            assert_eq!(app.document().text(), expected);
            app.document_mut().set_read_only(true);
            app.command(descending);
            assert_eq!(app.document().text(), expected);
            app.document_mut().set_read_only(false);
            app.command(descending);
            assert_eq!(app.document().text(), text);
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), expected);
        }
    }

    #[test]
    fn integer_sort_commands_preserve_history_and_read_only_buffers() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut()
            .convert_line_endings(LineEnding::Lf)
            .unwrap();
        app.document_mut().insert("part10\npart2\npart-1").unwrap();
        app.refresh();
        app.command(IDM_EDIT_SORTLINES_INTEGER_ASCENDING);
        assert_eq!(app.document().text(), "part-1\npart2\npart10");
        app.command(IDM_EDIT_SORTLINES_INTEGER_DESCENDING);
        assert_eq!(app.document().text(), "part10\npart2\npart-1");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "part-1\npart2\npart10");
        app.document_mut().set_read_only(true);
        app.command(IDM_EDIT_SORTLINES_INTEGER_DESCENDING);
        assert_eq!(app.document().text(), "part-1\npart2\npart10");
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "part10\npart2\npart-1");
    }

    #[test]
    fn case_insensitive_keys_agree_with_windows_crt() {
        unsafe extern "C" {
            fn _wcsicmp(left: *const u16, right: *const u16) -> i32;
        }
        let values = [
            "a", "A", "z", "Z", "_", "É", "é", "İ", "ß", "😀", "\u{e000}", "a\0z", "a\0b",
        ];
        for left in values {
            for right in values {
                // SAFETY: Both temporary UTF-16 buffers are NUL terminated and
                // remain live throughout the synchronous CRT comparison.
                let comparison = unsafe { _wcsicmp(wide(left).as_ptr(), wide(right).as_ptr()) };
                let mut doc = Document::new();
                doc.convert_line_endings(LineEnding::Lf).unwrap();
                doc.insert(&format!("{left}\n{right}")).unwrap();
                doc.order_lines(LineOrder::CaseInsensitiveAscending)
                    .unwrap();
                let expected = if comparison > 0 {
                    format!("{right}\n{left}")
                } else {
                    format!("{left}\n{right}")
                };
                assert_eq!(doc.text(), expected, "comparison of {left:?} and {right:?}");
            }
        }
    }

    #[test]
    fn additional_sort_commands_use_core_history_and_read_only_rules() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut()
            .convert_line_endings(LineEnding::Lf)
            .unwrap();
        app.document_mut().insert("zz\nB\naaa").unwrap();
        app.refresh();
        for (command, expected) in [
            (
                IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_ASCENDING,
                "aaa\nB\nzz",
            ),
            (
                IDM_EDIT_SORTLINES_LEXICO_CASE_INSENS_DESCENDING,
                "zz\nB\naaa",
            ),
            (IDM_EDIT_SORTLINES_LENGTH_ASCENDING, "B\nzz\naaa"),
            (IDM_EDIT_SORTLINES_LENGTH_DESCENDING, "aaa\nzz\nB"),
        ] {
            app.command(command);
            assert_eq!(app.document().text(), expected);
            // The descending case-insensitive input is already sorted, so it
            // does not create an undo entry.
            if expected != "zz\nB\naaa" {
                app.command(IDM_EDIT_UNDO);
            }
            app.document_mut().set_read_only(true);
            app.command(command);
            assert_eq!(app.document().text(), "zz\nB\naaa");
            app.document_mut().set_read_only(false);
        }
    }

    #[test]
    fn save_copy_writes_encoding_without_adopting_destination() {
        let directory = std::env::temp_dir().join(format!("npp-save-copy-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let original = directory.join("original.txt");
        let copy = directory.join("copy.txt");
        std::fs::write(&original, b"original").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&original).unwrap();
        app.command(IDM_FILE_NEW);
        app.document_mut().insert("é\r\n").unwrap();
        app.command(IDM_FORMAT_CONV2_UTF_16BE);
        app.document_mut().set_read_only(true);
        let id = app.active_id();
        assert_eq!(
            app.save_copy_to_checked(&original, false)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        app.save_copy_to_checked(&copy, false).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            Encoding::Utf16Be.encode("é\r\n").unwrap()
        );
        assert_eq!(std::fs::read(&original).unwrap(), b"original");
        assert_eq!(app.active_id(), id);
        assert!(app.document().path().is_none());
        assert!(app.document().is_dirty());
        assert!(app.document().is_user_read_only());
        assert_eq!(app.workspace.untitled_name(id), Some("new 1"));
        assert!(
            app.save_copy_to_checked(&directory.join("missing").join("copy.txt"), false)
                .is_err()
        );
        assert!(app.document().path().is_none());
        std::fs::remove_file(copy).unwrap();
        std::fs::remove_file(original).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn save_as_protects_open_tabs_and_unlocks_writable_copy() {
        let directory = std::env::temp_dir().join(format!("npp-app-save-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let original = directory.join("original.txt");
        let copy = directory.join("copy.txt");
        std::fs::write(&original, b"original content").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&original).unwrap();
        app.command(IDM_FILE_NEW);
        app.document_mut().insert("new content").unwrap();
        assert_eq!(
            app.save_to(&original).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(app.document().is_dirty());
        assert_eq!(std::fs::read(&original).unwrap(), b"original content");
        app.document_mut().set_file_read_only(true);
        app.save_to(&copy).unwrap();
        assert!(!app.document().is_read_only());
        assert!(!app.document().is_dirty());
        assert_eq!(
            app.document().path(),
            Some(normalized_save_path(&copy).unwrap().as_path())
        );
        assert_eq!(std::fs::read(&copy).unwrap(), b"new content");
        let buffer = app.active_id();
        app.document_mut().insert(" unsaved").unwrap();
        let previous = app.document().text().to_owned();
        std::fs::write(&copy, [0xff]).unwrap();
        assert!(app.reload().is_err());
        assert_eq!(app.document().text(), previous);
        assert!(app.document().is_dirty());
        std::fs::write(&copy, b"external content").unwrap();
        app.reload().unwrap();
        assert_eq!(app.active_id(), buffer);
        assert_eq!(control_text(app.edit), "external content");
        assert!(!app.document().is_dirty());
        assert!(!app.document().can_undo());
        app.command(IDM_FILE_CLOSE);
        std::fs::write(&copy, b"changed while closed").unwrap();
        app.command(IDM_FILE_RESTORELASTCLOSEDFILE);
        assert_eq!(control_text(app.edit), "changed while closed");
        assert!(!app.document().can_undo());
        app.command(IDM_FILE_CLOSE);
        std::fs::write(&copy, b"caf\xe9").unwrap();
        app.open_as(&copy, Some(Encoding::Windows1252)).unwrap();
        assert_eq!(control_text(app.edit), "café");
        std::fs::write(&copy, [0x80]).unwrap();
        app.reload().unwrap();
        assert_eq!(control_text(app.edit), "€");
        assert_eq!(app.document().encoding(), Encoding::Windows1252);
        app.command(IDM_FILE_CLOSE);
        app.command(IDM_FILE_RESTORELASTCLOSEDFILE);
        assert_eq!(control_text(app.edit), "€");
        assert_eq!(app.document().encoding(), Encoding::Windows1252);
        app.command(IDM_FORMAT_CONV2_AS_UTF_8);
        app.save_to(&copy).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), "€".as_bytes());
        for (command, encoding) in [
            (CONVERT_UTF16LE_NO_BOM, Encoding::Utf16LeNoBom),
            (CONVERT_UTF16BE_NO_BOM, Encoding::Utf16BeNoBom),
        ] {
            app.command(command);
            app.save_to(&copy).unwrap();
            assert_eq!(std::fs::read(&copy).unwrap(), encoding.encode("€").unwrap());
            app.reload().unwrap();
            assert_eq!(control_text(app.edit), "€");
            app.command(IDM_FILE_CLOSE);
            app.command(IDM_FILE_RESTORELASTCLOSEDFILE);
            assert_eq!(app.document().encoding(), encoding);
            assert_eq!(control_text(app.edit), "€");
        }
        app.command(IDM_FORMAT_CONV2_AS_UTF_8);
        app.save_to(&copy).unwrap();
        app.document_mut().insert("local edit").unwrap();
        std::fs::write(&copy, b"another program").unwrap();
        assert_eq!(
            app.save_to(&copy).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(std::fs::read(&copy).unwrap(), b"another program");
        assert!(app.document().is_dirty());
        app.save_to_checked(&copy, true).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            app.document().to_bytes().unwrap()
        );
        std::fs::remove_file(&copy).unwrap();
        assert_eq!(
            app.save_to(&copy).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        app.save_to_checked(&copy, true).unwrap();
        assert!(!app.document().is_dirty());
        enqueue(Event::OpenPaths(vec![
            original.clone(),
            copy.clone(),
            original.clone(),
        ]));
        app.drain();
        assert_eq!(app.workspace.buffers().count(), 2);
        assert_eq!(control_text(app.edit), "original content");
        drop(app);
        std::fs::remove_file(original).unwrap();
        std::fs::remove_file(copy).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn extended_search_mode_applies_to_native_requests_and_repeat_search() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\nb\n").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(SEARCH_EXTENDED);
        app.search_text = r"\n".into();
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 1..2);
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACE,
            needle: r"\n".into(),
            replacement: r"\t".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "a\tb\n");
        assert_eq!(app.document().selection().range(), 3..4);
        assert_eq!(app.search_options.mode, SearchMode::Extended);
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: r"\t".into(),
            replacement: r"\\n".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(control_text(app.edit), "a\\nb\r\n");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\tb\n");
        app.command(SEARCH_EXTENDED);
        assert_eq!(app.search_options.mode, SearchMode::Normal);
        assert!(search_dialog::native_replacement(SearchMode::Extended, r"\0").is_err());
        assert_eq!(
            search_dialog::native_replacement(SearchMode::Normal, r"\0").unwrap(),
            r"\0"
        );
    }

    #[test]
    fn native_plugin_menu_dispatch_and_shutdown_keep_dll_alive() {
        use npp_compat::messages::{
            NPPM_GETMENUHANDLE, NPPM_SETMENUITEMCHECK, NPPMAINMENU, NPPPLUGINMENU,
        };
        use npp_compat::{loader::LoadedPlugin, plugin::NppData, registry::FIRST_PLUGIN_COMMAND};
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "npp-rust-native-plugin-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("fixture.dll");
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/npp-compat/tests/fixtures/plugin.rs");
        let output = std::process::Command::new("rustc")
            .arg("--crate-type=cdylib")
            .arg("--edition=2024")
            .arg(source)
            .arg("-o")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut app = App::create(false).unwrap();
        app.drain();
        let host_window = app.window;
        let data = NppData {
            npp_handle: app.window,
            scintilla_main_handle: null_mut(),
            scintilla_second_handle: null_mut(),
        };
        // SAFETY: Controlled fixture ignores editor handles, starts no threads,
        // and uses scalar host queries plus IsWindow on the live main window.
        unsafe {
            let plugin = LoadedPlugin::load(&path, data).unwrap();
            // An extra reference lets the test inspect the fixture after App drops.
            let observer = LoadedPlugin::load(&path, data).unwrap();
            app.plugins.register(plugin).unwrap();
            let old = app.plugin_menu;
            app.rebuild_plugin_menu().unwrap();
            assert_eq!(IsMenu(old), 0);
            let submenu = GetSubMenu(app.plugin_menu, 0);
            assert_eq!(GetMenuItemCount(submenu), 2);
            assert_eq!(GetMenuItemID(submenu, 0), FIRST_PLUGIN_COMMAND as u32);
            assert_ne!(GetMenuState(submenu, 0, MF_BYPOSITION) & MF_CHECKED, 0);
            assert_ne!(GetMenuState(submenu, 1, MF_BYPOSITION) & MF_SEPARATOR, 0);
            let mut shortcut = npp_compat::plugin::ShortcutKey {
                is_ctrl: false,
                is_alt: false,
                is_shift: true,
                key: b'Q',
            };
            let query_shortcut = npp_compat::messages::NPPM_GETSHORTCUTBYCMDID;
            let shortcut_ptr = &mut shortcut as *mut _ as isize;
            assert_eq!(
                SendMessageW(
                    app.window,
                    query_shortcut,
                    FIRST_PLUGIN_COMMAND as usize,
                    shortcut_ptr
                ),
                1
            );
            assert_eq!(
                shortcut,
                npp_compat::plugin::ShortcutKey {
                    is_ctrl: true,
                    is_alt: true,
                    is_shift: false,
                    key: b'R'
                }
            );
            let expected_shortcut = shortcut;
            for id in [0, FIRST_PLUGIN_COMMAND as usize + 1, 0xffff] {
                assert_eq!(
                    SendMessageW(app.window, query_shortcut, id, shortcut_ptr),
                    0
                );
                assert_eq!(shortcut, expected_shortcut);
            }
            assert_eq!(
                SendMessageW(app.window, query_shortcut, FIRST_PLUGIN_COMMAND as usize, 0),
                0
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPMAINMENU, 0),
                GetMenu(app.window) as isize
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPPLUGINMENU, 0),
                app.plugin_menu as isize
            );
            assert_eq!(SendMessageW(app.window, NPPM_GETMENUHANDLE, 99, 0), 0);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_SETMENUITEMCHECK,
                    FIRST_PLUGIN_COMMAND as usize,
                    0
                ),
                1
            );
            assert_eq!(GetMenuState(submenu, 0, MF_BYPOSITION) & MF_CHECKED, 0);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_SETMENUITEMCHECK,
                    FIRST_PLUGIN_COMMAND as usize,
                    -1
                ),
                1
            );
            assert_ne!(GetMenuState(submenu, 0, MF_BYPOSITION) & MF_CHECKED, 0);
            SendMessageW(
                app.window,
                NPPM_SETMENUITEMCHECK,
                FIRST_PLUGIN_COMMAND as usize,
                0,
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_SETMENUITEMCHECK, 0xffff, 1),
                1
            );
            SendMessageW(app.window, WM_COMMAND, FIRST_PLUGIN_COMMAND as usize, 0);
            app.drain();
            assert_eq!(observer.message(0, 0, 0), 11);
            // Set only this test thread's queued keyboard state, then restore it
            // before assertions. No physical/global keyboard input is synthesized.
            let mut original_keys = [0u8; 256];
            assert_ne!(GetKeyboardState(original_keys.as_mut_ptr()), 0);
            let mut keys = [0u8; 256];
            keys[VK_CONTROL as usize] = 0x80;
            keys[VK_MENU as usize] = 0x80;
            assert_ne!(SetKeyboardState(keys.as_ptr()), 0);
            let key_message = MSG {
                hwnd: app.edit,
                message: WM_SYSKEYDOWN,
                wParam: b'R' as usize,
                ..MSG::default()
            };
            let handled = app.plugin_accelerators.translate(app.window, &key_message);
            keys[VK_SHIFT as usize] = 0x80;
            let extra_keys_set = SetKeyboardState(keys.as_ptr());
            let extra_modifier_handled =
                app.plugin_accelerators.translate(app.window, &key_message);
            let no_keys_set = SetKeyboardState([0u8; 256].as_ptr());
            let no_modifier_handled = app.plugin_accelerators.translate(app.window, &key_message);
            let restored = SetKeyboardState(original_keys.as_ptr());
            assert_ne!(restored, 0);
            assert_ne!(extra_keys_set, 0);
            assert_ne!(no_keys_set, 0);
            assert!(handled);
            assert!(!extra_modifier_handled);
            assert!(!no_modifier_handled);
            app.drain();
            assert_eq!(observer.message(0, 0, 0), 12);
            let token = SendMessageW(
                app.window,
                npp_compat::messages::NPPM_GETCURRENTBUFFERID,
                0,
                0,
            );
            assert_ne!(token, 0);
            assert_eq!(observer.message(4, 0, 0), token);
            assert_eq!(
                observer.message(5, 0, 0),
                npp_compat::messages::MAIN_VIEW as isize
            );
            app.rebuild_plugin_menu().unwrap();
            assert_eq!(
                GetMenuItemID(GetSubMenu(app.plugin_menu, 0), 0),
                FIRST_PLUGIN_COMMAND as u32
            );
            assert_eq!(
                GetMenuState(GetSubMenu(app.plugin_menu, 0), 0, MF_BYPOSITION) & MF_CHECKED,
                0
            );
            assert_eq!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPPLUGINMENU, 0),
                app.plugin_menu as isize
            );
            app.refresh();
            assert_eq!(
                SendMessageW(app.window, NPPM_GETMENUHANDLE, NPPPLUGINMENU, 0),
                app.plugin_menu as isize
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    query_shortcut,
                    FIRST_PLUGIN_COMMAND as usize,
                    shortcut_ptr
                ),
                1
            );
            assert_eq!(shortcut, expected_shortcut);
            let remove_shortcut = npp_compat::messages::NPPM_REMOVESHORTCUTBYCMDID;
            assert_eq!(SendMessageW(app.window, remove_shortcut, 0, 0), 0);
            assert_eq!(SendMessageW(app.window, remove_shortcut, 0xffff, 0), 1);
            assert_eq!(
                SendMessageW(
                    app.window,
                    remove_shortcut,
                    FIRST_PLUGIN_COMMAND as usize,
                    0
                ),
                1
            );
            assert_eq!(
                SendMessageW(
                    app.window,
                    query_shortcut,
                    FIRST_PLUGIN_COMMAND as usize,
                    shortcut_ptr
                ),
                0
            );
            assert_eq!(shortcut, expected_shortcut);
            app.rebuild_plugin_menu().unwrap();
            assert_eq!(
                SendMessageW(
                    app.window,
                    query_shortcut,
                    FIRST_PLUGIN_COMMAND as usize,
                    shortcut_ptr
                ),
                0
            );
            keys[VK_SHIFT as usize] = 0;
            assert_ne!(SetKeyboardState(keys.as_ptr()), 0);
            let removed_handled = app.plugin_accelerators.translate(app.window, &key_message);
            let restored = SetKeyboardState(original_keys.as_ptr());
            assert_ne!(restored, 0);
            assert!(!removed_handled);
            app.drain();
            assert_eq!(observer.message(0, 0, 0), 12);
            SendMessageW(app.window, WM_COMMAND, FIRST_PLUGIN_COMMAND as usize, 0);
            app.drain();
            assert_eq!(observer.message(0, 0, 0), 13);
            drop(app);
            assert_eq!(observer.message(2, 0, 0), 1009);
            assert_eq!(observer.message(3, 0, 0), 1);
            assert_eq!(IsWindow(host_window), 0);
            assert_eq!(observer.message(0, 0, 0), 113);
            drop(observer);
        }
        std::fs::remove_file(path).unwrap();
        // Only this test's compiler outputs exist in the uniquely created directory.
        for entry in std::fs::read_dir(&directory).unwrap() {
            std::fs::remove_file(entry.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn plugin_caret_and_line_text_queries_match_tab_aware_status() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\t🦀\r\nb").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 6,
                caret: 6,
            })
            .unwrap();
        app.refresh();
        assert!(control_text(app.status).starts_with("Ln 1, Col 6"));
        // SAFETY: Test owns these windows and output buffers throughout dispatch.
        unsafe {
            assert_eq!(SendMessageW(app.window, NPPM_GETCURRENTLINE, 0, 0), 0);
            assert_eq!(SendMessageW(app.window, NPPM_GETCURRENTCOLUMN, 0, 0), 5);
            let mut output = [0xa55au16; 8];
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETCURRENTLINESTR,
                    6,
                    output.as_mut_ptr() as isize
                ),
                0
            );
            assert_eq!(output, [0xa55a; 8]);
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETCURRENTLINESTR,
                    7,
                    output.as_mut_ptr() as isize
                ),
                1
            );
            assert_eq!(String::from_utf16_lossy(&output[..6]), "a\t🦀\r\n");
            assert_eq!(&output[6..], &[0, 0xa55a]);
            SendMessageW(app.edit, EM_SETSEL, 6, 6);
            SendMessageW(app.edit, WM_KEYUP, VK_RIGHT as usize, 0);
            app.drain();
            assert_eq!(SendMessageW(app.window, NPPM_GETCURRENTLINE, 0, 0), 1);
            assert_eq!(SendMessageW(app.window, NPPM_GETCURRENTCOLUMN, 0, 0), 0);
        }
        for length in [2047, 2048] {
            app.workspace
                .add(Document::from_bytes("a".repeat(length).as_bytes()).unwrap());
            app.refresh();
            let mut output = vec![0xa55au16; 2050];
            // SAFETY: Output has capacity for either the full line or an empty string.
            unsafe {
                assert_eq!(
                    SendMessageW(
                        app.window,
                        NPPM_GETCURRENTLINESTR,
                        output.len(),
                        output.as_mut_ptr() as isize
                    ),
                    1
                );
            }
            let expected = if length == 2048 { 0 } else { length };
            assert_eq!(output.iter().position(|&unit| unit == 0), Some(expected));
            assert_eq!(output[expected + 1], 0xa55a);
        }
    }

    #[test]
    fn untitled_names_agree_in_tabs_title_and_plugin_queries_after_reorder() {
        use npp_compat::messages::NPPM_GETFILENAME;
        let mut app = App::create(false).unwrap();
        app.drain();
        let first = app.active_id();
        app.command(IDM_FILE_NEW);
        app.command(IDM_FILE_NEW);
        app.command(IDM_VIEW_TAB_MOVEBACKWARD);
        assert!(app.close_documents(vec![first], |_| true));
        assert!(control_text(app.window).starts_with("new 3 —"));
        // SAFETY: Sized writable UTF-16 buffers and TCITEMW remain live during each call.
        unsafe {
            let mut name = [0u16; 64];
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETFILENAME,
                    name.len(),
                    name.as_mut_ptr() as isize
                ),
                1
            );
            assert_eq!(String::from_utf16_lossy(&name[..5]), "new 3");
            let mut item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: name.as_mut_ptr(),
                cchTextMax: name.len() as i32,
                ..Default::default()
            };
            assert_ne!(
                SendMessageW(app.tabs, TCM_GETITEMW, 0, &mut item as *mut _ as isize),
                0
            );
            assert_eq!(String::from_utf16_lossy(&name[..5]), "new 3");
            assert_ne!(
                SendMessageW(app.tabs, TCM_GETITEMW, 1, &mut item as *mut _ as isize),
                0
            );
            assert_eq!(String::from_utf16_lossy(&name[..5]), "new 2");
        }
        app.command(IDM_FILE_NEW);
        assert!(control_text(app.window).starts_with("new 1 —"));
    }

    #[test]
    fn plugin_executable_queries_ignore_document_path_and_check_capacity() {
        use npp_compat::messages::*;
        use std::os::windows::ffi::OsStrExt;
        let mut app = App::create(false).unwrap();
        app.drain();
        let executable = std::env::current_exe().unwrap();
        for (message, expected) in [
            (NPPM_GETNPPFULLFILEPATH, executable.as_path()),
            (NPPM_GETNPPDIRECTORY, executable.parent().unwrap()),
        ] {
            let expected: Vec<_> = expected.as_os_str().encode_wide().collect();
            assert!(expected.len() < 260);
            let mut output = vec![0xa55a; expected.len() + 3];
            // SAFETY: The destination has capacity for the path, terminator and
            // guards; all messages target this test's live native window.
            unsafe {
                let destination = output.as_mut_ptr().add(1) as isize;
                assert_eq!(
                    SendMessageW(app.window, message, expected.len(), destination),
                    0
                );
                assert!(output.iter().all(|&unit| unit == 0xa55a));
                assert_eq!(
                    SendMessageW(app.window, message, expected.len() + 1, destination),
                    1
                );
                assert_eq!(&output[1..expected.len() + 1], expected);
                assert_eq!(output[expected.len() + 1], 0);
                assert_eq!(output[0], 0xa55a);
                assert_eq!(output[expected.len() + 2], 0xa55a);
                assert_eq!(SendMessageW(app.window, message, 0, 0), 0);
                app.document_mut()
                    .save_with(Path::new(r"C:\unrelated\file.txt"), |_, _| Ok(()))
                    .unwrap();
                app.refresh();
                output.fill(0xa55a);
                assert_eq!(SendMessageW(app.window, message, 0, destination), 1);
                assert_eq!(&output[1..expected.len() + 1], expected);
            }
        }
    }

    #[test]
    fn plugin_path_queries_obey_utf16_lengths_capacities_and_terminators() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        let path = Path::new(r"C:\folder\résumé🦀.txt");
        app.document_mut().save_with(path, |_, _| Ok(())).unwrap();
        app.refresh();
        // SAFETY: All output pointers below refer to sized writable allocations.
        unsafe {
            let token = SendMessageW(app.window, NPPM_GETCURRENTBUFFERID, 0, 0) as usize;
            let length = SendMessageW(app.window, NPPM_GETFULLPATHFROMBUFFERID, token, 0) as usize;
            let expected: Vec<_> = r"C:\folder\résumé🦀.txt".encode_utf16().collect();
            assert_eq!(length, expected.len());
            let mut output = vec![0xa55a; length + 3];
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETFULLPATHFROMBUFFERID,
                    token,
                    output.as_mut_ptr().add(1) as isize
                ),
                length as isize
            );
            assert_eq!(&output[1..length + 1], expected);
            assert_eq!(output[length + 1], 0);
            assert_eq!(output[0], 0xa55a);
            assert_eq!(output[length + 2], 0xa55a);
            for (message, text) in [
                (NPPM_GETFULLCURRENTPATH, r"C:\folder\résumé🦀.txt"),
                (NPPM_GETCURRENTDIRECTORY, r"C:\folder"),
                (NPPM_GETFILENAME, "résumé🦀.txt"),
                (NPPM_GETNAMEPART, "résumé🦀"),
                (NPPM_GETEXTPART, ".txt"),
            ] {
                let expected: Vec<_> = text.encode_utf16().collect();
                let mut output = vec![0xa55a; expected.len() + 3];
                let destination = output.as_mut_ptr().add(1) as isize;
                assert_eq!(
                    SendMessageW(app.window, message, expected.len(), destination),
                    0
                );
                assert!(output.iter().all(|&unit| unit == 0xa55a));
                assert_eq!(
                    SendMessageW(app.window, message, expected.len() + 1, destination),
                    1
                );
                assert_eq!(&output[1..expected.len() + 1], expected);
                assert_eq!(output[expected.len() + 1], 0);
                assert_eq!(output[expected.len() + 2], 0xa55a);
                assert_eq!(output[0], 0xa55a);
                output.fill(0xa55a);
                assert_eq!(SendMessageW(app.window, message, 0, destination), 1);
                assert_eq!(SendMessageW(app.window, message, expected.len() + 1, 0), 0);
            }
            let mut untouched = [0xa55au16; 2];
            assert_eq!(
                SendMessageW(
                    app.window,
                    NPPM_GETFULLPATHFROMBUFFERID,
                    0,
                    untouched.as_mut_ptr() as isize
                ),
                0
            );
            assert_eq!(untouched, [0xa55a; 2]);
        }
    }

    #[test]
    fn plugin_view_queries_use_view_index_and_a_32_bit_output() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        for command in [IDM_FILE_NEW, IDM_VIEW_TAB_MOVEBACKWARD, IDM_VIEW_TAB_NEXT] {
            app.command(command);
            // SAFETY: The middle element is a writable i32 surrounded by sentinels.
            unsafe {
                let mut output = [0x12345678i32, -1, 0x76543210];
                assert_eq!(
                    SendMessageW(app.window, NPPM_GETCURRENTVIEW, 0, 0),
                    MAIN_VIEW as isize
                );
                assert_eq!(SendMessageW(app.window, NPPM_GETCURRENTSCINTILLA, 0, 0), 0);
                assert_eq!(
                    SendMessageW(
                        app.window,
                        NPPM_GETCURRENTSCINTILLA,
                        usize::MAX,
                        output.as_mut_ptr().add(1) as isize
                    ),
                    1
                );
                assert_eq!(output, [0x12345678, MAIN_VIEW, 0x76543210]);
            }
        }
    }

    #[test]
    fn plugin_buffer_queries_follow_tabs_metadata_and_close() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        let window = app.window;
        // SAFETY: Synchronous scalar-only messages to this test's live window.
        let query = |message, wp, lp| unsafe { SendMessageW(window, message, wp, lp) };
        let first = query(NPPM_GETCURRENTBUFFERID, 0, 0) as usize;
        assert_ne!(first, 0);
        assert_eq!(query(NPPM_GETNBOPENFILES, 0, 0), 1);
        assert_eq!(query(NPPM_GETNBOPENFILES, 0, 2), 0);
        assert_eq!(query(NPPM_GETCURRENTDOCINDEX, 0, 1), -1);
        assert_eq!(query(NPPM_GETBUFFERIDFROMPOS, 0, 0) as usize, first);
        app.command(IDM_FILE_NEW);
        let second = query(NPPM_GETCURRENTBUFFERID, 0, 0) as usize;
        assert_ne!(second, first);
        assert_eq!(query(NPPM_GETCURRENTDOCINDEX, 0, 0), 1);
        assert_eq!(query(NPPM_GETNBOPENFILES, 0, 1), 2);
        app.command(IDM_VIEW_TAB_MOVEBACKWARD);
        assert_eq!(query(NPPM_GETCURRENTBUFFERID, 0, 0) as usize, second);
        assert_eq!(query(NPPM_GETPOSFROMBUFFERID, first, 1), 1);
        assert_eq!(query(NPPM_GETPOSFROMBUFFERID, second, 0), 0);
        app.command(IDM_FORMAT_CONV2_UTF_16BE);
        app.command(IDM_FORMAT_TOMAC);
        assert_eq!(query(NPPM_GETBUFFERENCODING, second, 0), 2);
        assert_eq!(query(NPPM_GETBUFFERFORMAT, second, 0), 1);
        let id = app.active_id();
        assert!(app.close_documents(vec![id], |_| true));
        assert_eq!(query(NPPM_GETBUFFERENCODING, second, 0), -1);
        assert_eq!(query(NPPM_GETPOSFROMBUFFERID, second, 0), -1);
        app.command(IDM_FILE_NEW);
        assert_ne!(query(NPPM_GETCURRENTBUFFERID, 0, 0) as usize, second);
        assert_eq!(query(NPPM_GETBUFFERIDFROMPOS, usize::MAX, 0), 0);
        assert_eq!(query(NPPM_GETBUFFERIDFROMPOS, 0, 1), 0);
        assert_eq!(query(NPPM_GETBUFFERFORMAT, 0, 0), -1);
        drop(app);
        assert_eq!(
            crate::plugin_host::query(window, NPPM_GETCURRENTBUFFERID, 0, 0),
            None
        );
    }

    #[test]
    fn scoped_count_and_replace_use_native_unicode_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀 a a a").unwrap();
        app.refresh();
        // SAFETY: Select the middle two occurrences using UTF-16 control positions.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 3, 6);
        }
        app.search_text = "a".into();
        app.command(SEARCH_IN_SELECTION);
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 2 occurrences");
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "a".into(),
            replacement: "bb".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "🦀 bb bb a");
        assert_eq!(control_selection(app.edit), (3, 8));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "🦀 a a a");
        assert_eq!(control_selection(app.edit), (3, 6));
        // SAFETY: Collapse the native selection to verify empty scope stays empty.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 3, 3);
        }
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "a".into(),
            replacement: "x".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "🦀 a a a");
        assert_eq!(control_text(app.status), "Replaced 0 occurrences");
    }

    #[test]
    fn expanded_search_folds_select_native_utf16_and_replace_original_ranges() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀 ß SS ﬃ FFI").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.search_text = "ss".into();
        app.command(IDM_SEARCH_FINDNEXT);
        assert_eq!(app.document().selection().range(), 5..7);
        assert_eq!(control_selection(app.edit), (3, 4));
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACE,
            needle: "ss".into(),
            replacement: "X".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "🦀 X SS ﬃ FFI");
        assert_eq!(app.document().selection().range(), 7..9);
        assert_eq!(control_selection(app.edit), (5, 7));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "🦀 ß SS ﬃ FFI");
        assert_eq!(app.document().selection().range(), 5..7);
        app.search_text = "ffi".into();
        app.command(IDM_SEARCH_FINDNEXT);
        assert_eq!(app.document().selection().range(), 11..14);
        assert_eq!(control_selection(app.edit), (8, 9));
    }

    #[test]
    fn repeat_search_uses_saved_query_live_options_and_native_caret() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a A AA A").unwrap();
        app.refresh();
        app.show_search(false);
        let dialog = app.search_dialog.as_ref().unwrap().hwnd;
        // SAFETY: Set unsent values and a new editor caret through owned controls.
        unsafe {
            SetDlgItemTextW(dialog, edt1 as i32, wide("unsubmitted").as_ptr());
            CheckDlgButton(dialog, chx2 as i32, BST_CHECKED);
            CheckDlgButton(dialog, chx1 as i32, BST_CHECKED);
            SendMessageW(app.edit, EM_SETSEL, 0, 0);
        }
        app.search_text = "A".into();
        app.command(IDM_SEARCH_FINDNEXT);
        assert_eq!(app.document().selection().range(), 2..3);
        app.command(IDM_SEARCH_FINDNEXT);
        assert_eq!(app.document().selection().range(), 7..8);
        app.command(IDM_SEARCH_FINDPREV);
        assert_eq!(app.document().selection().range(), 2..3);
        assert_eq!(app.search_text, "A");
        app.search_dialog.as_ref().unwrap().set_query_text("");
        app.command(IDM_SEARCH_FINDNEXT);
        assert_eq!(app.document().selection().range(), 7..8);
        assert_eq!(app.search_dialog.as_ref().unwrap().hwnd, dialog);
        assert!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .needle
                .is_empty()
        );
    }

    #[test]
    fn cancelling_search_captures_unsent_fields_before_destruction() {
        let mut app = App::create(false).unwrap();
        app.drain();
        for (replace, close_message) in [(true, WM_CLOSE), (false, WM_COMMAND)] {
            app.show_search(replace);
            let dialog = app.search_dialog.as_ref().unwrap();
            let hwnd = dialog.hwnd;
            dialog.set_query_text("unsent 🦀");
            dialog.set_replacement_text("replacement é");
            // SAFETY: Set options and close our own dialog through native processing.
            unsafe {
                CheckDlgButton(hwnd, chx2 as i32, BST_CHECKED);
                SendMessageW(
                    hwnd,
                    close_message,
                    if close_message == WM_COMMAND {
                        IDCANCEL as usize
                    } else {
                        0
                    },
                    0,
                );
            }
            app.drain();
            assert_eq!(app.search_text, "unsent 🦀");
            assert_eq!(app.search_replacement, "replacement é");
            assert!(app.search_options.match_case);
            assert!(app.search_dialog.as_ref().unwrap().hwnd.is_null());
            app.show_search(replace);
            assert_eq!(
                app.search_dialog
                    .as_ref()
                    .unwrap()
                    .current_query()
                    .unwrap()
                    .needle,
                "unsent 🦀"
            );
        }
        assert!(!app.document().can_undo());
    }

    #[test]
    fn switching_search_modes_preserves_unsent_replacement_and_empty_edits() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.search_options.direction = SearchDirection::Backward;
        app.show_search(true);
        let first_id = app.search_dialog.as_ref().unwrap().id;
        let dialog = app.search_dialog.as_ref().unwrap();
        dialog.set_query_text("needle");
        dialog.set_replacement_text("🦀é\\n");
        app.show_search(false);
        assert_ne!(app.search_dialog.as_ref().unwrap().id, first_id);
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .needle,
            "needle"
        );
        assert_eq!(app.search_options.direction, SearchDirection::Backward);
        app.show_search(true);
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .replacement,
            "🦀é\\n"
        );
        app.search_dialog.as_ref().unwrap().set_replacement_text("");
        app.show_search(false);
        app.show_search(true);
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .replacement,
            ""
        );
        assert!(app.document().text().is_empty());
        assert!(!app.document().can_undo());
    }

    #[test]
    fn reopening_search_reuses_live_fields_and_expands_caret_word() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("  word").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.search_options.direction = SearchDirection::Backward;
        app.refresh();
        app.show_search(false);
        let dialog = app.search_dialog.as_ref().unwrap().hwnd;
        let id = app.search_dialog.as_ref().unwrap().id;
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .options
                .direction,
            SearchDirection::Backward
        );
        // SAFETY: Edit only the controls of this test's owned dialog.
        unsafe {
            SetDlgItemTextW(dialog, edt1 as i32, wide("unsubmitted").as_ptr());
            CheckDlgButton(dialog, chx2 as i32, BST_CHECKED);
        }
        app.show_search(false);
        assert_eq!(app.search_dialog.as_ref().unwrap().id, id);
        let query = app.search_dialog.as_ref().unwrap().current_query().unwrap();
        assert_eq!(query.needle, "unsubmitted");
        assert!(query.options.match_case);
        app.document_mut()
            .select(Selection {
                anchor: 4,
                caret: 4,
            })
            .unwrap();
        app.refresh();
        app.show_search(false);
        assert_eq!(app.document().selection().range(), 2..6);
        assert_eq!(app.search_dialog.as_ref().unwrap().hwnd, dialog);
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .needle,
            "word"
        );
    }

    #[test]
    fn tab_scroll_positions_survive_switching_and_session_round_trip() {
        let directory = std::env::temp_dir().join(format!("npp-tab-scroll-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = directory.join("first.txt");
        let second = directory.join("second.txt");
        let output = directory.join("session.xml");
        let text = "line\n".repeat(150);
        std::fs::write(&first, text.as_bytes()).unwrap();
        std::fs::write(&second, text.as_bytes()).unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&first).unwrap();
        app.scroll_to_line(30);
        app.open(&second).unwrap();
        app.scroll_to_line(60);
        app.command(IDM_VIEW_TAB_PREV);
        // SAFETY: Read-only queries of the test's live native EDIT control.
        unsafe {
            assert_eq!(SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0), 30);
            app.command(IDM_VIEW_TAB_NEXT);
            assert_eq!(SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0), 60);
        }
        app.save_session_to(&output, atomic_write).unwrap();
        let session = npp_editor_core::Session::from_xml(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(
            session.main_view.files[0].attributes["firstVisibleLine"],
            "30"
        );
        assert_eq!(
            session.main_view.files[1].attributes["firstVisibleLine"],
            "60"
        );
        app.scroll_to_line(0);
        app.command(IDM_VIEW_TAB_PREV);
        app.scroll_to_line(0);
        app.load_session_from(&output).unwrap();
        // SAFETY: Read-only queries of the test's live native EDIT control.
        unsafe {
            assert_eq!(SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0), 60);
            app.command(IDM_VIEW_TAB_PREV);
            assert_eq!(SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0), 30);
        }
        assert!(
            app.workspace
                .buffers()
                .all(|(_, document)| !document.is_dirty() && !document.can_undo())
        );
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn session_load_replaces_initial_empty_tab_and_restores_active_view_scroll() {
        use npp_editor_core::{Session, SessionFile};
        let directory =
            std::env::temp_dir().join(format!("npp-session-views-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = directory.join("main.txt");
        let second = directory.join("sub.txt");
        let output = directory.join("session.xml");
        std::fs::write(&first, b"main").unwrap();
        let text = (0..150)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        std::fs::write(&second, text.as_bytes()).unwrap();
        let mut session = Session {
            active_view: 1,
            ..Session::default()
        };
        session
            .main_view
            .files
            .push(SessionFile::new(first.to_str().unwrap().into()));
        let mut sub = SessionFile::new(second.to_str().unwrap().into());
        sub.attributes
            .insert("firstVisibleLine".into(), "60".into());
        session.sub_view.files.push(sub);
        std::fs::write(&output, session.to_xml().unwrap()).unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.load_session_from(&output).unwrap();
        assert_eq!(app.workspace.buffers().count(), 2);
        assert_eq!(app.document().text(), text);
        assert_eq!(app.document().selection(), Selection::default());
        // SAFETY: Read-only query of the test's live EDIT control.
        assert_eq!(
            unsafe { SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0) },
            60
        );
        let before = app.active_id();
        std::fs::write(&output, Session::default().to_xml().unwrap()).unwrap();
        app.load_session_from(&output).unwrap();
        assert_eq!(app.active_id(), before);
        // Empty sessions leave the current editor view untouched.
        // SAFETY: Read-only query of the test's live EDIT control.
        assert_eq!(
            unsafe { SendMessageW(app.edit, EM_GETFIRSTVISIBLELINE, 0, 0) },
            60
        );
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn manual_session_load_keeps_dirty_buffers_and_restores_successful_entries() {
        use npp_editor_core::{Session, SessionFile};
        let directory =
            std::env::temp_dir().join(format!("npp-session-load-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let existing = directory.join("existing.txt");
        let legacy = directory.join("legacy.txt");
        let output = directory.join("session.xml");
        std::fs::write(&existing, b"abc\nxyz").unwrap();
        std::fs::write(&legacy, [0x82, 0xa0, b'x']).unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&existing).unwrap();
        let existing_id = app.active_id();
        app.document_mut().insert("unsaved ").unwrap();
        app.document_mut().toggle_bookmark(0).unwrap();
        app.refresh();

        let mut session = Session::default();
        let mut first = SessionFile::new(existing.to_str().unwrap().into());
        first.attributes.insert("startPos".into(), "12".into());
        first.attributes.insert("endPos".into(), "8".into());
        first.attributes.insert("userReadOnly".into(), "yes".into());
        first.bookmarks = vec![1];
        let mut second = SessionFile::new(legacy.to_str().unwrap().into());
        second.attributes.insert("encoding".into(), "932".into());
        second.attributes.insert("startPos".into(), "2".into());
        second.attributes.insert("endPos".into(), "3".into());
        session.main_view.files = vec![
            SessionFile::new(directory.join("missing.txt").to_str().unwrap().into()),
            first,
            second,
        ];
        std::fs::write(&output, session.to_xml().unwrap()).unwrap();
        let error = app.load_session_from(&output).unwrap_err();
        assert!(error.to_string().contains("missing.txt"));
        assert_eq!(app.workspace.buffers().count(), 2);
        assert_eq!(app.active_id(), existing_id);
        assert_eq!(app.document().text(), "unsaved abc\nxyz");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 12,
                caret: 8
            }
        );
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(app.document().is_user_read_only());
        assert!(app.document().is_dirty());
        assert!(app.document().can_undo());
        assert_eq!(std::fs::read(&existing).unwrap(), b"abc\nxyz");

        // Repeated loads reuse buffers and resolve the active index after the
        // missing file has been discarded, like the source.
        session.main_view.active_index = 1;
        std::fs::write(&output, session.to_xml().unwrap()).unwrap();
        assert!(app.load_session_from(&output).is_err());
        assert_eq!(app.workspace.buffers().count(), 2);
        assert_eq!(app.document().text(), "あx");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 3,
                caret: 4
            }
        );
        assert_eq!(control_selection(app.edit), (1, 2));
        assert!(!app.document().is_dirty());

        let before = app.active_id();
        std::fs::write(&output, b"<NotepadPlus><Session>").unwrap();
        assert!(app.load_session_from(&output).is_err());
        assert_eq!(app.active_id(), before);
        assert_eq!(app.workspace.buffers().count(), 2);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 3,
                caret: 4
            }
        );
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(existing).unwrap();
        std::fs::remove_file(legacy).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn manual_session_save_preserves_dirty_documents_and_records_bookmarks() {
        let directory =
            std::env::temp_dir().join(format!("npp-session-save-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("source.txt");
        let output = directory.join("session.xml");
        std::fs::write(&file, b"one\ntwo").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&file).unwrap();
        app.document_mut().insert("unsaved ").unwrap();
        app.document_mut().toggle_bookmark(1).unwrap();
        let selection = Selection {
            anchor: 11,
            caret: 8,
        };
        app.document_mut().select(selection).unwrap();
        app.refresh();
        assert!(
            app.save_session_to(&output, |_, _| Err(io::Error::other("fixture")))
                .is_err()
        );
        assert!(!output.exists());
        assert!(app.document().is_dirty());
        assert_eq!(app.document().selection(), selection);
        app.save_session_to(&output, atomic_write).unwrap();
        let session = npp_editor_core::Session::from_xml(&std::fs::read(&output).unwrap()).unwrap();
        assert_eq!(session.main_view.files.len(), 1);
        assert_eq!(session.main_view.files[0].bookmarks, vec![1]);
        assert_eq!(session.main_view.files[0].attributes["startPos"], "11");
        assert_eq!(session.main_view.files[0].attributes["endPos"], "8");
        assert_eq!(std::fs::read(&file).unwrap(), b"one\ntwo");
        assert!(app.document().is_dirty());
        assert!(app.document().can_undo());
        std::fs::remove_file(output).unwrap();
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn clear_bookmarks_in_reverse_selection_keeps_other_lines_and_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\nb\nc\n").unwrap();
        app.document_mut().invert_bookmarks();
        let before = Selection {
            anchor: 5,
            caret: 1,
        };
        app.document_mut().select(before).unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.command(CLEAR_BOOKMARKS_IN_SELECTION);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(app.document().selection(), before);
        assert_eq!(app.document().text(), "a\r\nb\nc\n");
    }

    #[test]
    fn bookmark_matching_lines_honors_scope_wrap_and_read_only() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("hit\r\nx\nhit\rhit").unwrap();
        let before = Selection {
            anchor: 10,
            caret: 7,
        };
        app.document_mut().select(before).unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.search_text = "hit".into();
        app.search_in_selection = true;
        app.command(SEARCH_BOOKMARK_MATCHES);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(app.document().selection(), before);
        assert!(control_text(app.status).contains("1 occurrences"));
        app.search_in_selection = false;
        app.search_options.wrap = false;
        app.search_options.direction = SearchDirection::Forward;
        app.command(SEARCH_BOOKMARK_MATCHES);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![2, 3]
        );
        app.search_options.wrap = true;
        app.command(SEARCH_BOOKMARK_MATCHES);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
        assert_eq!(app.document().selection(), before);
    }

    #[test]
    fn paste_bookmarks_uses_clipboard_text_and_preserves_state_on_read_failure() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\nb\n").unwrap();
        app.document_mut().toggle_bookmark(0).unwrap();
        app.document_mut().toggle_bookmark(2).unwrap();
        app.refresh();
        app.paste_bookmarked_lines(|| Ok(None)).unwrap();
        assert!(
            app.paste_bookmarked_lines(|| Err(io::Error::other("fixture")))
                .is_err()
        );
        assert_eq!(app.document().text(), "a\r\nb\n");
        app.paste_bookmarked_lines(|| Ok(Some("🦀".into())))
            .unwrap();
        assert_eq!(app.document().text(), "🦀\r\nb\n🦀");
        assert_eq!(control_text(app.edit), "🦀\r\nb\r\n🦀");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\r\nb\n");
        app.document_mut().set_read_only(true);
        app.paste_bookmarked_lines(|| panic!("read-only paste must not read clipboard"))
            .unwrap();
    }

    #[test]
    fn cut_bookmarked_lines_keeps_one_undo_step_when_clipboard_fails() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\nb\nc").unwrap();
        app.document_mut().toggle_bookmark(1).unwrap();
        app.refresh();
        let result = app.cut_bookmarked_lines(|text| {
            assert_eq!(text, "b\n");
            Err(io::Error::other("clipboard fixture"))
        });
        assert!(result.is_err());
        assert_eq!(app.document().text(), "a\nc");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\nb\nc");
        app.document_mut().toggle_bookmark(1).unwrap();
        app.cut_bookmarked_lines(|text| {
            assert_eq!(text, "b\n");
            Ok(())
        })
        .unwrap();
        assert_eq!(app.document().text(), "a\nc");
    }

    #[test]
    fn marked_line_copy_and_delete_commands_preserve_endings_and_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\nb\nc").unwrap();
        app.document_mut().toggle_bookmark(1).unwrap();
        app.document_mut().toggle_bookmark(2).unwrap();
        app.refresh();
        app.copy_bookmarked_lines(|text| {
            assert_eq!(text, "b\nc");
            Ok(())
        })
        .unwrap();
        app.command(IDM_SEARCH_DELETEMARKEDLINES);
        assert_eq!(app.document().text(), "a\r\n");
        assert_eq!(control_text(app.edit), "a\r\n");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\r\nb\nc");
        assert_eq!(app.document().bookmarked_lines().count(), 0);
        app.document_mut().toggle_bookmark(1).unwrap();
        app.command(IDM_SEARCH_DELETEUNMARKEDLINES);
        assert_eq!(app.document().text(), "b\n");
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![0]
        );
        app.document_mut().set_read_only(true);
        app.command(IDM_SEARCH_DELETEMARKEDLINES);
        assert_eq!(app.document().text(), "b\n");
        app.copy_bookmarked_lines(|text| {
            assert_eq!(text, "b\n");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn bookmark_commands_navigate_and_report_the_current_line() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\nb\r\nc").unwrap();
        app.refresh();
        app.command(IDM_SEARCH_TOGGLE_BOOKMARK);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![2]
        );
        assert!(control_text(app.status).contains("Bookmarked"));
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh_selection();
        app.command(IDM_SEARCH_TOGGLE_BOOKMARK);
        app.document_mut().set_read_only(true);
        app.command(IDM_SEARCH_NEXT_BOOKMARK);
        assert_eq!(app.document().selection().caret, 6);
        assert_eq!(control_selection(app.edit), (6, 6));
        app.command(IDM_SEARCH_NEXT_BOOKMARK);
        assert_eq!(app.document().selection().caret, 0);
        app.command(IDM_SEARCH_PREV_BOOKMARK);
        assert_eq!(app.document().selection().caret, 6);
        app.command(IDM_SEARCH_CLEAR_BOOKMARKS);
        assert_eq!(app.document().bookmarked_lines().count(), 0);
        assert!(!control_text(app.status).contains("Bookmarked"));
        app.command(IDM_SEARCH_INVERSEMARKS);
        assert_eq!(
            app.document().bookmarked_lines().collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(control_text(app.status).contains("Bookmarked"));
        app.command(IDM_SEARCH_INVERSEMARKS);
        assert_eq!(app.document().bookmarked_lines().count(), 0);
    }

    #[test]
    fn backward_find_overlaps_but_replace_keeps_its_incremental_origin() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("ababa").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 5,
                caret: 2,
            })
            .unwrap();
        app.refresh();
        app.search_text = "aba".into();
        app.command(IDM_SEARCH_FINDPREV);
        assert_eq!(app.document().selection().range(), 0..3);
        assert_eq!(control_selection(app.edit), (0, 3));
        app.command(IDM_SEARCH_FINDPREV);
        assert_eq!(app.document().selection().range(), 2..5);
        app.command(IDM_SEARCH_VOLATILE_FINDPREV);
        assert_eq!(app.document().selection().range(), 0..3);
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACE,
            needle: "aba".into(),
            replacement: "aba".into(),
            options: SearchOptions {
                direction: SearchDirection::Backward,
                ..SearchOptions::default()
            },
        });
        // processReplace uses FirstIncremental for its subsequent search too.
        assert_eq!(app.document().selection().range(), 0..3);
        assert_eq!(app.document().text(), "ababa");
    }

    #[test]
    fn replace_first_finds_inside_a_larger_selection_in_both_directions() {
        for (direction, expected) in [
            (SearchDirection::Forward, 0..4),
            (SearchDirection::Backward, 5..9),
        ] {
            let mut app = App::create(false).unwrap();
            app.drain();
            app.document_mut().insert("word word tail").unwrap();
            app.document_mut()
                .select(Selection {
                    anchor: 9,
                    caret: 0,
                })
                .unwrap();
            app.refresh();
            app.search_options.wrap = false;
            app.search_request(Request {
                id: 0,
                flags: FR_REPLACE,
                needle: "word".into(),
                replacement: "X".into(),
                options: SearchOptions {
                    direction,
                    ..SearchOptions::default()
                },
            });
            assert_eq!(app.document().text(), "word word tail");
            assert_eq!(app.document().selection().range(), expected);
            // Merely locating a match must not add a history entry.
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), "");
        }
    }

    #[test]
    fn identical_single_replace_advances_and_restores_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀 🦀").unwrap();
        app.document_mut()
            .save_with(std::path::Path::new("replace-fixture.txt"), |_, _| Ok(()))
            .unwrap();
        let before = Selection {
            anchor: 4,
            caret: 0,
        };
        app.document_mut().select(before).unwrap();
        app.refresh();
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACE,
            needle: "🦀".into(),
            replacement: "🦀".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "🦀 🦀");
        assert_eq!(app.document().selection().range(), 5..9);
        assert_eq!(control_selection(app.edit), (3, 5));
        assert!(app.document().is_dirty());
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "🦀 🦀");
        assert_eq!(app.document().selection(), before);
        assert!(!app.document().is_dirty());
        app.command(IDM_EDIT_REDO);
        assert_eq!(app.document().selection().range(), 5..9);
        assert!(app.document().is_dirty());
    }

    #[test]
    fn identical_replace_all_can_undo_its_selection_change() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("word word").unwrap();
        app.document_mut()
            .save_with(
                std::path::Path::new("save-point-fixture.txt"),
                |_, _| Ok(()),
            )
            .unwrap();
        assert!(!app.document().is_dirty());
        let before = Selection {
            anchor: 2,
            caret: 2,
        };
        app.document_mut().select(before).unwrap();
        app.refresh();
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "word".into(),
            replacement: "word".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "word word");
        assert!(app.document().is_dirty());
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 0,
                caret: 0
            }
        );
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "word word");
        assert!(!app.document().is_dirty());
        assert_eq!(app.document().selection(), before);
        app.command(IDM_EDIT_REDO);
        assert_eq!(app.document().text(), "word word");
        assert!(app.document().is_dirty());
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 0,
                caret: 0
            }
        );
    }

    #[test]
    fn whole_document_replace_all_preserves_adjusted_reverse_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a xx b xx c").unwrap();
        let before = Selection {
            anchor: 9,
            caret: 2,
        };
        app.document_mut().select(before).unwrap();
        app.refresh();
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "xx".into(),
            replacement: "QQQ".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "a QQQ b QQQ c");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 8,
                caret: 5
            }
        );
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), before);
        assert_eq!(app.document().text(), "a xx b xx c");
    }

    #[test]
    fn replace_all_reselects_adjusted_scope_and_clears_empty_selection_mode() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("x ß ss y").unwrap();
        let before = Selection {
            anchor: 7,
            caret: 2,
        };
        app.document_mut().select(before).unwrap();
        app.refresh();
        app.command(SEARCH_IN_SELECTION);
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "ss".into(),
            replacement: "long".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "x long long y");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 2,
                caret: 11
            }
        );
        assert!(app.search_in_selection);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), before);
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACEALL,
            needle: "ß ss".into(),
            replacement: "".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "x  y");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 2,
                caret: 2
            }
        );
        assert!(!app.search_in_selection);
        // SAFETY: Inspect the menu belonging to this test's window.
        unsafe {
            assert_eq!(
                GetMenuState(
                    crate::plugin_host::main_menu(app.window),
                    SEARCH_IN_SELECTION,
                    MF_BYCOMMAND
                ) & MF_CHECKED,
                0
            );
        }
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), before);
        assert_eq!(app.document().text(), "x ß ss y");
    }

    #[test]
    fn count_and_replace_all_honor_direction_without_wrap() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a a a a").unwrap();
        let selection = Selection {
            anchor: 5,
            caret: 2,
        };
        app.document_mut().select(selection).unwrap();
        app.search_text = "a".into();
        app.refresh();
        app.command(SEARCH_WRAP);
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 3 occurrences");
        app.search_options.direction = SearchDirection::Backward;
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 3 occurrences");
        app.command(SEARCH_IN_SELECTION);
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 2 occurrences");
        app.command(SEARCH_IN_SELECTION);
        for (direction, expected) in [
            (SearchDirection::Forward, "a x x x"),
            (SearchDirection::Backward, "x x x a"),
        ] {
            app.search_request(Request {
                id: 0,
                flags: FR_REPLACEALL,
                needle: "a".into(),
                replacement: "x".into(),
                options: SearchOptions {
                    direction,
                    ..Default::default()
                },
            });
            assert_eq!(app.document().text(), expected);
            assert!(!app.search_options.wrap);
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().selection(), selection);
        }
        app.command(SEARCH_WRAP);
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 4 occurrences");
    }

    #[test]
    fn count_reads_live_dialog_query_without_moving_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a a A").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.search_text = "missing".into();
        app.show_search(false);
        let selection = app.document().selection();
        let dialog = app.search_dialog.as_ref().unwrap().hwnd;
        // SAFETY: Test owns the live common dialog and addresses standard controls.
        unsafe {
            SetDlgItemTextW(dialog, edt1 as i32, wide("a").as_ptr());
            CheckDlgButton(dialog, chx2 as i32, BST_CHECKED);
        }
        app.command(SEARCH_COUNT);
        assert_eq!(control_text(app.status), "Count: 2 occurrences");
        assert_eq!(app.document().selection(), selection);
        assert_eq!(app.search_text, "a");
    }

    #[test]
    fn duplicate_removal_commands_share_native_undo_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\na\r\nb\r\na").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(IDM_EDIT_REMOVE_CONSECUTIVE_DUP_LINES);
        assert_eq!(control_text(app.edit), "a\r\nb\r\na");
        app.command(IDM_EDIT_REMOVE_ANY_DUP_LINES);
        assert_eq!(control_text(app.edit), "a\r\nb");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "a\r\nb\r\na");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "a\r\na\r\nb\r\na");
    }

    #[test]
    fn whitespace_commands_update_native_text_and_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀\tx\n    y").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(IDM_EDIT_TAB2SW);
        assert_eq!(control_text(app.edit), "🦀   x\r\n    y");
        app.command(IDM_EDIT_SW2TAB_LEADING);
        assert_eq!(control_text(app.edit), "🦀   x\r\n\ty");
        app.command(IDM_EDIT_SW2TAB_ALL);
        assert_eq!(control_text(app.edit), "🦀\tx\r\n\ty");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "🦀   x\r\n\ty");
    }

    #[test]
    fn native_tab_and_indent_menu_share_core_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\nb\r\nc").unwrap();
        app.refresh();
        // SAFETY: This test owns the edit control and sends synchronous messages.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 0, 6);
            SendMessageW(app.edit, WM_CHAR, VK_TAB as usize, 0);
        }
        app.drain();
        assert_eq!(control_text(app.edit), "\ta\r\n\tb\r\nc");
        assert_eq!(control_selection(app.edit), (0, 8));
        app.command(BACK_TAB);
        assert_eq!(control_text(app.edit), "a\r\nb\r\nc");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "\ta\r\n\tb\r\nc");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "a\r\nb\r\nc");
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 1, 1);
        }
        app.command(IDM_EDIT_INS_TAB);
        assert_eq!(control_text(app.edit), "\ta\r\nb\r\nc");
        assert_eq!(control_selection(app.edit), (2, 2));
        app.command(IDM_EDIT_RMV_TAB);
        assert_eq!(control_text(app.edit), "a\r\nb\r\nc");
        assert_eq!(control_selection(app.edit), (1, 1));
    }

    #[test]
    fn native_line_delete_and_transpose_preserve_endings_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("first\r\n🦀\rlast").unwrap();
        app.refresh();
        // SAFETY: This test owns the control; position 9 follows the surrogate pair.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 9, 9);
        }
        app.command(TRANSPOSE_LINE);
        assert_eq!(app.document().text(), "🦀\r\nfirst\rlast");
        assert_eq!(control_selection(app.edit), (4, 4));
        app.command(DELETE_LINE);
        assert_eq!(app.document().text(), "🦀\r\nlast");
        assert_eq!(control_text(app.edit), "🦀\r\nlast");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "🦀\r\nfirst\rlast");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "first\r\n🦀\rlast");
        assert_eq!(control_selection(app.edit), (9, 9));
    }

    #[test]
    fn search_wrap_feedback_tracks_direction_and_clears_on_next_match() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("é x é").unwrap();
        app.search_text = "é".to_owned();
        app.refresh();
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 0..2);
        assert_eq!(
            control_text(app.status),
            "Reached end of page, continued from top"
        );
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 5..7);
        assert!(!control_text(app.status).contains("Reached"));
        app.find(SearchDirection::Backward);
        assert_eq!(app.document().selection().range(), 0..2);
        app.find(SearchDirection::Backward);
        assert_eq!(
            control_text(app.status),
            "Reached top of page, continued from bottom"
        );
        app.search_options.wrap = false;
        app.find(SearchDirection::Forward);
        assert_eq!(control_text(app.status), "No match found");
        assert_eq!(app.document().selection().range(), 5..7);
        app.command(IDM_SEARCH_VOLATILE_FINDNEXT);
        assert_eq!(
            control_text(app.status),
            "Reached end of page, continued from top"
        );
        app.command(IDM_SEARCH_VOLATILE_FINDPREV);
        assert_eq!(
            control_text(app.status),
            "Reached top of page, continued from bottom"
        );
    }

    #[test]
    fn set_and_find_saves_literal_query_and_respects_threshold() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("\\n \\n\n").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: 2,
            })
            .unwrap();
        app.search_options.mode = SearchMode::Extended;
        app.search_options.match_case = true;
        app.refresh();
        app.show_search(false);
        app.command(IDM_SEARCH_SETANDFINDNEXT);
        assert_eq!(app.document().selection().range(), 3..5);
        assert_eq!(app.search_text, "\\n");
        assert_eq!(app.search_options.mode, SearchMode::Extended);
        assert!(app.search_options.match_case);
        assert_eq!(
            app.search_dialog
                .as_ref()
                .unwrap()
                .current_query()
                .unwrap()
                .needle,
            "\\n"
        );
        app.command(IDM_SEARCH_SETANDFINDPREV);
        assert_eq!(app.document().selection().range(), 0..2);
        app.command(IDM_FILE_NEW);
        let large = "é".repeat(1025);
        app.document_mut().insert(&large).unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: large.len(),
            })
            .unwrap();
        app.refresh();
        app.command(IDM_SEARCH_SETANDFINDNEXT);
        assert_eq!(app.search_text, "\\n");
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: 2048,
            })
            .unwrap();
        app.refresh();
        app.command(IDM_SEARCH_SETANDFINDNEXT);
        assert_eq!(app.search_text.chars().count(), 1024);
    }

    #[test]
    fn temporary_selected_search_expands_word_and_keeps_saved_options() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut()
            .insert("e\u{301} E\u{301} e\u{301}")
            .unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 1,
                caret: 1,
            })
            .unwrap();
        app.document_mut().set_read_only(true);
        app.search_text = "unchanged".to_owned();
        app.search_options.match_case = true;
        app.search_options.whole_word = true;
        app.search_options.wrap = false;
        app.search_options.mode = SearchMode::Extended;
        app.refresh();
        app.command(IDM_SEARCH_VOLATILE_FINDNEXT);
        assert_eq!(app.document().selection().range(), 4..7);
        app.command(IDM_SEARCH_VOLATILE_FINDPREV);
        assert_eq!(app.document().selection().range(), 0..3);
        app.command(IDM_SEARCH_VOLATILE_FINDPREV);
        assert_eq!(app.document().selection().range(), 8..11);
        assert_eq!(app.search_text, "unchanged");
        assert!(app.search_options.match_case && app.search_options.whole_word);
        assert!(!app.search_options.wrap);
        assert_eq!(app.search_options.mode, SearchMode::Extended);
    }

    #[test]
    fn search_goto_braces_and_selection_gesture_do_not_reset_native_text() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let text = "{🦀}\nneedle needle";
        app.document_mut().insert(text).unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.search_text = "needle".to_owned();
        // SAFETY: This test owns the edit and uses its native modify flag as a
        // sentinel for accidental WM_SETTEXT calls; core dirty state is separate.
        unsafe {
            SendMessageW(app.edit, EM_SETMODIFY, 1, 0);
        }
        let projection = app.projection.text.as_ptr();
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 7..13);
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 14..20);
        enqueue(Event::GoTo(1, false));
        app.drain();
        assert_eq!(app.document().selection().caret, 0);
        app.command(IDM_SEARCH_GOTOMATCHINGBRACE);
        app.command(IDM_SEARCH_SELECTMATCHINGBRACES);
        app.command(IDM_EDIT_BEGINENDSELECT);
        enqueue(Event::GoTo(7, true));
        app.drain();
        app.command(IDM_EDIT_BEGINENDSELECT);
        assert_eq!(app.projection.text.as_ptr(), projection);
        assert_eq!(app.document().text(), text);
        assert_eq!(control_text(app.edit), app.projection.text);
        unsafe {
            assert_ne!(SendMessageW(app.edit, EM_GETMODIFY, 0, 0), 0);
        }
    }

    #[test]
    fn navigation_keeps_native_text_buffer_and_publishes_caret_queries() {
        use npp_compat::messages::*;
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut()
            .insert("  someValue 🦀\n\nlastLine")
            .unwrap();
        app.refresh();
        let projection_address = app.projection.text.as_ptr();
        // SAFETY: The modify marker belongs to this test's edit, not the core savepoint.
        unsafe {
            SendMessageW(app.edit, EM_SETMODIFY, 1, 0);
        }
        for command in [
            DOCUMENT_START,
            WORD_RIGHT,
            WORD_RIGHT_EXTEND,
            SMART_HOME,
            SMART_HOME_EXTEND,
            LINE_END,
            LINE_END_EXTEND,
            WORD_PART_RIGHT,
            WORD_PART_LEFT_EXTEND,
            PARAGRAPH_DOWN,
            PARAGRAPH_UP_EXTEND,
            DOCUMENT_END_EXTEND,
        ] {
            app.command(command);
            assert_eq!(app.projection.text.as_ptr(), projection_address);
            assert_eq!(control_text(app.edit), app.projection.text);
            // WM_SETTEXT resets this marker; navigation must not send it.
            unsafe {
                assert_ne!(SendMessageW(app.edit, EM_GETMODIFY, 0, 0), 0);
            }
            let (line, column) = app
                .document()
                .line_column(app.document().selection().caret, app.tab_width)
                .unwrap();
            unsafe {
                assert_eq!(
                    SendMessageW(app.window, NPPM_GETCURRENTLINE, 0, 0),
                    line as isize
                );
                assert_eq!(
                    SendMessageW(app.window, NPPM_GETCURRENTCOLUMN, 0, 0),
                    column as isize
                );
            }
        }
    }

    #[test]
    fn native_word_part_navigation_preserves_reverse_anchor() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("é🦀someValue").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.command(WORD_PART_LEFT_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 15,
                caret: 10
            }
        );
        app.command(WORD_PART_LEFT_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 15,
                caret: 6
            }
        );
        assert_eq!(control_selection(app.edit), (3, 12));
        app.command(WORD_PART_LEFT);
        assert_eq!(app.document().selection(), Selection::default());
        app.command(WORD_PART_RIGHT);
        assert_eq!(control_selection(app.edit), (3, 3));
        app.command(WORD_PART_RIGHT_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 6,
                caret: 10
            }
        );
    }

    #[test]
    fn native_paragraph_navigation_projects_mixed_endings_and_reverse_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀\r\n \t\rb\n\nc").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.command(DOCUMENT_START);
        app.command(PARAGRAPH_DOWN_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 0,
                caret: 9
            }
        );
        assert_eq!(control_selection(app.edit), (0, 8));
        app.command(PARAGRAPH_DOWN);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 12,
                caret: 12
            }
        );
        app.command(PARAGRAPH_UP_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 12,
                caret: 9
            }
        );
        app.command(PARAGRAPH_UP_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 12,
                caret: 0
            }
        );
        app.command(PARAGRAPH_UP);
        assert_eq!(app.document().selection(), Selection::default());
        assert_eq!(app.document().text(), "🦀\r\n \t\rb\n\nc");
    }

    #[test]
    fn native_end_and_document_navigation_preserve_unicode_selection() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a🦀\nb\rc").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.command(DOCUMENT_START_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 9,
                caret: 0
            }
        );
        app.command(LINE_END_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 9,
                caret: 5
            }
        );
        assert_eq!(control_selection(app.edit), (3, 9));
        app.command(LINE_END);
        assert_eq!(control_selection(app.edit), (3, 3));
        app.command(DOCUMENT_END_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 5,
                caret: 9
            }
        );
        app.command(DOCUMENT_START);
        assert_eq!(app.document().selection(), Selection::default());
        app.command(DOCUMENT_END);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 9,
                caret: 9
            }
        );
        assert_eq!(app.document().text(), "a🦀\nb\rc");
    }

    #[test]
    fn native_home_toggles_indentation_and_preserves_anchor() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("first\n \t🦀 tail").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        let anchor = app.document().selection().caret;
        app.command(SMART_HOME_EXTEND);
        assert_eq!(app.document().selection(), Selection { anchor, caret: 8 });
        assert_eq!(control_selection(app.edit), (9, 16));
        app.command(SMART_HOME_EXTEND);
        assert_eq!(app.document().selection(), Selection { anchor, caret: 6 });
        app.command(SMART_HOME);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 8,
                caret: 8
            }
        );
        app.command(SMART_HOME);
        assert_eq!(control_selection(app.edit), (7, 7));
        assert_eq!(app.document().text(), "first\n \t🦀 tail");
    }

    #[test]
    fn backward_word_selection_survives_commands_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("one 🦀 two").unwrap();
        app.refresh();
        app.command(WORD_LEFT_EXTEND);
        app.command(WORD_LEFT_EXTEND);
        let selection = Selection {
            anchor: 12,
            caret: 4,
        };
        assert_eq!(app.document().selection(), selection);
        app.command(DELETE_WORD_LEFT);
        assert_eq!(app.document().text(), "🦀 two");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 8,
                caret: 0
            }
        );
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), selection);
        app.command(DELETE_WORD_RIGHT);
        assert_eq!(app.document().text(), "one two");
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 4,
                caret: 4
            }
        );
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), selection);
        app.command(IDM_EDIT_UPPERCASE);
        assert_eq!(app.document().text(), "one 🦀 TWO");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().selection(), selection);
        app.command(IDM_EDIT_BEGINENDSELECT);
        app.command(WORD_RIGHT);
        app.command(IDM_EDIT_BEGINENDSELECT);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 4,
                caret: 9
            }
        );
    }

    #[test]
    fn native_word_navigation_retains_reverse_selection_after_key_up() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("one 🦀 two").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        for caret in [9, 4, 0] {
            app.command(WORD_LEFT_EXTEND);
            assert_eq!(app.document().selection(), Selection { anchor: 12, caret });
            // SAFETY: Dispatch a key release to our owned edit control.
            unsafe {
                SendMessageW(app.edit, WM_KEYUP, VK_LEFT as usize, 0);
            }
            app.drain();
            assert_eq!(app.document().selection(), Selection { anchor: 12, caret });
        }
        app.command(WORD_RIGHT_EXTEND);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 12,
                caret: 4
            }
        );
        app.command(WORD_RIGHT);
        assert_eq!(
            app.document().selection(),
            Selection {
                anchor: 9,
                caret: 9
            }
        );
        assert_eq!(control_selection(app.edit), (7, 7));
        assert_eq!(app.document().text(), "one 🦀 two");
    }

    #[test]
    fn native_word_deletion_uses_core_boundaries_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("e\u{301} 🦀+ tail").unwrap();
        app.refresh();
        // SAFETY: Position 3 is after the combining sequence and its space.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 3, 3);
        }
        app.command(DELETE_WORD_RIGHT);
        assert_eq!(app.document().text(), "e\u{301} tail");
        assert_eq!(control_selection(app.edit), (3, 3));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "e\u{301} 🦀+ tail");
        app.command(DELETE_WORD_LEFT);
        assert_eq!(app.document().text(), "🦀+ tail");
        app.command(IDM_EDIT_UNDO);
        app.document_mut().set_read_only(true);
        app.refresh();
        app.command(DELETE_WORD_LEFT);
        app.command(DELETE_WORD_RIGHT);
        assert_eq!(app.document().text(), "e\u{301} 🦀+ tail");
    }

    #[test]
    fn native_line_boundary_deletion_uses_utf16_caret_and_core_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\n🦀xy\rz").unwrap();
        app.refresh();
        // SAFETY: The test owns the native edit control; 5 follows the surrogate pair.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 0, 5);
        }
        app.command(DELETE_LINE_RIGHT);
        assert_eq!(app.document().text(), "a\r\n🦀\rz");
        assert_eq!(control_selection(app.edit), (5, 5));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_selection(app.edit), (0, 5));
        app.command(DELETE_LINE_LEFT);
        assert_eq!(app.document().text(), "a\r\nxy\rz");
        assert_eq!(control_selection(app.edit), (0, 3));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\r\n🦀xy\rz");
    }

    #[test]
    fn native_empty_line_commands_preserve_mode_difference_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\n\r\n \t\n🦀\n").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(IDM_EDIT_REMOVEEMPTYLINES);
        assert_eq!(app.document().text(), "a\r\n \t\n🦀");
        app.command(IDM_EDIT_REMOVEEMPTYLINESWITHBLANK);
        assert_eq!(control_text(app.edit), "a\r\n🦀");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\r\n \t\n🦀");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\r\n\r\n \t\n🦀\n");
    }

    #[test]
    fn native_join_lines_expands_selection_and_preserves_final_ending() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("🦀\r\n  b\rc").unwrap();
        app.refresh();
        // SAFETY: Test owns this edit; position 9 starts the third projected line.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 2, 9);
        }
        app.command(IDM_EDIT_JOIN_LINES);
        assert_eq!(app.document().text(), "🦀   b\rc");
        assert_eq!(control_text(app.edit), "🦀   b\r\nc");
        assert_eq!(control_selection(app.edit), (0, 6));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "🦀\r\n  b\rc");
        assert_eq!(control_selection(app.edit), (2, 9));
    }

    #[test]
    fn native_blank_line_commands_move_caret_and_undo() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\r\n🦀\r\nc").unwrap();
        app.refresh();
        // SAFETY: Test owns the edit control; 5 follows the surrogate pair.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 0, 5);
        }
        app.command(IDM_EDIT_BLANKLINEABOVECURRENT);
        assert_eq!(control_text(app.edit), "a\r\n\r\n🦀\r\nc");
        assert_eq!(control_selection(app.edit), (3, 3));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_selection(app.edit), (0, 5));
        app.command(IDM_EDIT_BLANKLINEBELOWCURRENT);
        assert_eq!(control_text(app.edit), "a\r\n🦀\r\n\r\nc");
        assert_eq!(control_selection(app.edit), (7, 7));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "a\r\n🦀\r\nc");
    }

    #[test]
    fn native_enter_uses_configured_ending_and_basic_indent() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.workspace
            .add(Document::from_bytes(b"  abc\nlast").unwrap());
        app.refresh();
        app.command(BASIC_AUTO_INDENT);
        // SAFETY: The test owns the edit control and uses synchronous messages.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 5, 5);
            SendMessageW(app.edit, WM_CHAR, VK_RETURN as usize, 0);
        }
        app.drain();
        assert_eq!(app.document().text(), "  abc\n  \nlast");
        assert_eq!(control_selection(app.edit), (9, 9));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "  abc\nlast");
        assert_eq!(control_selection(app.edit), (5, 5));
        app.command(BASIC_AUTO_INDENT);
        app.command(INSERT_NEWLINE);
        assert_eq!(app.document().text(), "  abc\n\nlast");
    }

    #[test]
    fn native_move_lines_keeps_block_selected_and_undoes() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.workspace
            .add(Document::from_bytes("a\n🦀\nb".as_bytes()).unwrap());
        app.refresh();
        // SAFETY: The test owns this native edit control.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 3, 5);
        }
        app.command(IDM_EDIT_LINE_DOWN);
        assert_eq!(app.document().text(), "a\nb\n🦀");
        assert_eq!(control_selection(app.edit), (6, 8));
        app.command(IDM_EDIT_LINE_UP);
        assert_eq!(app.document().text(), "a\n🦀\nb");
        assert_eq!(control_selection(app.edit), (3, 7));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\nb\n🦀");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "a\n🦀\nb");
        assert_eq!(control_selection(app.edit), (3, 5));
    }

    #[test]
    fn readonly_commands_are_ignored_and_menu_state_follows_active_buffer() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("\t abcdef\r\nnext").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 2,
                caret: 2,
            })
            .unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        let before = app.document().text().to_owned();
        for &command in EDIT_COMMANDS {
            app.command(command);
            assert_eq!(app.document().text(), before, "command {command}");
            assert_eq!(
                app.document().selection(),
                Selection {
                    anchor: 2,
                    caret: 2
                }
            );
        }
        // Back-tab within indentation fails silently; within text it can navigate.
        app.document_mut()
            .select(Selection {
                anchor: 1,
                caret: 1,
            })
            .unwrap();
        app.refresh();
        app.command(BACK_TAB);
        assert_eq!(app.document().text(), before);
        app.document_mut()
            .select(Selection {
                anchor: 6,
                caret: 6,
            })
            .unwrap();
        app.refresh();
        app.command(BACK_TAB);
        assert!(app.document().selection().caret < 6);
        // SAFETY: The test owns the window and its menu.
        unsafe {
            assert_ne!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_DUP_LINE, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_COPY, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
        }
        app.command(IDM_FILE_NEW);
        unsafe {
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_DUP_LINE, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
            assert_ne!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_UNDO, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
        }
        app.document_mut().insert("x").unwrap();
        app.refresh();
        unsafe {
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_UNDO, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
        }
        app.command(IDM_EDIT_UNDO);
        unsafe {
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_REDO, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
        }
    }

    #[test]
    fn replace_dialog_tracks_readonly_buffers_and_rejects_queued_mutations() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("abc abc").unwrap();
        app.document_mut().set_read_only(true);
        app.document_mut().select(Selection::default()).unwrap();
        app.search_text = "abc".into();
        app.refresh();
        app.show_search(true);
        let selection = app.document().selection();
        let dialog = app.search_dialog.as_ref().unwrap().hwnd;
        // SAFETY: This test owns the dialog and its standard child controls.
        unsafe {
            for id in [psh1, psh2] {
                assert_eq!(IsWindowEnabled(GetDlgItem(dialog, id as i32)), 0);
            }
            SetWindowTextW(GetDlgItem(dialog, edt1 as i32), wide("ab").as_ptr());
            for id in [psh1, psh2] {
                assert_eq!(IsWindowEnabled(GetDlgItem(dialog, id as i32)), 0);
            }
            assert_ne!(IsWindowEnabled(GetDlgItem(dialog, IDOK)), 0);
        }
        for flags in [FR_REPLACE, FR_REPLACEALL] {
            app.search_request(Request {
                id: 0,
                flags,
                needle: "abc".into(),
                replacement: "x".into(),
                options: SearchOptions::default(),
            });
            assert_eq!(app.document().text(), "abc abc");
            assert_eq!(app.document().selection(), selection);
        }
        app.command(IDM_FILE_NEW);
        unsafe {
            for id in [psh1, psh2] {
                assert_ne!(IsWindowEnabled(GetDlgItem(dialog, id as i32)), 0);
            }
            SetWindowTextW(GetDlgItem(dialog, edt1 as i32), wide("").as_ptr());
            for id in [psh1, psh2] {
                assert_eq!(IsWindowEnabled(GetDlgItem(dialog, id as i32)), 0);
            }
            SetWindowTextW(GetDlgItem(dialog, edt1 as i32), wide("abc").as_ptr());
            SetWindowTextW(GetDlgItem(dialog, edt2 as i32), wide("x").as_ptr());
        }
        app.document_mut().insert("abc abc").unwrap();
        app.refresh();
        unsafe {
            SendMessageW(GetDlgItem(dialog, psh2 as i32), BM_CLICK, 0, 0);
        }
        app.drain();
        assert_eq!(app.document().text(), "x x");
    }

    #[test]
    fn user_readonly_commands_preserve_file_protection_and_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("abc").unwrap();
        let first = app.active_id();
        app.command(IDM_EDIT_TOGGLEREADONLY);
        assert!(app.document().is_user_read_only());
        assert!(app.document().is_dirty());
        // SAFETY: The test owns the window and its native controls.
        unsafe {
            assert_ne!(
                GetMenuState(GetMenu(app.window), IDM_EDIT_TOGGLEREADONLY, MF_BYCOMMAND)
                    & MF_CHECKED,
                0
            );
            assert_ne!(GetWindowLongW(app.edit, GWL_STYLE) & ES_READONLY, 0);
        }
        app.command(IDM_EDIT_TOGGLEREADONLY);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "");
        app.command(IDM_FILE_NEW);
        app.document_mut().set_file_read_only(true);
        app.command(IDM_EDIT_SETREADONLYFORALLDOCS);
        assert!(
            app.workspace
                .buffers()
                .all(|(_, doc)| doc.is_user_read_only())
        );
        app.command(IDM_EDIT_CLEARREADONLYFORALLDOCS);
        assert!(
            app.workspace
                .buffers()
                .all(|(_, doc)| !doc.is_user_read_only())
        );
        assert!(app.document().is_read_only());
        assert!(!app.workspace.get(first).unwrap().is_read_only());
        app.command(IDM_EDIT_TOGGLEREADONLY);
        app.command(IDM_EDIT_TOGGLEREADONLY);
        assert!(app.document().is_file_read_only());
    }

    #[test]
    fn file_readonly_command_updates_buffer_without_clearing_user_protection() {
        let path = std::env::temp_dir().join(format!("npp-attribute-{}.txt", std::process::id()));
        std::fs::write(&path, b"content").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.command(IDM_EDIT_TOGGLESYSTEMREADONLY); // Untitled is a no-op.
        app.open(&path).unwrap();
        app.command(IDM_EDIT_TOGGLEREADONLY);
        app.command(IDM_EDIT_TOGGLESYSTEMREADONLY);
        let protected = app.document().is_file_read_only();
        let disk_protected = std::fs::metadata(&path).unwrap().permissions().readonly();
        app.command(IDM_EDIT_TOGGLESYSTEMREADONLY);
        assert!(protected && disk_protected);
        assert!(!app.document().is_file_read_only());
        assert!(app.document().is_user_read_only());
        assert!(app.document().is_read_only());
        assert!(!app.document().is_dirty());
        assert!(!app.document().can_undo());
        assert_eq!(std::fs::read(&path).unwrap(), b"content");
        drop(app);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn asynchronous_attribute_refresh_preserves_unsaved_text_and_ignores_stale_paths() {
        let path =
            std::env::temp_dir().join(format!("npp-attribute-refresh-{}.txt", std::process::id()));
        std::fs::write(&path, b"disk").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&path).unwrap();
        app.document_mut().insert("unsaved ").unwrap();
        app.refresh();
        crate::file_io::toggle_file_read_only(&path).unwrap();
        // Queue the same event as WM_ACTIVATEAPP; the worker owns only paths.
        enqueue(Event::CheckFileAttributes);
        app.drain();
        let receiver = app.attribute_check.take().unwrap();
        let states = receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        crate::file_io::toggle_file_read_only(&path).unwrap(); // Restore fixture before assertions.
        let selection = control_selection(app.edit);
        app.apply_file_attributes(states);
        assert!(app.document().is_file_read_only());
        assert_eq!(app.document().text(), "unsaved disk");
        assert!(app.document().is_dirty());
        assert_eq!(control_selection(app.edit), selection);
        app.apply_file_attributes(vec![crate::file_attributes::AttributeState {
            id: app.active_id(),
            path: path.with_extension("stale"),
            read_only: Some(false),
            disk: crate::file_attributes::DiskState::Unchanged,
        }]);
        assert!(app.document().is_file_read_only());
        enqueue(Event::CheckFileAttributes);
        app.drain();
        let states = app
            .attribute_check
            .take()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.apply_file_attributes(states);
        assert!(!app.document().is_file_read_only());
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "disk");
        drop(app);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn background_disk_status_distinguishes_edits_deletion_and_recovery() {
        use crate::file_attributes::DiskState;
        let path = std::env::temp_dir().join(format!("npp-disk-status-{}.txt", std::process::id()));
        std::fs::write(&path, b"disk").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&path).unwrap();
        app.document_mut().insert("unsaved ").unwrap();
        app.refresh();
        let check = |app: &mut App| {
            let paths = vec![(
                app.active_id(),
                app.document().path().unwrap().to_owned(),
                app.document().saved_fingerprint(),
            )];
            let states = crate::file_attributes::start(app.window, paths)
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            app.apply_file_attributes(states);
        };
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Unchanged);
        std::fs::write(&path, b"DISK").unwrap(); // Same length; detect byte changes.
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Changed);
        assert!(control_text(app.status).contains("File changed on disk"));
        assert_eq!(app.document().text(), "unsaved disk");
        assert!(app.document().is_dirty());
        std::fs::remove_file(&path).unwrap();
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Deleted);
        assert_eq!(app.document().text(), "unsaved disk");
        std::fs::create_dir(&path).unwrap();
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Unavailable);
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"restored").unwrap();
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Changed);
        app.reload().unwrap();
        assert_eq!(app.document().text(), "restored");
        assert!(!app.disk_states.contains_key(&app.active_id()));
        check(&mut app);
        assert_eq!(app.disk_states[&app.active_id()], DiskState::Unchanged);
        drop(app);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn deleted_clean_file_gets_dirty_indicator_and_close_protection() {
        let path =
            std::env::temp_dir().join(format!("npp-deleted-clean-{}.txt", std::process::id()));
        std::fs::write(&path, b"only copy").unwrap();
        let mut app = App::create(false).unwrap();
        app.drain();
        app.open(&path).unwrap();
        assert!(!app.document().is_dirty());
        std::fs::remove_file(&path).unwrap();
        let paths = vec![(
            app.active_id(),
            app.document().path().unwrap().to_owned(),
            app.document().saved_fingerprint(),
        )];
        let states = crate::file_attributes::start(app.window, paths)
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.apply_file_attributes(states);
        assert!(app.document().is_dirty());
        assert!(control_text(app.window).starts_with('*'));
        assert!(control_text(app.status).contains("File deleted on disk"));
        assert!(matches!(
            app.workspace.close(app.active_id(), false),
            Err(npp_editor_core::CloseError::UnsavedChanges)
        ));
        assert_eq!(app.document().text(), "only copy");
        assert!(!app.document().can_undo());
    }

    #[test]
    fn zoom_changes_native_geometry_without_editing_and_restores_stock_font() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("\tx\r\n    x").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 1,
                caret: 1,
            })
            .unwrap();
        app.refresh();
        // SAFETY: The test owns these windows; messages return scalar positions/fonts.
        unsafe {
            let original_font = SendMessageW(app.edit, WM_GETFONT, 0, 0);
            let original_x = SendMessageW(app.edit, EM_POSFROMCHAR, 1, 0) as u16;
            app.set_zoom(10).unwrap();
            let enlarged_x = SendMessageW(app.edit, EM_POSFROMCHAR, 1, 0) as u16;
            assert!(enlarged_x > original_x);
            let spaces_x = SendMessageW(app.edit, EM_POSFROMCHAR, 8, 0) as u16;
            assert_eq!(enlarged_x, spaces_x);
            assert_eq!(control_selection(app.edit), (1, 1));
            assert_eq!(app.document().text(), "\tx\r\n    x");
            app.command(IDM_VIEW_ZOOMOUT);
            assert_eq!(app.zoom, 9);
            app.command(IDM_VIEW_ZOOMIN);
            assert_eq!(app.zoom, 10);
            app.command(IDM_VIEW_ZOOMRESTORE);
            assert_eq!(SendMessageW(app.edit, WM_GETFONT, 0, 0), original_font);
            assert_eq!(
                SendMessageW(app.edit, EM_POSFROMCHAR, 1, 0) as u16,
                original_x
            );
        }
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "");
    }

    #[test]
    fn control_wheel_accumulates_partial_deltas_and_zooms_once_per_message() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("text").unwrap();
        app.refresh();
        let send = |app: &mut App, delta: i16, modifiers: u32| {
            let wp = ((delta as u16 as usize) << 16) | modifiers as usize;
            // SAFETY: Test owns the edit; wheel message has only scalar payloads.
            unsafe {
                SendMessageW(app.edit, WM_MOUSEWHEEL, wp, 0);
            }
            app.drain();
        };
        let mut lines = 0u32;
        unsafe {
            SystemParametersInfoW(SPI_GETWHEELSCROLLLINES, 0, &mut lines as *mut _ as _, 0);
        }
        // Respect a user's disabled wheel setting rather than changing system settings.
        if lines == 0 {
            send(&mut app, 120, MK_CONTROL);
            assert_eq!(app.zoom, 0);
            return;
        }
        send(&mut app, 60, MK_CONTROL);
        assert_eq!(app.zoom, 0);
        send(&mut app, 60, MK_CONTROL);
        assert_eq!(app.zoom, 1);
        send(&mut app, 360, MK_CONTROL);
        assert_eq!(app.zoom, 2);
        send(&mut app, -120, MK_CONTROL);
        assert_eq!(app.zoom, 1);
        send(&mut app, 120, 0);
        assert_eq!(app.zoom, 1);
        send(&mut app, 120, MK_CONTROL | MK_SHIFT);
        assert_eq!(app.zoom, 1);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "");
    }

    #[test]
    fn file_launch_uses_saved_identity_without_saving_unsaved_changes() {
        use crate::file_launch::Action;
        let mut app = App::create(false).unwrap();
        app.drain();
        app.launch_file_with(Action::Reveal, |_, _| {
            panic!("untitled documents cannot launch")
        })
        .unwrap();
        let path = std::env::current_exe().unwrap();
        app.document_mut().save_with(&path, |_, _| Ok(())).unwrap();
        app.document_mut().insert("unsaved changes").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        for action in [Action::Reveal, Action::DefaultViewer] {
            app.launch_file_with(action, |actual, mode| {
                assert_eq!(actual, path);
                assert_eq!(mode, action);
                Ok(())
            })
            .unwrap();
        }
        assert!(
            app.launch_file_with(Action::DefaultViewer, |_, _| Err(io::Error::other(
                "no association"
            )))
            .is_err()
        );
        assert_eq!(app.document().text(), "unsaved changes");
        assert!(app.document().is_dirty());
        assert!(app.document().can_undo());
        // SAFETY: Query a test-owned menu, without executing either shell action.
        unsafe {
            assert_eq!(
                GetMenuState(GetMenu(app.window), IDM_FILE_OPEN_FOLDER, MF_BYCOMMAND) & MF_GRAYED,
                0
            );
        }
        app.command(IDM_FILE_NEW);
        // SAFETY: Query a test-owned menu.
        unsafe {
            for command in [IDM_FILE_OPEN_FOLDER, IDM_FILE_OPEN_DEFAULT_VIEWER] {
                assert_ne!(
                    GetMenuState(GetMenu(app.window), command, MF_BYCOMMAND) & MF_GRAYED,
                    0
                );
            }
        }
    }

    #[test]
    fn file_path_clipboard_uses_active_identity_without_changing_document() {
        use crate::path_clipboard::Part;
        use std::ffi::OsStr;
        let mut app = App::create(false).unwrap();
        app.drain();
        app.command(IDM_FILE_NEW);
        for (part, expected) in [
            (Part::Full, "new 2"),
            (Part::Name, "new 2"),
            (Part::Directory, ""),
        ] {
            app.copy_file_path(part, |text| {
                assert_eq!(text, OsStr::new(expected));
                Ok(())
            })
            .unwrap();
        }
        app.document_mut()
            .save_with(Path::new(r"C:\folder\résumé🦀.txt"), |_, _| Ok(()))
            .unwrap();
        app.document_mut().insert("unsaved text").unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 2,
                caret: 5,
            })
            .unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        let selection = app.document().selection();
        for (part, expected) in [
            (Part::Full, r"C:\folder\résumé🦀.txt"),
            (Part::Name, "résumé🦀.txt"),
            (Part::Directory, r"C:\folder"),
        ] {
            app.copy_file_path(part, |text| {
                assert_eq!(text, OsStr::new(expected));
                Ok(())
            })
            .unwrap();
        }
        assert!(
            app.copy_file_path(Part::Full, |_| Err(io::Error::other("clipboard busy")))
                .is_err()
        );
        assert_eq!(app.document().text(), "unsaved text");
        assert_eq!(app.document().selection(), selection);
        assert!(app.document().is_dirty());
        assert!(app.document().can_undo());
        // SAFETY: Querying menu state of the test-owned window.
        unsafe {
            for command in [
                IDM_EDIT_FULLPATHTOCLIP,
                IDM_EDIT_FILENAMETOCLIP,
                IDM_EDIT_CURRENTDIRTOCLIP,
            ] {
                assert_eq!(
                    GetMenuState(GetMenu(app.window), command, MF_BYCOMMAND) & MF_GRAYED,
                    0
                );
            }
        }
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn line_clipboard_preserves_endings_and_does_not_cut_on_write_failure() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let original = "é😀\nx\rlast";
        app.document_mut().insert(original).unwrap();
        app.refresh();
        // Native projection expands LF to CRLF: offset 5 is the second line start.
        // SAFETY: Test owns the edit control and sets a valid native selection.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 1, 5);
        }
        app.line_clipboard(false, |text| {
            assert_eq!(text, "é😀\nx\r");
            Ok(())
        })
        .unwrap();
        assert_eq!(control_selection(app.edit), (1, 5));
        assert_eq!(app.document().text(), original);
        assert!(
            app.line_clipboard(true, |_| Err(io::Error::other("clipboard busy")))
                .is_err()
        );
        assert_eq!(app.document().text(), original);
        assert_eq!(control_selection(app.edit), (1, 5));
        app.document_mut().set_read_only(true);
        app.line_clipboard(true, |_| panic!("read-only cut must not publish"))
            .unwrap();
        app.line_clipboard(false, |text| {
            assert_eq!(text, "é😀\nx\r");
            Ok(())
        })
        .unwrap();
        app.document_mut().set_read_only(false);
        app.line_clipboard(true, |text| {
            assert_eq!(text, "é😀\nx\r");
            Ok(())
        })
        .unwrap();
        assert_eq!(app.document().text(), "last");
        assert_eq!(control_selection(app.edit), (0, 0));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), original);
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn goto_dialog_navigates_read_only_text_and_preserves_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("é😀\r\nlast").unwrap();
        app.document_mut().set_read_only(true);
        app.refresh();
        app.goto_dialog = Some(crate::goto_dialog::GoToDialog::new(app.window).unwrap());
        let hwnd = app.goto_dialog.as_ref().unwrap().handle();
        app.goto_dialog.as_ref().unwrap().update(app.document());
        // SAFETY: Test owns dialog and sends standard control messages synchronously.
        unsafe {
            assert!(control_text(GetDlgItem(hwnd, 203)).contains("Maximum: 2"));
            SetDlgItemTextW(hwnd, 204, w!("2"));
            SendDlgItemMessageW(hwnd, IDOK, BM_CLICK, 0, 0);
        }
        app.drain();
        assert_eq!(app.document().selection().caret, 8);
        assert_eq!(control_selection(app.edit), (5, 5));
        unsafe {
            SendDlgItemMessageW(hwnd, 202, BM_CLICK, 0, 0);
        }
        app.drain();
        unsafe {
            assert!(control_text(GetDlgItem(hwnd, 203)).contains("Maximum: 11"));
            SetDlgItemTextW(hwnd, 204, w!("3"));
            SendDlgItemMessageW(hwnd, IDOK, BM_CLICK, 0, 0);
        }
        app.drain();
        assert_eq!(app.document().selection().caret, 6);
        assert_eq!(control_selection(app.edit), (3, 3));
        unsafe {
            SetDlgItemTextW(hwnd, 204, w!("0"));
            SendDlgItemMessageW(hwnd, IDCANCEL, BM_CLICK, 0, 0);
        }
        app.drain();
        assert_eq!(app.document().selection().caret, 6);
        app.document_mut().set_read_only(false);
        app.command(IDM_EDIT_UNDO);
        assert!(app.document().text().is_empty());
    }

    #[test]
    fn horizontal_wheel_accumulates_and_preserves_buffer_state() {
        let mut app = App::create(false).unwrap();
        app.drain();
        let original = "x".repeat(2000);
        app.document_mut().insert(&original).unwrap();
        app.document_mut()
            .select(Selection {
                anchor: 0,
                caret: 2,
            })
            .unwrap();
        app.refresh();
        let send = |app: &mut App, message, delta: i16, modifiers: u32| {
            let wp = ((delta as u16 as usize) << 16) | modifiers as usize;
            // SAFETY: The owned edit receives scalar wheel parameters.
            let result = unsafe { SendMessageW(app.edit, message, wp, 0) };
            app.drain();
            result
        };
        // SAFETY: The owned edit accepts a character-position query.
        let position =
            |app: &App| unsafe { SendMessageW(app.edit, EM_POSFROMCHAR, 0, 0) as u16 as i16 };
        let mut characters = 0u32;
        // SAFETY: The system setting is copied into a valid u32.
        unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLCHARS,
                0,
                &mut characters as *mut _ as _,
                0,
            );
        }
        let initial = position(&app);
        assert_eq!(send(&mut app, WM_MOUSEHWHEEL, 60, 0), 1);
        assert_eq!(position(&app), initial);
        send(&mut app, WM_MOUSEHWHEEL, 60, 0);
        if characters != 0 {
            assert!(position(&app) < initial);
        } else {
            assert_eq!(position(&app), initial);
        }
        send(&mut app, WM_MOUSEHWHEEL, -120, 0);
        assert_eq!(position(&app), initial);
        assert_eq!(send(&mut app, WM_MOUSEWHEEL, -60, MK_SHIFT | MK_CONTROL), 0);
        assert_eq!(position(&app), initial);
        send(&mut app, WM_MOUSEWHEEL, -60, MK_SHIFT);
        if characters != 0 {
            assert!(position(&app) < initial);
        }
        send(&mut app, WM_MOUSEWHEEL, 120, MK_SHIFT);
        assert_eq!(position(&app), initial);
        assert_eq!(app.zoom, 0);
        assert_eq!(control_selection(app.edit), (0, 2));
        assert_eq!(app.document().text(), original);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "");
    }

    #[test]
    fn spaces_preference_affects_tab_and_menu_without_converting_existing_text() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("a\tb").unwrap();
        app.refresh();
        app.command(INDENT_WITH_SPACES);
        assert_eq!(control_text(app.edit), "a\tb");
        // SAFETY: The test owns the menu and native edit control.
        unsafe {
            assert_ne!(
                GetMenuState(GetMenu(app.window), INDENT_WITH_SPACES, MF_BYCOMMAND) & MF_CHECKED,
                0
            );
            SendMessageW(app.edit, EM_SETSEL, 3, 3);
            SendMessageW(app.edit, WM_CHAR, VK_TAB as usize, 0);
        }
        app.drain();
        assert_eq!(control_text(app.edit), "a\tb   ");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "a\tb");
        app.command(IDM_EDIT_INS_TAB);
        assert_eq!(control_text(app.edit), "    a\tb");
        app.command(IDM_EDIT_UNDO);
        app.command(INDENT_WITH_SPACES);
        app.command(INSERT_TAB);
        assert_eq!(control_text(app.edit), "a\tb\t");
        app.command(IDM_EDIT_UNDO);
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "");
    }

    #[test]
    fn tab_width_updates_rendering_columns_and_conversion_without_editing() {
        let mut app = App::create(false).unwrap();
        app.drain();
        for (command, width) in [(TAB_WIDTH_2, 2), (TAB_WIDTH_4, 4), (TAB_WIDTH_8, 8)] {
            let mut doc = Document::new();
            doc.insert(&format!("\tx\r\n{}x", " ".repeat(width)))
                .unwrap();
            doc.select(Selection {
                anchor: 1,
                caret: 1,
            })
            .unwrap();
            app.workspace.add(doc);
            app.refresh();
            let before = app.document().text().to_owned();
            let dirty = app.document().is_dirty();
            app.command(command);
            assert_eq!(app.document().text(), before);
            assert_eq!(app.document().is_dirty(), dirty);
            assert_eq!(control_selection(app.edit), (1, 1));
            assert!(control_text(app.status).starts_with(&format!("Ln 1, Col {}", width + 1)));
            // SAFETY: Synchronous messages target controls owned by this test.
            unsafe {
                assert_eq!(
                    SendMessageW(
                        app.window,
                        npp_compat::messages::NPPM_GETCURRENTCOLUMN,
                        0,
                        0
                    ),
                    width as isize
                );
                let tab_x = SendMessageW(app.edit, EM_POSFROMCHAR, 1, 0) as u16;
                let spaces_x = SendMessageW(app.edit, EM_POSFROMCHAR, width + 4, 0) as u16;
                assert_eq!(tab_x, spaces_x, "width {width}");
                assert_ne!(
                    GetMenuState(GetMenu(app.window), command, MF_BYCOMMAND) & MF_CHECKED,
                    0
                );
            }
            app.command(IDM_EDIT_TAB2SW);
            assert_eq!(
                app.document().text(),
                format!("{}x\r\n{}x", " ".repeat(width), " ".repeat(width))
            );
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), before);
            app.command(IDM_EDIT_UNDO);
            assert_eq!(app.document().text(), "");
        }
    }

    #[test]
    fn editing_commands_update_native_view_and_share_undo_history() {
        let mut app = App::create(false).unwrap();
        app.drain();
        app.document_mut().insert("  hello  \nworld ").unwrap();
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.command(IDM_EDIT_TRIM_BOTH);
        assert_eq!(app.document().text(), "hello\nworld");
        assert_eq!(control_text(app.edit), "hello\r\nworld");
        // SAFETY: The test owns this edit control and selects UTF-16 positions.
        unsafe {
            SendMessageW(app.edit, EM_SETSEL, 0, 5);
        }
        app.command(IDM_EDIT_UPPERCASE);
        assert_eq!(control_text(app.edit), "HELLO\r\nworld");
        app.command(IDM_EDIT_LOWERCASE);
        assert_eq!(control_text(app.edit), "hello\r\nworld");
        app.command(IDM_EDIT_INVERTCASE);
        assert_eq!(control_text(app.edit), "HELLO\r\nworld");
        assert_eq!(control_selection(app.edit), (0, 5));
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "hello\r\nworld");
        app.command(IDM_EDIT_DUP_LINE);
        // New buffers retain their CRLF preference even when pasted text uses LF.
        assert_eq!(app.document().text(), "hello\r\nhello\nworld");
        app.command(IDM_EDIT_UNDO);
        assert_eq!(app.document().text(), "hello\nworld");
        app.document_mut().select(Selection::default()).unwrap();
        app.document_mut()
            .convert_line_endings(LineEnding::Lf)
            .unwrap();
        app.refresh();
        app.command(IDM_EDIT_SORTLINES_REVERSE_ORDER);
        assert_eq!(control_text(app.edit), "world\r\nhello");
        app.command(IDM_EDIT_SORTLINES_LEXICOGRAPHIC_ASCENDING);
        assert_eq!(control_text(app.edit), "hello\r\nworld");
        app.command(IDM_EDIT_SORTLINES_LEXICOGRAPHIC_DESCENDING);
        assert_eq!(control_text(app.edit), "world\r\nhello");
    }
    #[test]
    fn real_windows_controls_edit_undo_and_switch_documents() {
        let mut app = App::create(false).unwrap();
        app.drain();
        // SAFETY: Test owns these controls and sends synchronous edit messages.
        unsafe {
            SendMessageW(
                app.edit,
                EM_REPLACESEL,
                1,
                wide("hello 🦀\r\nworld").as_ptr() as isize,
            );
        }
        app.drain();
        assert_eq!(app.document().text(), "hello 🦀\r\nworld");
        assert!(app.document().is_dirty());
        app.command(IDM_EDIT_UNDO);
        assert_eq!(control_text(app.edit), "");
        app.command(IDM_EDIT_REDO);
        assert_eq!(control_text(app.edit), "hello 🦀\r\nworld");
        app.search_text = "HELLO".into();
        app.find(SearchDirection::Forward);
        assert_eq!(app.document().selection().range(), 0..5);
        assert_eq!(control_selection(app.edit), (0, 5));
        app.search_request(Request {
            id: 0,
            flags: FR_REPLACE,
            needle: "hello".into(),
            replacement: "Hi".into(),
            options: SearchOptions::default(),
        });
        assert_eq!(app.document().text(), "Hi 🦀\r\nworld");
        app.command(IDM_EDIT_UNDO);
        app.document_mut().select(Selection::default()).unwrap();
        app.refresh();
        app.show_search(false);
        let dialog = app.search_dialog.as_ref().unwrap().hwnd;
        // SAFETY: Exercise the real modeless dialog's button notification and
        // close lifecycle. IDOK is its Find Next button.
        unsafe {
            let button = GetDlgItem(dialog, IDOK);
            assert!(!button.is_null());
            SendMessageW(button, BM_CLICK, 0, 0);
        }
        app.drain();
        assert_eq!(app.document().selection().range(), 0..5);
        unsafe {
            SendMessageW(dialog, WM_CLOSE, 0, 0);
        }
        app.drain();
        assert!(app.search_dialog.as_ref().unwrap().hwnd.is_null());
        let original = app.active_id();
        app.command(IDM_FILE_NEW);
        assert_eq!(control_text(app.edit), "");
        app.workspace.activate(original);
        app.refresh();
        assert_eq!(control_text(app.edit), "hello 🦀\r\nworld");
        app.command(IDM_FORMAT_TOUNIX);
        assert_eq!(app.document().text(), "hello 🦀\nworld");
        assert_eq!(control_text(app.edit), "hello 🦀\r\nworld");
    }
}
