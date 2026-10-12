use npp_editor_core::EditError;
use std::ptr::null;
use windows_sys::Win32::Globalization::*;

pub fn key(text: &str) -> Result<Vec<u8>, EditError> {
    let text: Vec<u16> = text.encode_utf16().collect();
    let length = i32::try_from(text.len()).map_err(|_| EditError::InvalidSortKey)?;
    let flags =
        LCMAP_SORTKEY | NORM_LINGUISTIC_CASING | LINGUISTIC_IGNORECASE | SORT_DIGITSASNUMBERS;
    // SAFETY: Source units are valid for the explicit length. SORTKEY sizes and
    // output counts are bytes despite LCMapStringEx's nominal UTF-16 pointer.
    unsafe {
        let size = LCMapStringEx(
            null(),
            flags,
            text.as_ptr(),
            length,
            std::ptr::null_mut(),
            0,
            null(),
            std::ptr::null_mut(),
            0,
        );
        if size == 0 {
            return Err(EditError::InvalidSortKey);
        }
        let mut output = vec![0u8; size as usize];
        let copied = LCMapStringEx(
            null(),
            flags,
            text.as_ptr(),
            length,
            output.as_mut_ptr().cast(),
            size,
            null(),
            std::ptr::null_mut(),
            0,
        );
        if copied == 0 {
            return Err(EditError::InvalidSortKey);
        }
        Ok(output)
    }
}
