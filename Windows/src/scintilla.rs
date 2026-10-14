//! Scintilla owns rendering/lexing/folding; the Rust document owns text/history.
//! The small EDIT-message adapter keeps existing UTF-16 UI commands working.
use std::{
    ffi::{CString, c_void},
    ptr::null,
};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*,
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

#[allow(dead_code)]
pub mod ids {
    include!(concat!(env!("OUT_DIR"), "/scintilla_ids.rs"));
}
use ids::*;
unsafe extern "C" {
    fn Scintilla_RegisterClasses(instance: *mut c_void) -> i32;
}
unsafe extern "system" {
    fn CreateLexer(name: *const i8) -> *mut c_void;
}

pub fn send(hwnd: HWND, message: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, wp, lp) }
}
pub fn register() -> std::io::Result<()> {
    static REGISTERED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *REGISTERED
        .get_or_init(|| unsafe { Scintilla_RegisterClasses(GetModuleHandleW(null())) != 0 })
    {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "Could not register the bundled Scintilla editor",
        ))
    }
}
pub fn text(hwnd: HWND) -> String {
    let length = send(hwnd, SCI_GETLENGTH, 0, 0).max(0) as usize;
    let mut bytes = vec![0; length + 1];
    send(hwnd, SCI_GETTEXT, bytes.len(), bytes.as_mut_ptr() as isize);
    bytes.truncate(length);
    String::from_utf8_lossy(&bytes).into_owned()
}
fn byte(hwnd: HWND, position: usize) -> usize {
    if position == usize::MAX {
        return send(hwnd, SCI_GETLENGTH, 0, 0) as usize;
    }
    let text = text(hwnd);
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units >= position {
            return index;
        }
        units += ch.len_utf16();
    }
    text.len()
}
fn units(hwnd: HWND, position: usize) -> usize {
    let text = text(hwnd);
    let mut end = position.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].encode_utf16().count()
}
pub fn set_text(hwnd: HWND, value: &str) {
    let old = text(hwnd);
    if old == value {
        return;
    }
    let prefix: usize = old
        .chars()
        .zip(value.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let suffix: usize = old[prefix..]
        .chars()
        .rev()
        .zip(value[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let readonly = send(hwnd, SCI_GETREADONLY, 0, 0);
    send(hwnd, SCI_SETREADONLY, 0, 0);
    send(hwnd, SCI_SETTARGETSTART, prefix, 0);
    send(hwnd, SCI_SETTARGETEND, old.len() - suffix, 0);
    let replacement = &value.as_bytes()[prefix..value.len() - suffix];
    send(
        hwnd,
        SCI_REPLACETARGET,
        replacement.len(),
        replacement.as_ptr() as isize,
    );
    send(hwnd, SCI_SETREADONLY, readonly as usize, 0);
}
pub fn install(hwnd: HWND) -> std::io::Result<()> {
    unsafe {
        if SetWindowSubclass(hwnd, Some(adapter), 72, 0) == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    send(hwnd, SCI_SETCODEPAGE, SC_CP_UTF8 as usize, 0);
    send(hwnd, SCI_SETUNDOCOLLECTION, 0, 0);
    send(hwnd, SCI_SETEOLMODE, SC_EOL_CRLF as usize, 0);
    send(hwnd, SCI_SETMARGINTYPEN, 0, SC_MARGIN_NUMBER as isize);
    send(hwnd, SCI_SETMARGINWIDTHN, 0, 48);
    send(hwnd, SCI_SETMARGINWIDTHN, 1, 0);
    send(hwnd, SCI_SETMARGINTYPEN, 2, SC_MARGIN_SYMBOL as isize);
    send(hwnd, SCI_SETMARGINMASKN, 2, SC_MASK_FOLDERS as isize);
    send(hwnd, SCI_SETMARGINSENSITIVEN, 2, 1);
    send(hwnd, SCI_SETMARGINWIDTHN, 2, 18);
    for (marker, symbol) in [
        (SC_MARKNUM_FOLDEROPEN, SC_MARK_BOXMINUS),
        (SC_MARKNUM_FOLDER, SC_MARK_BOXPLUS),
        (SC_MARKNUM_FOLDERSUB, SC_MARK_VLINE),
        (SC_MARKNUM_FOLDERTAIL, SC_MARK_LCORNER),
        (SC_MARKNUM_FOLDEREND, SC_MARK_BOXPLUSCONNECTED),
        (SC_MARKNUM_FOLDEROPENMID, SC_MARK_BOXMINUSCONNECTED),
        (SC_MARKNUM_FOLDERMIDTAIL, SC_MARK_TCORNER),
    ] {
        send(hwnd, SCI_MARKERDEFINE, marker as usize, symbol as isize);
        send(hwnd, SCI_MARKERSETFORE, marker as usize, 0xffffff);
        send(hwnd, SCI_MARKERSETBACK, marker as usize, 0x808080);
    }
    send(
        hwnd,
        SCI_SETAUTOMATICFOLD,
        (SC_AUTOMATICFOLD_SHOW | SC_AUTOMATICFOLD_CLICK | SC_AUTOMATICFOLD_CHANGE) as usize,
        0,
    );
    send(
        hwnd,
        SCI_SETFOLDFLAGS,
        SC_FOLDFLAG_LINEAFTER_CONTRACTED as usize,
        0,
    );
    send(hwnd, SCI_SETINDENTATIONGUIDES, SC_IV_LOOKBOTH as usize, 0);
    send(hwnd, SCI_SETSELFORE, 1, 0xffffff);
    send(hwnd, SCI_SETSELBACK, 1, 0xb57933);
    send(hwnd, SCI_SETCARETLINEVISIBLE, 1, 0);
    send(hwnd, SCI_SETCARETLINEBACK, 0xfaf5f0, 0);
    send(hwnd, SCI_SETSCROLLWIDTH, 1, 0);
    send(hwnd, SCI_SETSCROLLWIDTHTRACKING, 1, 0);
    Ok(())
}

pub fn property(hwnd: HWND, name: &str, value: &str) {
    if let (Ok(name), Ok(value)) = (CString::new(name), CString::new(value)) {
        send(
            hwnd,
            SCI_SETPROPERTY,
            name.as_ptr() as usize,
            value.as_ptr() as isize,
        );
    }
}
pub fn language(hwnd: HWND, language: &crate::languages::Language) {
    let data = crate::syntax_data::lookup(language.command);
    let lexer_name = data.map_or("null", |d| d.lexer);
    let lexer = CString::new(lexer_name).unwrap();
    let instance = unsafe { CreateLexer(lexer.as_ptr()) };
    send(hwnd, SCI_SETILEXER, 0, instance as isize);
    send(hwnd, SCI_STYLESETFORE, STYLE_DEFAULT as usize, 0x202020);
    send(hwnd, SCI_STYLESETBACK, STYLE_DEFAULT as usize, 0xffffff);
    send(hwnd, SCI_STYLESETBOLD, STYLE_DEFAULT as usize, 0);
    send(hwnd, SCI_STYLESETITALIC, STYLE_DEFAULT as usize, 0);
    send(hwnd, SCI_STYLECLEARALL, 0, 0);
    send(hwnd, SCI_STYLESETFORE, STYLE_LINENUMBER as usize, 0x808080);
    send(hwnd, SCI_STYLESETBACK, STYLE_LINENUMBER as usize, 0xf4f4f4);
    for i in 0..9 {
        send(hwnd, SCI_SETKEYWORDS, i, c"".as_ptr() as isize);
    }
    if let Some(data) = data {
        for &(index, words) in data.keywords {
            let words = CString::new(words).unwrap();
            send(hwnd, SCI_SETKEYWORDS, index, words.as_ptr() as isize);
        }
        for style in data.styles {
            send(hwnd, SCI_STYLESETFORE, style.id, style.fore as isize);
            send(hwnd, SCI_STYLESETBACK, style.id, style.back as isize);
            send(
                hwnd,
                SCI_STYLESETBOLD,
                style.id,
                (style.flags & 1 != 0) as isize,
            );
            send(
                hwnd,
                SCI_STYLESETITALIC,
                style.id,
                (style.flags & 2 != 0) as isize,
            );
        }
    }
    for (key, value) in [
        ("fold", "1"),
        ("fold.compact", "0"),
        ("fold.comment", "1"),
        ("fold.preprocessor", "1"),
        ("fold.html", "1"),
        ("fold.hypertext.comment", "1"),
        ("fold.xml.at.tag.open", "1"),
        ("fold.quotes.python", "1"),
        ("fold.at.else", "1"),
        ("lexer.cpp.track.preprocessor", "0"),
        ("lexer.json.allow.comments", "1"),
    ] {
        property(hwnd, key, value);
    }
    property(
        hwnd,
        "lexer.cpp.backquoted.strings",
        if language.command == npp_compat::commands::IDM_LANG_GOLANG {
            "1"
        } else {
            "0"
        },
    );
    send(hwnd, SCI_COLOURISE, 0, -1);
}
pub fn collapsed(hwnd: HWND) -> Vec<usize> {
    (0..send(hwnd, SCI_GETLINECOUNT, 0, 0) as usize)
        .filter(|&line| {
            send(hwnd, SCI_GETFOLDLEVEL, line, 0) as u32 & SC_FOLDLEVELHEADERFLAG != 0
                && send(hwnd, SCI_GETFOLDEXPANDED, line, 0) == 0
        })
        .collect()
}
pub fn restore_folds(hwnd: HWND, lines: &[usize]) {
    send(hwnd, SCI_COLOURISE, 0, -1);
    send(hwnd, SCI_FOLDALL, SC_FOLDACTION_EXPAND as usize, 0);
    for &line in lines {
        send(hwnd, SCI_FOLDLINE, line, SC_FOLDACTION_CONTRACT as isize);
    }
}
pub fn fold(hwnd: HWND, expand: bool, level: Option<usize>, current: bool) {
    send(hwnd, SCI_COLOURISE, 0, -1);
    let action = if expand {
        SC_FOLDACTION_EXPAND
    } else {
        SC_FOLDACTION_CONTRACT
    };
    if current {
        let line = send(
            hwnd,
            SCI_LINEFROMPOSITION,
            send(hwnd, SCI_GETCURRENTPOS, 0, 0) as usize,
            0,
        ) as usize;
        let header = if send(hwnd, SCI_GETFOLDLEVEL, line, 0) as u32 & SC_FOLDLEVELHEADERFLAG != 0 {
            line as isize
        } else {
            send(hwnd, SCI_GETFOLDPARENT, line, 0)
        };
        if header >= 0 {
            send(hwnd, SCI_FOLDLINE, header as usize, action as isize);
        }
    } else if let Some(level) = level {
        for line in 0..send(hwnd, SCI_GETLINECOUNT, 0, 0) as usize {
            let fold = send(hwnd, SCI_GETFOLDLEVEL, line, 0) as u32;
            if fold & SC_FOLDLEVELHEADERFLAG == 0 {
                continue;
            }
            // Indentation lexers encode columns, not nesting depth, in levels.
            let mut depth = 1;
            let mut parent = send(hwnd, SCI_GETFOLDPARENT, line, 0);
            while parent >= 0 && depth <= level {
                depth += 1;
                parent = send(hwnd, SCI_GETFOLDPARENT, parent as usize, 0);
            }
            if depth == level {
                send(hwnd, SCI_FOLDLINE, line, action as isize);
            }
        }
    } else {
        send(hwnd, SCI_FOLDALL, action as usize, 0);
    }
}

unsafe extern "system" fn adapter(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_GETTEXTLENGTH => return text(hwnd).encode_utf16().count() as isize,
            WM_GETTEXT => {
                if wp == 0 || lp == 0 {
                    return 0;
                }
                let units: Vec<_> = text(hwnd).encode_utf16().take(wp - 1).collect();
                std::ptr::copy_nonoverlapping(units.as_ptr(), lp as *mut u16, units.len());
                *(lp as *mut u16).add(units.len()) = 0;
                return units.len() as isize;
            }
            WM_SETTEXT => {
                if lp != 0 {
                    let mut len = 0;
                    while *(lp as *const u16).add(len) != 0 {
                        len += 1;
                    }
                    set_text(
                        hwnd,
                        &String::from_utf16_lossy(std::slice::from_raw_parts(
                            lp as *const u16,
                            len,
                        )),
                    );
                }
                return 1;
            }
            EM_GETSEL => {
                let a = units(hwnd, send(hwnd, SCI_GETANCHOR, 0, 0) as usize);
                let c = units(hwnd, send(hwnd, SCI_GETCURRENTPOS, 0, 0) as usize);
                let (start, end) = (a.min(c), a.max(c));
                if wp != 0 {
                    *(wp as *mut u32) = start as u32;
                }
                if lp != 0 {
                    *(lp as *mut u32) = end as u32;
                }
                return (start.min(65535) | (end.min(65535) << 16)) as isize;
            }
            EM_SETSEL => {
                return send(
                    hwnd,
                    SCI_SETSEL,
                    byte(hwnd, wp),
                    byte(hwnd, lp as usize) as isize,
                );
            }
            EM_SETREADONLY => return send(hwnd, SCI_SETREADONLY, wp, 0),
            EM_LIMITTEXT | EM_SETMARGINS => return 0,
            EM_GETMARGINS => return 0,
            EM_SETTABSTOPS => {
                let spacing = if wp > 0 && lp != 0 {
                    (*(lp as *const u32) / 4).max(1)
                } else {
                    4
                };
                send(hwnd, SCI_SETTABWIDTH, spacing as usize, 0);
                send(hwnd, SCI_SETINDENT, spacing as usize, 0);
                return 1;
            }
            EM_SCROLLCARET => {
                let line = send(
                    hwnd,
                    SCI_LINEFROMPOSITION,
                    send(hwnd, SCI_GETCURRENTPOS, 0, 0) as usize,
                    0,
                );
                send(hwnd, SCI_ENSUREVISIBLE, line as usize, 0);
                return send(hwnd, SCI_SCROLLCARET, 0, 0);
            }
            EM_GETLINECOUNT => return send(hwnd, SCI_GETLINECOUNT, 0, 0),
            EM_GETFIRSTVISIBLELINE => {
                return send(
                    hwnd,
                    SCI_DOCLINEFROMVISIBLE,
                    send(hwnd, SCI_GETFIRSTVISIBLELINE, 0, 0) as usize,
                    0,
                );
            }
            EM_LINEINDEX => {
                return units(
                    hwnd,
                    send(hwnd, SCI_POSITIONFROMLINE, wp, 0).max(0) as usize,
                ) as isize;
            }
            EM_LINESCROLL => {
                if wp != 0 {
                    send(
                        hwnd,
                        SCI_SETXOFFSET,
                        (send(hwnd, SCI_GETXOFFSET, 0, 0) + (wp as isize) * 8).max(0) as usize,
                        0,
                    );
                }
                return send(hwnd, SCI_LINESCROLL, 0, lp);
            }
            EM_POSFROMCHAR => {
                let position = byte(hwnd, wp);
                let x = send(hwnd, SCI_POINTXFROMPOSITION, 0, position as isize);
                let y = send(hwnd, SCI_POINTYFROMPOSITION, 0, position as isize);
                return ((x as u16 as u32) | ((y as u16 as u32) << 16)) as isize;
            }
            WM_SETFONT => {
                SetPropW(hwnd, w!("EditorFont"), wp as HANDLE);
                let mut font = LOGFONTW::default();
                if GetObjectW(
                    wp as HGDIOBJ,
                    size_of::<LOGFONTW>() as i32,
                    (&mut font as *mut LOGFONTW).cast(),
                ) != 0
                {
                    let len = font.lfFaceName.iter().position(|&c| c == 0).unwrap_or(32);
                    let name =
                        CString::new(String::from_utf16_lossy(&font.lfFaceName[..len])).unwrap();
                    let dc = GetDC(hwnd);
                    let dpi = GetDeviceCaps(dc, LOGPIXELSY as i32).max(96);
                    ReleaseDC(hwnd, dc);
                    for style in 0..256 {
                        send(hwnd, SCI_STYLESETFONT, style, name.as_ptr() as isize);
                        send(
                            hwnd,
                            SCI_STYLESETSIZEFRACTIONAL,
                            style,
                            (font.lfHeight.abs() * 7200 / dpi) as isize,
                        );
                    }
                }
                return 0;
            }
            WM_GETFONT => return GetPropW(hwnd, w!("EditorFont")) as isize,
            WM_NCDESTROY => {
                RemovePropW(hwnd, w!("EditorFont"));
            }
            _ => (),
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}

#[repr(C)]
pub struct Notification {
    pub header: NMHDR,
    pub position: isize,
    pub ch: i32,
    pub modifiers: i32,
    pub modification_type: i32,
    pub text: *const i8,
    pub length: isize,
    pub lines_added: isize,
    pub message: i32,
    pub wp: usize,
    pub lp: isize,
    pub line: isize,
    pub fold_level_now: i32,
    pub fold_level_prev: i32,
    pub margin: i32,
}
