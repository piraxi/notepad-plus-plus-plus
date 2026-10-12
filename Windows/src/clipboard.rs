use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    System::{DataExchange::*, Memory::*},
};

struct TextMemory(HGLOBAL);
impl TextMemory {
    fn new(text: &OsStr) -> io::Result<Self> {
        let units: Vec<u16> = text.encode_wide().chain(Some(0)).collect();
        // SAFETY: Allocation matches the complete UTF-16 payload including NUL.
        unsafe {
            let memory = Self(GlobalAlloc(GMEM_MOVEABLE, units.len() * 2));
            if memory.0.is_null() {
                return Err(io::Error::last_os_error());
            }
            let pointer = GlobalLock(memory.0).cast::<u16>();
            if pointer.is_null() {
                return Err(io::Error::last_os_error());
            }
            std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
            GlobalUnlock(memory.0);
            Ok(memory)
        }
    }
}
impl Drop for TextMemory {
    fn drop(&mut self) {
        // SAFETY: Untransferred allocations remain exclusively owned here.
        unsafe {
            if !self.0.is_null() {
                GlobalFree(self.0);
            }
        }
    }
}

struct Clipboard;
impl Drop for Clipboard {
    fn drop(&mut self) {
        // SAFETY: Created only after OpenClipboard succeeds on this thread.
        unsafe {
            CloseClipboard();
        }
    }
}

pub fn write_text(owner: HWND, text: &str) -> io::Result<()> {
    write_os_text(owner, OsStr::new(text))
}

fn decode_text(units: &[u16]) -> io::Result<String> {
    let end = units.iter().position(|&unit| unit == 0).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Clipboard text is not terminated",
        )
    })?;
    Ok(String::from_utf16_lossy(&units[..end]))
}

pub fn read_text(owner: HWND) -> io::Result<Option<String>> {
    const CF_UNICODETEXT: u32 = 13;
    // SAFETY: Clipboard ownership and memory locking bound the handle lifetime;
    // GlobalSize bounds the read, including malformed unterminated payloads.
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(None);
        }
        if OpenClipboard(owner) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _clipboard = Clipboard;
        let memory = GetClipboardData(CF_UNICODETEXT);
        if memory.is_null() {
            return Err(io::Error::last_os_error());
        }
        let bytes = GlobalSize(memory);
        if bytes < 2 || bytes > isize::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid clipboard text size",
            ));
        }
        let pointer = GlobalLock(memory).cast::<u16>();
        if pointer.is_null() {
            return Err(io::Error::last_os_error());
        }
        let result = decode_text(std::slice::from_raw_parts(pointer, bytes / 2));
        GlobalUnlock(memory);
        result.map(Some)
    }
}

pub fn write_os_text(owner: HWND, text: &OsStr) -> io::Result<()> {
    const CF_UNICODETEXT: u32 = 13;
    let mut memory = TextMemory::new(text)?;
    // SAFETY: Caller provides the editor window as clipboard owner. Windows takes
    // ownership of the movable allocation only after SetClipboardData succeeds.
    unsafe {
        if OpenClipboard(owner) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _clipboard = Clipboard;
        if EmptyClipboard() == 0 {
            return Err(io::Error::last_os_error());
        }
        if SetClipboardData(CF_UNICODETEXT, memory.0).is_null() {
            return Err(io::Error::last_os_error());
        }
        memory.0 = null_mut();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipboard_decoding_is_bounded_and_preserves_unicode_and_endings() {
        let units: Vec<_> = "🦀\r\nx\ny\r".encode_utf16().chain([0, 99]).collect();
        assert_eq!(decode_text(&units).unwrap(), "🦀\r\nx\ny\r");
        assert_eq!(decode_text(&[0]).unwrap(), "");
        assert!(decode_text(&[65, 66]).is_err());
        assert_eq!(decode_text(&[0xd800, 0]).unwrap(), "\u{fffd}");
    }
    #[test]
    fn native_path_units_survive_clipboard_encoding() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let units = [
            b'C' as u16,
            b':' as u16,
            b'\\' as u16,
            0xd800,
            b'\r' as u16,
            b'\n' as u16,
        ];
        let memory = TextMemory::new(&OsString::from_wide(&units)).unwrap();
        // SAFETY: Reads initialized UTF-16 units, including the added terminator.
        unsafe {
            let pointer = GlobalLock(memory.0).cast::<u16>();
            assert!(!pointer.is_null());
            assert_eq!(std::slice::from_raw_parts(pointer, units.len()), units);
            assert_eq!(*pointer.add(units.len()), 0);
            GlobalUnlock(memory.0);
        }
    }

    #[test]
    fn unicode_payload_preserves_mixed_endings_and_terminator() {
        let text = "é😀\r\nx\ny\r";
        let memory = TextMemory::new(OsStr::new(text)).unwrap();
        let expected: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: Test reads only the initialized units of its own allocation.
        unsafe {
            let pointer = GlobalLock(memory.0).cast::<u16>();
            assert!(!pointer.is_null());
            assert_eq!(
                std::slice::from_raw_parts(pointer, expected.len()),
                expected
            );
            GlobalUnlock(memory.0);
        }
    }
}
