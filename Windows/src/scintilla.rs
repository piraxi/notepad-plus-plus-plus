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
#[path = "control_chars.rs"]
mod control_chars;
#[path = "hidden_line_icons.rs"]
mod hidden_line_icons;

/// Refresh symbol artwork and width without changing the user's visibility choice.
pub fn update_symbol_margin(hwnd: HWND, visible: bool) {
    let large = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd) } >= 144;
    let size = if large { 18 } else { 14 };
    send(hwnd, SCI_RGBAIMAGESETWIDTH, size, 0);
    send(hwnd, SCI_RGBAIMAGESETHEIGHT, size, 0);
    let (begin, end): (&[u8], &[u8]) = if large {
        (
            &hidden_line_icons::HIDELINES_BEGIN18,
            &hidden_line_icons::HIDELINES_END18,
        )
    } else {
        (
            &hidden_line_icons::HIDELINES_BEGIN14,
            &hidden_line_icons::HIDELINES_END14,
        )
    };
    send(hwnd, SCI_MARKERDEFINERGBAIMAGE, 19, begin.as_ptr() as isize);
    send(hwnd, SCI_MARKERDEFINERGBAIMAGE, 18, end.as_ptr() as isize);
    send(
        hwnd,
        SCI_SETELEMENTCOLOUR,
        SC_ELEMENT_HIDDEN_LINE as usize,
        0xff77cc77u32 as isize,
    );
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd) }.max(96);
    let width = ((16 * dpi + 48) / 96).max(size as u32 + 2);
    send(
        hwnd,
        SCI_SETMARGINWIDTHN,
        1,
        if visible { width as isize } else { 0 },
    );
}

/// Upstream avoids DirectWrite on Wine and Windows Server Core.
pub fn direct_write_unavailable() -> bool {
    use windows_sys::Win32::System::{
        LibraryLoader::GetProcAddress, Registry::*, SystemInformation::*,
    };
    // SAFETY: Read-only module/registry queries with fixed, sized output storage.
    unsafe {
        let ntdll = GetModuleHandleW(w!("ntdll.dll"));
        if !ntdll.is_null() && GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_some()
        {
            return true;
        }
        let mut version = OSVERSIONINFOEXW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOEXW>() as u32,
            ..Default::default()
        };
        let mut product = 0;
        if GetVersionExW((&mut version as *mut OSVERSIONINFOEXW).cast()) != 0
            && GetProductInfo(
                version.dwMajorVersion,
                version.dwMinorVersion,
                version.wServicePackMajor as u32,
                version.wServicePackMinor as u32,
                &mut product,
            ) != 0
            && matches!(
                product,
                PRODUCT_STANDARD_SERVER_CORE_
                    | PRODUCT_STANDARD_A_SERVER_CORE
                    | PRODUCT_STANDARD_SERVER_CORE_V
                    | PRODUCT_STANDARD_SERVER_SOLUTIONS_CORE
                    | PRODUCT_SMALLBUSINESS_SERVER_PREMIUM_CORE
                    | PRODUCT_ENTERPRISE_SERVER_CORE
                    | PRODUCT_ENTERPRISE_SERVER_CORE_V
                    | PRODUCT_DATACENTER_SERVER_CORE
                    | PRODUCT_DATACENTER_A_SERVER_CORE
                    | PRODUCT_DATACENTER_SERVER_CORE_V
                    | PRODUCT_STORAGE_STANDARD_SERVER_CORE
                    | PRODUCT_STORAGE_WORKGROUP_SERVER_CORE
                    | PRODUCT_STORAGE_ENTERPRISE_SERVER_CORE
                    | PRODUCT_STORAGE_EXPRESS_SERVER_CORE
                    | PRODUCT_WEB_SERVER_CORE
            )
        {
            return true;
        }
        let mut installation = [0u16; 128];
        let mut size = std::mem::size_of_val(&installation) as u32;
        let result = RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            w!("InstallationType"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            installation.as_mut_ptr().cast(),
            &mut size,
        );
        result == ERROR_SUCCESS
            && String::from_utf16_lossy(
                &installation[..installation
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(installation.len())],
            )
            .eq_ignore_ascii_case("Server Core")
    }
}

pub fn configure_rendering(hwnd: HWND, requested: usize) -> bool {
    let mode = if direct_write_unavailable() {
        0
    } else {
        requested.min(4)
    };
    send(hwnd, SCI_SETTECHNOLOGY, mode, 0);
    if send(hwnd, SCI_GETTECHNOLOGY, 0, 0) as usize != mode {
        send(hwnd, SCI_SETTECHNOLOGY, 0, 0);
        return false;
    }
    true
}

/// Match Notepad++'s GDI mirrored view and horizontal navigation bindings.
pub fn text_direction(hwnd: HWND, rtl: bool) {
    // SAFETY: The editor handle belongs to this UI thread. Only its layout flag changes.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let mirrored = style & WS_EX_LAYOUTRTL as isize != 0;
        if mirrored == rtl {
            return;
        }
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            if rtl {
                style | WS_EX_LAYOUTRTL as isize
            } else {
                style & !(WS_EX_LAYOUTRTL as isize)
            },
        );
    }
    // SCK_LEFT/RIGHT and SCMOD values from the bundled Scintilla.h.
    for (modifier, left, right) in [
        (0, SCI_CHARLEFT, SCI_CHARRIGHT),
        (1, SCI_CHARLEFTEXTEND, SCI_CHARRIGHTEXTEND),
        (5, SCI_CHARLEFTRECTEXTEND, SCI_CHARRIGHTRECTEXTEND),
        (2, SCI_WORDLEFT, SCI_WORDRIGHT),
        (3, SCI_WORDLEFTEXTEND, SCI_WORDRIGHTEXTEND),
    ] {
        send(
            hwnd,
            SCI_ASSIGNCMDKEY,
            302 | (modifier << 16),
            if rtl { right } else { left } as isize,
        );
        send(
            hwnd,
            SCI_ASSIGNCMDKEY,
            303 | (modifier << 16),
            if rtl { left } else { right } as isize,
        );
    }
    // Upstream forces a wrap recalculation to refresh mirrored characters.
    let wrap = send(hwnd, SCI_GETWRAPMODE, 0, 0);
    send(hwnd, SCI_SETWRAPMODE, if wrap == 0 { 1 } else { 0 }, 0);
    send(hwnd, SCI_SETWRAPMODE, wrap as usize, 0);
    // SAFETY: Invalidate the live editor after its layout changes.
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 1);
    }
}

pub fn non_printing(hwnd: HWND, show: bool) {
    for &(encoded, label) in control_chars::NON_PRINTING {
        if show {
            send(
                hwnd,
                SCI_SETREPRESENTATION,
                encoded.as_ptr() as usize,
                label.as_ptr() as isize,
            );
            send(
                hwnd,
                SCI_SETREPRESENTATIONAPPEARANCE,
                encoded.as_ptr() as usize,
                SC_REPRESENTATION_BLOB as isize,
            );
        } else {
            send(hwnd, SCI_CLEARREPRESENTATION, encoded.as_ptr() as usize, 0);
        }
    }
}

pub fn selection_foreground(hwnd: HWND, enabled: bool) {
    for element in [
        SC_ELEMENT_SELECTION_TEXT,
        SC_ELEMENT_SELECTION_INACTIVE_TEXT,
        SC_ELEMENT_SELECTION_ADDITIONAL_TEXT,
        SC_ELEMENT_SELECTION_SECONDARY_TEXT,
    ] {
        send(
            hwnd,
            if enabled {
                SCI_SETELEMENTCOLOUR
            } else {
                SCI_RESETELEMENTCOLOUR
            },
            element as usize,
            0xffffffffu32 as isize,
        );
    }
}
pub fn control_characters(hwnd: HWND, show: bool, language: u32) {
    for &(encoded, label) in control_chars::CONTROLS {
        let display: &[u8] = if show { label } else { b"\xe2\x80\x8b\0" };
        send(
            hwnd,
            SCI_SETREPRESENTATION,
            encoded.as_ptr() as usize,
            display.as_ptr() as isize,
        );
        send(
            hwnd,
            SCI_SETREPRESENTATIONAPPEARANCE,
            encoded.as_ptr() as usize,
            if show {
                SC_REPRESENTATION_BLOB
            } else {
                SC_REPRESENTATION_PLAIN
            } as isize,
        );
    }
    // Match upstream visibility of ANSI escape sequences for these lexers.
    let styles: &[usize] = match language {
        npp_compat::commands::IDM_LANG_ERRORLIST => &[23, 24],
        npp_compat::commands::IDM_LANG_ESCSEQ => &[171, 172],
        _ => &[],
    };
    for &style in styles {
        send(hwnd, SCI_STYLESETVISIBLE, style, show as isize);
    }
}
unsafe extern "C" {
    fn Scintilla_RegisterClasses(instance: *mut c_void) -> i32;
}
unsafe extern "system" {
    fn CreateLexer(name: *const i8) -> *mut c_void;
}

pub fn send(hwnd: HWND, message: u32, wp: usize, lp: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, wp, lp) }
}
/// Match the brace before the caret first, as Notepad++ does. Scintilla
/// restricts matching to the lexer's style, ignoring braces in other tokens.
pub fn line_number_width(hwnd: HWND) -> isize {
    let dynamic = unsafe { GetPropW(hwnd, w!("ConstantLineNumberWidth")).is_null() };
    let lines = if dynamic {
        let last =
            send(hwnd, SCI_GETFIRSTVISIBLELINE, 0, 0) + send(hwnd, SCI_LINESONSCREEN, 0, 0) + 1;
        send(hwnd, SCI_DOCLINEFROMVISIBLE, last.max(0) as usize, 0)
    } else {
        send(hwnd, SCI_GETLINECOUNT, 0, 0)
    };
    let digits = lines
        .max(1)
        .to_string()
        .len()
        .max(if dynamic { 3 } else { 4 });
    8 + digits as isize
        * send(
            hwnd,
            SCI_TEXTWIDTH,
            STYLE_LINENUMBER as usize,
            c"8".as_ptr() as isize,
        )
}

pub fn matching_braces(hwnd: HWND) -> (isize, isize) {
    let caret = send(hwnd, SCI_GETCURRENTPOS, 0, 0);
    let length = send(hwnd, SCI_GETLENGTH, 0, 0);
    for position in [caret - 1, caret] {
        if position >= 0 && position < length {
            let ch = send(hwnd, SCI_GETCHARAT, position as usize, 0);
            if b"[](){}".contains(&(ch as u8)) {
                return (position, send(hwnd, SCI_BRACEMATCH, position as usize, 0));
            }
        }
    }
    (-1, -1)
}

pub fn highlight_braces(hwnd: HWND) -> bool {
    let (brace, opposite) = matching_braces(hwnd);
    if brace >= 0 && opposite < 0 {
        send(hwnd, SCI_BRACEBADLIGHT, brace as usize, 0);
    } else {
        send(hwnd, SCI_BRACEHIGHLIGHT, brace as usize, opposite);
    }
    let column = if opposite >= 0 && send(hwnd, SCI_GETINDENTATIONGUIDES, 0, 0) != 0 {
        send(hwnd, SCI_GETCOLUMN, brace as usize, 0).min(send(
            hwnd,
            SCI_GETCOLUMN,
            opposite as usize,
            0,
        ))
    } else {
        0
    };
    send(hwnd, SCI_SETHIGHLIGHTGUIDE, column as usize, 0);
    opposite >= 0
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
/// Status-bar counts follow Notepad++'s multi-range and rectangle display.
pub fn multiple_selection_status(hwnd: HWND) -> Option<String> {
    let count = send(hwnd, SCI_GETSELECTIONS, 0, 0).max(0) as usize;
    if count <= 1 {
        return None;
    }
    let rectangle = send(hwnd, SCI_SELECTIONISRECTANGLE, 0, 0) != 0;
    let mut total = 0;
    let mut maximum = 0;
    let mut first_width = None;
    let mut same_width = true;
    let mut lines = Vec::new();
    for index in 0..count {
        let start = send(hwnd, SCI_GETSELECTIONNSTART, index, 0).max(0) as usize;
        let end = send(hwnd, SCI_GETSELECTIONNEND, index, 0).max(0) as usize;
        let characters = send(hwnd, SCI_COUNTCHARACTERS, start, end as isize).max(0) as usize;
        total += characters;
        maximum = maximum.max(characters);
        if let Some(first) = first_width {
            same_width &= first == characters;
        } else {
            first_width = Some(characters);
        }
        if !rectangle && count <= 99 {
            let first = send(hwnd, SCI_LINEFROMPOSITION, start, 0).max(0) as usize;
            let mut last = send(hwnd, SCI_LINEFROMPOSITION, end, 0).max(0) as usize;
            if last > first && send(hwnd, SCI_POSITIONFROMLINE, last, 0) as usize == end {
                last -= 1;
            }
            lines.push((first, last));
        }
    }
    if rectangle {
        return Some(format!(
            "Sel: {count}x{maximum} {} {total}",
            if same_width { "=" } else { "->" }
        ));
    }
    if count > 99 {
        return Some(format!("Sel {count} : {total} | ..."));
    }
    lines.sort_unstable();
    let mut covered = 0;
    let mut previous = None;
    for (first, last) in lines {
        let start = previous.map_or(first, |end: usize| first.max(end + 1));
        if start <= last {
            covered += last - start + 1;
        }
        previous = Some(previous.map_or(last, |end| end.max(last)));
    }
    Some(format!("Sel {count} : {total} | {covered}"))
}

/// Scintilla serializes direction, selection mode, all ranges and virtual space.
pub fn selection_snapshot(hwnd: HWND) -> Vec<u8> {
    let length = send(hwnd, SCI_GETSELECTIONSERIALIZED, 0, 0).max(0) as usize;
    let mut value = vec![0; length + 1];
    send(
        hwnd,
        SCI_GETSELECTIONSERIALIZED,
        0,
        value.as_mut_ptr() as isize,
    );
    value
}

pub fn restore_selection(hwnd: HWND, snapshot: &[u8]) {
    if snapshot.last() == Some(&0) {
        send(
            hwnd,
            SCI_SETSELECTIONSERIALIZED,
            0,
            snapshot.as_ptr() as isize,
        );
        send(hwnd, SCI_CHOOSECARETX, 0, 0);
    }
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
    send(hwnd, SCI_SETMARGINTYPEN, 1, SC_MARGIN_SYMBOL as isize);
    send(hwnd, SCI_SETMARGINWIDTHN, 1, 16);
    send(hwnd, SCI_SETMARGINMASKN, 1, 1 | (1 << 18) | (1 << 19));
    update_symbol_margin(hwnd, true);
    send(hwnd, SCI_SETMARGINSENSITIVEN, 1, 1);
    send(hwnd, SCI_MARKERDEFINE, 0, SC_MARK_BOOKMARK as isize);
    send(hwnd, SCI_MARKERSETFORE, 0, 0xffffff);
    send(hwnd, SCI_MARKERSETBACK, 0, 0xd47830);
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
    // Match upstream editor initialization: typing and native deletion act on
    // every selection, and rectangles can extend past short line endings.
    send(hwnd, SCI_SETMULTIPLESELECTION, 1, 0);
    send(hwnd, SCI_SETADDITIONALSELECTIONTYPING, 1, 0);
    send(hwnd, SCI_SETVIRTUALSPACEOPTIONS, 1, 0); // SCVS_RECTANGULARSELECTION
    send(hwnd, SCI_SETMOUSESELECTIONRECTANGULARSWITCH, 1, 0);
    send(hwnd, SCI_SETSELFORE, 1, 0xffffff);
    send(hwnd, SCI_SETSELBACK, 1, 0xb57933);
    send(hwnd, SCI_SETCARETLINEVISIBLE, 1, 0);
    send(hwnd, SCI_SETCARETLINEBACK, 0xfaf5f0, 0);
    send(hwnd, SCI_SETSCROLLWIDTH, 1, 0);
    send(hwnd, SCI_SETSCROLLWIDTHTRACKING, 1, 0);
    Ok(())
}

pub fn fold_symbols(hwnd: HWND, style: usize) {
    let markers = [
        SC_MARKNUM_FOLDEROPEN,
        SC_MARKNUM_FOLDER,
        SC_MARKNUM_FOLDERSUB,
        SC_MARKNUM_FOLDERTAIL,
        SC_MARKNUM_FOLDEREND,
        SC_MARKNUM_FOLDEROPENMID,
        SC_MARKNUM_FOLDERMIDTAIL,
    ];
    let symbols = match style {
        1 => [
            SC_MARK_MINUS,
            SC_MARK_PLUS,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
        ],
        2 => [
            SC_MARK_ARROWDOWN,
            SC_MARK_ARROW,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
            SC_MARK_EMPTY,
        ],
        3 => [
            SC_MARK_CIRCLEMINUS,
            SC_MARK_CIRCLEPLUS,
            SC_MARK_VLINE,
            SC_MARK_LCORNERCURVE,
            SC_MARK_CIRCLEPLUSCONNECTED,
            SC_MARK_CIRCLEMINUSCONNECTED,
            SC_MARK_TCORNERCURVE,
        ],
        _ => [
            SC_MARK_BOXMINUS,
            SC_MARK_BOXPLUS,
            SC_MARK_VLINE,
            SC_MARK_LCORNER,
            SC_MARK_BOXPLUSCONNECTED,
            SC_MARK_BOXMINUSCONNECTED,
            SC_MARK_TCORNER,
        ],
    };
    for (marker, symbol) in markers.into_iter().zip(symbols) {
        send(hwnd, SCI_MARKERDEFINE, marker as usize, symbol as isize);
    }
    send(
        hwnd,
        SCI_SETMARGINWIDTHN,
        2,
        if style == 0 { 0 } else { 18 },
    );
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
    send(hwnd, SCI_STYLESETFORE, STYLE_BRACELIGHT as usize, 0x0000ff);
    send(hwnd, SCI_STYLESETBACK, STYLE_BRACELIGHT as usize, 0xc0c0c0);
    send(hwnd, SCI_STYLESETBOLD, STYLE_BRACELIGHT as usize, 1);
    send(hwnd, SCI_STYLESETFORE, STYLE_BRACEBAD as usize, 0x0000ff);
    send(hwnd, SCI_STYLESETBACK, STYLE_BRACEBAD as usize, 0xffffff);
    send(hwnd, SCI_STYLESETBOLD, STYLE_BRACEBAD as usize, 1);
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

pub fn sql_backslash_escape(hwnd: HWND, enabled: bool) {
    send(
        hwnd,
        SCI_SETPROPERTY,
        c"sql.backslash.escapes".as_ptr() as usize,
        if enabled { c"1" } else { c"0" }.as_ptr() as isize,
    );
    send(hwnd, SCI_COLOURISE, 0, -1);
}
pub fn bookmarks(hwnd: HWND, lines: impl Iterator<Item = usize>) {
    let expected: Vec<_> = lines.collect();
    let mut actual = Vec::new();
    let mut next = send(hwnd, SCI_MARKERNEXT, 0, 1);
    while next >= 0 {
        actual.push(next as usize);
        next = send(hwnd, SCI_MARKERNEXT, next as usize + 1, 1);
    }
    if actual != expected {
        send(hwnd, SCI_MARKERDELETEALL, 0, 0);
        for line in expected {
            send(hwnd, SCI_MARKERADD, line, 0);
        }
    }
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
        if line < send(hwnd, SCI_GETLINECOUNT, 0, 0) as usize
            && send(hwnd, SCI_GETFOLDLEVEL, line, 0) as u32 & SC_FOLDLEVELHEADERFLAG != 0
        {
            send(hwnd, SCI_FOLDLINE, line, SC_FOLDACTION_CONTRACT as isize);
        }
    }
}
pub fn hidden_sections(hwnd: HWND) -> Vec<(usize, usize)> {
    let mut sections = Vec::new();
    let mut open = None;
    let mut next = send(hwnd, SCI_MARKERNEXT, 0, (1 << 18) | (1 << 19));
    while next >= 0 {
        let line = next as usize;
        let markers = send(hwnd, SCI_MARKERGET, line, 0);
        if markers & (1 << 18) != 0 {
            if let Some(start) = open.take() {
                if line > start + 1 {
                    sections.push((start + 1, line - 1));
                }
            }
        }
        if markers & (1 << 19) != 0 {
            open = Some(line);
        }
        next = send(hwnd, SCI_MARKERNEXT, line + 1, (1 << 18) | (1 << 19));
    }
    sections
}

pub fn restore_hidden_sections(hwnd: HWND, sections: &[(usize, usize)]) {
    send(hwnd, SCI_MARKERDELETEALL, 18, 0);
    send(hwnd, SCI_MARKERDELETEALL, 19, 0);
    let count = send(hwnd, SCI_GETLINECOUNT, 0, 0) as usize;
    for &(first, last) in sections {
        if first > 0 && first <= last && last + 1 < count {
            send(hwnd, SCI_MARKERADD, first - 1, 19);
            send(hwnd, SCI_MARKERADD, last + 1, 18);
            send(hwnd, SCI_HIDELINES, first, last as isize);
        }
    }
}

/// Explicit hidden sections use their own markers, independently of lexer folds.
pub fn hide_selected_lines(hwnd: HWND) {
    let count = send(hwnd, SCI_GETLINECOUNT, 0, 0) as usize;
    if count < 3 {
        return;
    }
    let first = send(
        hwnd,
        SCI_LINEFROMPOSITION,
        send(hwnd, SCI_GETSELECTIONSTART, 0, 0) as usize,
        0,
    ) as usize;
    let last = send(
        hwnd,
        SCI_LINEFROMPOSITION,
        send(hwnd, SCI_GETSELECTIONEND, 0, 0) as usize,
        0,
    ) as usize;
    let mut first = first.max(1);
    let mut last = last.min(count - 2);
    if first > last {
        return;
    }
    let mut open = None;
    let mut sections = Vec::new();
    for line in 0..count {
        let markers = send(hwnd, SCI_MARKERGET, line, 0);
        if markers & (1 << 18) != 0 {
            if let Some(start) = open.take() {
                sections.push((start + 1, line - 1));
            }
        }
        if markers & (1 << 19) != 0 {
            open = Some(line);
        }
    }
    for &(start, end) in &sections {
        if start <= last + 1 && end + 1 >= first {
            first = first.min(start);
            last = last.max(end);
            send(hwnd, SCI_MARKERDELETE, start - 1, 19);
            send(hwnd, SCI_MARKERDELETE, end + 1, 18);
        }
    }
    send(hwnd, SCI_MARKERADD, first - 1, 19);
    send(hwnd, SCI_MARKERADD, last + 1, 18);
    send(hwnd, SCI_HIDELINES, first, last as isize);
    let position = send(hwnd, SCI_POSITIONFROMLINE, first - 1, 0);
    send(hwnd, SCI_GOTOPOS, position as usize, 0);
}

pub fn reveal_hidden_lines(hwnd: HWND, line: usize) -> bool {
    let markers = send(hwnd, SCI_MARKERGET, line, 0);
    let (start, end) = if markers & (1 << 19) != 0 {
        (line as isize, send(hwnd, SCI_MARKERNEXT, line + 1, 1 << 18))
    } else if markers & (1 << 18) != 0 {
        (
            if line > 0 {
                send(hwnd, SCI_MARKERPREVIOUS, line - 1, 1 << 19)
            } else {
                -1
            },
            line as isize,
        )
    } else {
        return false;
    };
    if start < 0 || end <= start {
        // Upstream consumes hidden-marker clicks even when an edit removed
        // the matching boundary; do not accidentally toggle a bookmark.
        if markers & (1 << 19) != 0 {
            send(hwnd, SCI_MARKERDELETE, line, 19);
        }
        if markers & (1 << 18) != 0 {
            send(hwnd, SCI_MARKERDELETE, line, 18);
        }
        return true;
    }
    let folds = collapsed(hwnd);
    send(hwnd, SCI_SHOWLINES, start as usize + 1, end - 1);
    // Revealing an explicit section must not expand closed language folds.
    for header in folds {
        let last = send(hwnd, SCI_GETLASTCHILD, header, -1);
        if last > header as isize {
            send(hwnd, SCI_HIDELINES, header + 1, last);
        }
    }
    send(hwnd, SCI_MARKERDELETE, start as usize, 19);
    send(hwnd, SCI_MARKERDELETE, end as usize, 18);
    true
}

pub fn rehide_marked_lines(hwnd: HWND) {
    let sections = hidden_sections(hwnd);
    let mut expected = std::collections::BTreeMap::new();
    for &(first, last) in &sections {
        *expected.entry(first - 1).or_insert(0isize) |= 1 << 19;
        *expected.entry(last + 1).or_insert(0isize) |= 1 << 18;
    }
    let mask = (1 << 18) | (1 << 19);
    let mut line = send(hwnd, SCI_MARKERNEXT, 0, mask);
    let mut malformed = false;
    while line >= 0 {
        let actual = send(hwnd, SCI_MARKERGET, line as usize, 0) & mask;
        if expected.get(&(line as usize)).copied().unwrap_or(0) != actual {
            malformed = true;
            break;
        }
        line = send(hwnd, SCI_MARKERNEXT, line as usize + 1, mask);
    }
    if malformed {
        // Deleted lines can coalesce or orphan boundary markers. Rebuild only
        // complete sections, making unreachable hidden text visible again.
        let folds = collapsed(hwnd);
        let count = send(hwnd, SCI_GETLINECOUNT, 0, 0);
        send(hwnd, SCI_SHOWLINES, 0, count.saturating_sub(1));
        for header in folds {
            let last = send(hwnd, SCI_GETLASTCHILD, header, -1);
            if last > header as isize {
                send(hwnd, SCI_HIDELINES, header + 1, last);
            }
        }
        restore_hidden_sections(hwnd, &sections);
        return;
    }
    for (first, last) in sections {
        send(hwnd, SCI_HIDELINES, first, last as isize);
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
            let action = if unsafe { !GetPropW(hwnd, w!("ToggleFoldCommands")).is_null() } {
                SC_FOLDACTION_TOGGLE
            } else {
                action
            };
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
    rehide_marked_lines(hwnd);
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
            WM_DPICHANGED_AFTERPARENT => {
                let result = DefSubclassProc(hwnd, msg, wp, lp);
                update_symbol_margin(hwnd, send(hwnd, SCI_GETMARGINWIDTHN, 1, 0) != 0);
                return result;
            }
            WM_LBUTTONUP => {
                let result = DefSubclassProc(hwnd, msg, wp, lp);
                // Automatic margin folding has completed by mouse release.
                rehide_marked_lines(hwnd);
                return result;
            }
            WM_RBUTTONDOWN => {
                if !GetPropW(hwnd, w!("KeepSelectionOnRightClick")).is_null() {
                    let margin =
                        send(hwnd, SCI_POINTXFROMPOSITION, 0, 0) + send(hwnd, SCI_GETXOFFSET, 0, 0);
                    if lp as i16 as isize >= margin {
                        return 0;
                    }
                }
                return DefSubclassProc(hwnd, msg, wp, lp);
            }

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
