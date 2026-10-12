use npp_editor_core::{Document, EncodeError};
use std::{os::windows::fs::MetadataExt, ptr::null};
use windows_sys::Win32::{
    Foundation::{FILETIME, SYSTEMTIME},
    Globalization::*,
    System::Time::*,
};

fn number(value: usize) -> String {
    let digits = value.to_string();
    let mut output = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i != 0 && (digits.len() - i) % 3 == 0 {
            output.push(',');
        }
        output.push(c);
    }
    output
}

fn timestamp(value: u64) -> String {
    let raw = FILETIME {
        dwLowDateTime: value as u32,
        dwHighDateTime: (value >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    let mut date = [0u16; 256];
    let mut time = [0u16; 256];
    // SAFETY: All structures and output buffers are initialized and sized.
    unsafe {
        if FileTimeToSystemTime(&raw, &mut utc) == 0
            || SystemTimeToTzSpecificLocalTime(null(), &utc, &mut local) == 0
        {
            return String::new();
        }
        let d = GetDateFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &local,
            null(),
            date.as_mut_ptr(),
            256,
        );
        let t = GetTimeFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &local,
            null(),
            time.as_mut_ptr(),
            256,
        );
        if d == 0 || t == 0 {
            return String::new();
        }
        format!(
            "{} {}",
            String::from_utf16_lossy(&date[..d as usize - 1]),
            String::from_utf16_lossy(&time[..t as usize - 1])
        )
    }
}

pub fn text(document: &Document) -> Result<String, EncodeError> {
    let stats = document.summary()?;
    let mut result = String::new();
    if let Some(path) = document.path() {
        if let Ok(metadata) = std::fs::metadata(path) {
            result.push_str(&format!(
                "Full file path: {}\r\nCreated: {}\r\nModified: {}\r\n",
                path.display(),
                timestamp(metadata.creation_time()),
                timestamp(metadata.last_write_time())
            ));
        }
    }
    result.push_str(&format!("Characters (without line endings): {}\r\nWords: {}\r\nLines: {}\r\nDocument length: {}\r\n{} selected characters ({} bytes) in {} ranges\r\n",
        number(stats.characters), number(stats.words), number(stats.lines), number(stats.bytes),
        number(stats.selected_characters), number(stats.selected_bytes), number(stats.ranges)));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use npp_editor_core::Selection;
    #[test]
    fn summary_formats_counts_and_selection_without_mutation() {
        let mut doc = Document::from_bytes("é🦀\r\nword".as_bytes()).unwrap();
        doc.select(Selection {
            anchor: 8,
            caret: 0,
        })
        .unwrap();
        assert_eq!(
            text(&doc).unwrap(),
            "Characters (without line endings): 6\r\nWords: 2\r\nLines: 2\r\nDocument length: 12\r\n2 selected characters (8 bytes) in 1 ranges\r\n"
        );
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
        assert_eq!(number(1234567), "1,234,567");
        assert_eq!(number(0), "0");
    }

    #[test]
    fn existing_file_summary_includes_native_metadata_dates() {
        let path = std::env::current_exe().unwrap();
        let mut doc = Document::new();
        // Associate with an existing file without writing or reading its bytes.
        doc.save_with(&path, |_, _| Ok(())).unwrap();
        let result = text(&doc).unwrap();
        assert!(result.starts_with(&format!("Full file path: {}\r\nCreated: ", path.display())));
        assert!(!result.contains("Created: \r\n"));
        assert!(!result.contains("Modified: \r\n"));
        assert!(result.contains("Document length: 0"));
    }
}
