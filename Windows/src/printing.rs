use std::{io, mem::size_of};
use windows_sys::{
    Win32::{Foundation::*, Graphics::Gdi::*, Storage::Xps::*, UI::Controls::Dialogs::*},
    core::w,
};

pub fn print(
    owner: HWND,
    title: &str,
    text: &str,
    selection: Option<&str>,
    immediate: bool,
    tab_width: usize,
) -> io::Result<()> {
    // SAFETY: The print dialog allocates device handles; all are released on
    // every return path. The spool document is aborted if rendering fails.
    unsafe {
        let mut dialog = PRINTDLGW {
            lStructSize: size_of::<PRINTDLGW>() as u32,
            hwndOwner: owner,
            Flags: PD_RETURNDC
                | PD_USEDEVMODECOPIESANDCOLLATE
                | PD_NOPAGENUMS
                | if selection.is_none() {
                    PD_NOSELECTION
                } else {
                    0
                }
                | if immediate { PD_RETURNDEFAULT } else { 0 },
            nCopies: 1,
            ..Default::default()
        };
        if PrintDlgW(&mut dialog) == 0 {
            let code = CommDlgExtendedError();
            if !dialog.hDevMode.is_null() {
                GlobalFree(dialog.hDevMode);
            }
            if !dialog.hDevNames.is_null() {
                GlobalFree(dialog.hDevNames);
            }
            return if code == 0 {
                Ok(())
            } else {
                Err(io::Error::other(format!("Print dialog failed ({code})")))
            };
        }
        let dc = dialog.hDC;
        let text = if dialog.Flags & PD_SELECTION != 0 {
            selection.unwrap_or(text)
        } else {
            text
        };
        let result = (|| -> io::Result<()> {
            let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let info = DOCINFOW {
                cbSize: size_of::<DOCINFOW>() as i32,
                lpszDocName: title.as_ptr(),
                ..Default::default()
            };
            if StartDocW(dc, &info) <= 0 {
                return Err(io::Error::last_os_error());
            }
            let dpi = GetDeviceCaps(dc, LOGPIXELSY as i32);
            let font = CreateFontW(
                -10 * dpi / 72,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                0,
                0,
                0,
                FIXED_PITCH as u32,
                w!("Consolas"),
            );
            if font.is_null() {
                AbortDoc(dc);
                return Err(io::Error::last_os_error());
            }
            let old = SelectObject(dc, font);
            let render = (|| -> io::Result<()> {
                let mut metrics = TEXTMETRICW::default();
                GetTextMetricsW(dc, &mut metrics);
                let line_height = (metrics.tmHeight + metrics.tmExternalLeading).max(1);
                let margin = dpi / 2;
                let width = GetDeviceCaps(dc, HORZRES as i32) - 2 * margin;
                let bottom = GetDeviceCaps(dc, VERTRES as i32) - margin;
                let columns = (width / metrics.tmAveCharWidth.max(1)).max(1) as usize;
                if StartPage(dc) <= 0 {
                    return Err(io::Error::last_os_error());
                }
                let mut y = margin;
                let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                for line in normalized.split('\n') {
                    let mut expanded = Vec::new();
                    for ch in line.chars() {
                        if ch == '\t' {
                            expanded.extend(std::iter::repeat_n(
                                ' ',
                                tab_width - expanded.len() % tab_width,
                            ));
                        } else {
                            expanded.push(ch);
                        }
                    }
                    if expanded.is_empty() {
                        expanded.push(' ');
                    }
                    for row in expanded.chunks(columns) {
                        if y + line_height > bottom {
                            if EndPage(dc) <= 0 || StartPage(dc) <= 0 {
                                return Err(io::Error::last_os_error());
                            }
                            y = margin;
                        }
                        let row: Vec<u16> = row.iter().collect::<String>().encode_utf16().collect();
                        if TextOutW(dc, margin, y, row.as_ptr(), row.len() as i32) == 0 {
                            return Err(io::Error::last_os_error());
                        }
                        y += line_height;
                    }
                }
                if EndPage(dc) <= 0 || EndDoc(dc) <= 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            })();
            if render.is_err() {
                AbortDoc(dc);
            }
            SelectObject(dc, old);
            DeleteObject(font);
            render
        })();
        DeleteDC(dc);
        if !dialog.hDevMode.is_null() {
            GlobalFree(dialog.hDevMode);
        }
        if !dialog.hDevNames.is_null() {
            GlobalFree(dialog.hDevNames);
        }
        result
    }
}
