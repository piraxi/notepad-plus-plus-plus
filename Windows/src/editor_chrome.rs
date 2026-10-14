use npp_compat::commands::*;
use std::{mem::size_of, ptr::null};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{
            Controls::*,
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

pub const TOOLBAR: i32 = 107;
pub const GUTTER: i32 = 108;

unsafe extern "system" fn gutter_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        if message == WM_PAINT {
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            FillRect(dc, &rect, GetSysColorBrush(COLOR_BTNFACE));
            let edit = GetDlgItem(GetParent(hwnd), 101);
            let font = SendMessageW(edit, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            SetTextColor(dc, GetSysColor(COLOR_GRAYTEXT));
            SetBkMode(dc, TRANSPARENT as i32);
            let first = SendMessageW(edit, EM_GETFIRSTVISIBLELINE, 0, 0).max(0) as usize;
            let count = SendMessageW(edit, EM_GETLINECOUNT, 0, 0).max(0) as usize;
            let mut text = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
            let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32) as usize;
            let starts: Vec<_> = std::iter::once(0)
                .chain(
                    text[..length]
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &c)| (c == 10).then_some(i + 1)),
                )
                .collect();
            let mut metrics = TEXTMETRICW::default();
            GetTextMetricsW(dc, &mut metrics);
            let height = (metrics.tmHeight + metrics.tmExternalLeading).max(1);
            for line in first..count.min(first + (rect.bottom / height + 2) as usize) {
                let index = SendMessageW(edit, EM_LINEINDEX, line, 0);
                let Ok(logical) = starts.binary_search(&(index as usize)) else {
                    continue;
                };
                let position = SendMessageW(edit, EM_POSFROMCHAR, index as usize, 0);
                let y = if position == -1 {
                    (line - first) as i32 * height + 1
                } else {
                    (position >> 16) as i16 as i32
                };
                let mut row = RECT {
                    left: 0,
                    top: y,
                    right: rect.right - 8,
                    bottom: y + height,
                };
                let text: Vec<u16> = (logical + 1).to_string().encode_utf16().collect();
                DrawTextW(
                    dc,
                    text.as_ptr(),
                    text.len() as i32,
                    &mut row,
                    DT_RIGHT | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
            SelectObject(dc, old);
            EndPaint(hwnd, &paint);
            return 0;
        }
        DefSubclassProc(hwnd, message, wp, lp)
    }
}

pub fn create(owner: HWND) -> std::io::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(null());
        let toolbar = CreateWindowExW(
            0,
            TOOLBARCLASSNAMEW,
            w!(""),
            WS_CHILD | WS_VISIBLE | TBSTYLE_FLAT | TBSTYLE_TOOLTIPS | CCS_NORESIZE as u32,
            0,
            0,
            100,
            28,
            owner,
            TOOLBAR as usize as HMENU,
            instance,
            null(),
        );
        let gutter = CreateWindowExW(
            0,
            w!("STATIC"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            0,
            56,
            52,
            100,
            owner,
            GUTTER as usize as HMENU,
            instance,
            null(),
        );
        if toolbar.is_null() || gutter.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        SendMessageW(toolbar, TB_BUTTONSTRUCTSIZE, size_of::<TBBUTTON>(), 0);
        SendMessageW(toolbar, TB_SETMAXTEXTROWS, 0, 0);
        let bitmap = TBADDBITMAP {
            hInst: -1isize as HINSTANCE,
            nID: IDB_STD_SMALL_COLOR as usize,
        };
        SendMessageW(toolbar, TB_ADDBITMAP, 0, &bitmap as *const _ as isize);
        let mut buttons = Vec::new();
        for (image, command, label) in [
            (STD_FILENEW, IDM_FILE_NEW, "New"),
            (STD_FILEOPEN, IDM_FILE_OPEN, "Open"),
            (STD_FILESAVE, IDM_FILE_SAVE, "Save"),
            (0, 0, ""),
            (STD_CUT, IDM_EDIT_CUT, "Cut"),
            (STD_COPY, IDM_EDIT_COPY, "Copy"),
            (STD_PASTE, IDM_EDIT_PASTE, "Paste"),
            (0, 0, ""),
            (STD_UNDO, IDM_EDIT_UNDO, "Undo"),
            (STD_REDOW, IDM_EDIT_REDO, "Redo"),
            (0, 0, ""),
            (STD_FIND, IDM_SEARCH_FIND, "Find"),
            (STD_REPLACE, IDM_SEARCH_REPLACE, "Replace"),
        ] {
            let text: Vec<u16> = label.encode_utf16().chain([0, 0]).collect();
            let string = SendMessageW(toolbar, TB_ADDSTRINGW, 0, text.as_ptr() as isize);
            buttons.push(TBBUTTON {
                iBitmap: image as i32,
                idCommand: command as i32,
                fsState: TBSTATE_ENABLED as u8,
                fsStyle: if command == 0 { BTNS_SEP } else { BTNS_BUTTON } as u8,
                iString: string,
                ..Default::default()
            });
        }
        SendMessageW(
            toolbar,
            TB_ADDBUTTONS,
            buttons.len(),
            buttons.as_ptr() as isize,
        );
        SetWindowSubclass(gutter, Some(gutter_proc), 1, 0);
    }
    Ok(())
}
