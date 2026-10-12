use crate::app::{Event, enqueue};
use npp_editor_core::Document;
use std::{io, ptr::null_mut};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::*, Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*},
    },
    core::w,
};

const LINE: i32 = 201;
const OFFSET: i32 = 202;
const INFO: i32 = 203;
const INPUT: i32 = 204;

unsafe extern "system" fn dialog_proc(hwnd: HWND, message: u32, wp: WPARAM, _: LPARAM) -> isize {
    // SAFETY: Windows invokes this procedure for our live dialog. All child IDs
    // belong to controls constructed below; requests copy text into the queue.
    unsafe {
        match message {
            WM_CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
                return 1;
            }
            WM_COMMAND => match (wp & 0xffff) as i32 {
                IDCANCEL => {
                    ShowWindow(hwnd, SW_HIDE);
                    return 1;
                }
                IDOK => {
                    let mut input = [0u16; 256];
                    let count =
                        GetDlgItemTextW(hwnd, INPUT, input.as_mut_ptr(), input.len() as i32);
                    if count != 0 {
                        let value =
                            parse_number(&String::from_utf16_lossy(&input[..count as usize]));
                        if value != -1 {
                            enqueue(Event::GoTo(
                                value,
                                IsDlgButtonChecked(hwnd, OFFSET) == BST_CHECKED,
                            ));
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    }
                    return 1;
                }
                LINE | OFFSET => {
                    enqueue(Event::GoToRefresh);
                    return 1;
                }
                _ => (),
            },
            _ => (),
        }
        0
    }
}

// Match the source dialog's strtoll decimal-prefix parsing and saturation.
fn parse_number(input: &str) -> i64 {
    let text = input.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let negative = text.starts_with('-');
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    let magnitude = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0u64, |n, digit| {
            n.saturating_mul(10).saturating_add(u64::from(digit - b'0'))
        });
    if negative {
        -(magnitude.min(i64::MAX as u64) as i64) - i64::from(magnitude > i64::MAX as u64)
    } else {
        magnitude.min(i64::MAX as u64) as i64
    }
}

pub struct GoToDialog {
    hwnd: HWND,
}

impl GoToDialog {
    #[cfg(test)]
    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    pub fn new(owner: HWND) -> io::Result<Self> {
        #[repr(C, align(4))]
        struct Template {
            header: DLGTEMPLATE,
            tail: [u16; 3],
        }
        let template = Template {
            header: DLGTEMPLATE {
                style: WS_POPUP | WS_CAPTION | WS_SYSMENU | DS_MODALFRAME as u32,
                dwExtendedStyle: 0,
                cdit: 0,
                x: 20,
                y: 20,
                cx: 260,
                cy: 115,
            },
            tail: [0; 3],
        };
        // SAFETY: Aligned empty dialog template remains live during creation.
        // No callback retains pointers into Rust storage.
        unsafe {
            let instance = GetModuleHandleW(null_mut());
            let hwnd =
                CreateDialogIndirectParamW(instance, &template.header, owner, Some(dialog_proc), 0);
            if hwnd.is_null() {
                return Err(io::Error::last_os_error());
            }
            let dialog = Self { hwnd };
            SetWindowTextW(hwnd, w!("Go To"));
            for (class, title, style, x, y, width, height, id) in [
                (
                    w!("BUTTON"),
                    w!("&Line"),
                    WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32,
                    15,
                    12,
                    95,
                    24,
                    LINE,
                ),
                (
                    w!("BUTTON"),
                    w!("&Offset"),
                    BS_AUTORADIOBUTTON as u32,
                    125,
                    12,
                    95,
                    24,
                    OFFSET,
                ),
                (w!("STATIC"), w!(""), WS_GROUP, 15, 44, 340, 24, INFO),
                (
                    w!("STATIC"),
                    w!("&You want to go to:"),
                    0,
                    15,
                    76,
                    140,
                    24,
                    205,
                ),
                (
                    w!("EDIT"),
                    w!(""),
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                    160,
                    73,
                    185,
                    25,
                    INPUT,
                ),
                (
                    w!("BUTTON"),
                    w!("Go To"),
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                    160,
                    117,
                    85,
                    28,
                    IDOK,
                ),
                (
                    w!("BUTTON"),
                    w!("Cancel"),
                    WS_TABSTOP,
                    260,
                    117,
                    85,
                    28,
                    IDCANCEL,
                ),
            ] {
                let child = CreateWindowExW(
                    0,
                    class,
                    title,
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    width,
                    height,
                    hwnd,
                    id as usize as HMENU,
                    instance,
                    null_mut(),
                );
                if child.is_null() {
                    return Err(io::Error::last_os_error());
                }
                SendMessageW(
                    child,
                    WM_SETFONT,
                    GetStockObject(DEFAULT_GUI_FONT) as usize,
                    1,
                );
            }
            CheckRadioButton(hwnd, LINE, OFFSET, LINE);
            SendDlgItemMessageW(hwnd, INPUT, EM_LIMITTEXT, 255, 0);
            Ok(dialog)
        }
    }

    pub fn update(&self, doc: &Document) {
        // SAFETY: The dialog and children remain owned until Drop.
        unsafe {
            let offset = IsDlgButtonChecked(self.hwnd, OFFSET) == BST_CHECKED;
            let (current, limit) = if offset {
                (doc.selection().caret, doc.text().len().saturating_sub(1))
            } else {
                (
                    doc.line_starts()
                        .partition_point(|&start| start <= doc.selection().caret),
                    doc.line_starts().len(),
                )
            };
            let info: Vec<u16> = format!("You are here: {current}     Maximum: {limit}")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            SetDlgItemTextW(self.hwnd, INFO, info.as_ptr());
        }
    }

    pub fn show(&self, doc: &Document) {
        self.update(doc);
        // SAFETY: Live owned dialog; standard focus and visibility operations.
        unsafe {
            SetDlgItemTextW(self.hwnd, INPUT, w!(""));
            ShowWindow(self.hwnd, SW_SHOW);
            SetForegroundWindow(self.hwnd);
            SetFocus(GetDlgItem(self.hwnd, INPUT));
        }
    }

    pub fn translate(&self, message: &MSG) -> bool {
        // SAFETY: Windows only reads MSG during synchronous dialog dispatch.
        unsafe { IsWindowVisible(self.hwnd) != 0 && IsDialogMessageW(self.hwnd, message) != 0 }
    }
}

impl Drop for GoToDialog {
    fn drop(&mut self) {
        // SAFETY: This wrapper uniquely owns the dialog on its UI thread.
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                DestroyWindow(self.hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_decimal_prefix_rules() {
        for (input, expected) in [
            ("  +42tail", 42),
            ("oops", 0),
            ("-1", -1),
            ("-9", -9),
            ("9999999999999999999999", i64::MAX),
            ("-9999999999999999999999", i64::MIN),
        ] {
            assert_eq!(parse_number(input), expected);
        }
    }
}
