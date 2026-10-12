use std::{
    ffi::{OsStr, OsString},
    os::windows::ffi::{OsStrExt, OsStringExt},
};

#[derive(Clone, Copy)]
pub enum Part {
    Full,
    Name,
    Directory,
}

pub fn text(path: &OsStr, part: Part) -> OsString {
    let mut units: Vec<_> = path.encode_wide().collect();
    match part {
        Part::Full => (),
        Part::Name => {
            units.push(0);
            // SAFETY: PathFindFileNameW returns a pointer inside the terminated
            // input, which remains allocated for the entire calculation.
            let offset = unsafe {
                windows_sys::Win32::UI::Shell::PathFindFileNameW(units.as_ptr())
                    .offset_from(units.as_ptr()) as usize
            };
            return OsString::from_wide(&units[offset..units.len() - 1]);
        }
        Part::Directory => {
            // Common.cpp::pathRemoveFileSpec, used by the clipboard command,
            // differs from the shell helper used by plugin directory queries.
            let length = match units.iter().rposition(|&c| c == b'\\' as u16) {
                Some(2) if units.get(1) == Some(&(b':' as u16)) => 3,
                Some(0) if units.len() > 1 => 1,
                Some(position) => position,
                None if units.get(1) == Some(&(b':' as u16)) => 2,
                None => 0,
            };
            units.truncate(length);
        }
    }
    OsString::from_wide(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_matches_source_roots_and_relative_paths() {
        for (path, expected) in [
            (r"C:\folder\file.txt", r"C:\folder"),
            (r"C:\file.txt", r"C:\"),
            (r"\file.txt", r"\"),
            (r"C:file.txt", "C:"),
            (r"\\server\share\file.txt", r"\\server\share"),
            ("new 1", ""),
            (r"\", ""),
            ("C:/file.txt", "C:"),
        ] {
            assert_eq!(
                text(OsStr::new(path), Part::Directory),
                OsStr::new(expected)
            );
        }
    }

    #[test]
    fn names_and_full_paths_keep_native_utf16() {
        let units = [67, 58, 92, 0xd800, 92, 0xdfff, 46, 116];
        let path = OsString::from_wide(&units);
        assert_eq!(
            text(&path, Part::Full).encode_wide().collect::<Vec<_>>(),
            units
        );
        assert_eq!(
            text(&path, Part::Name).encode_wide().collect::<Vec<_>>(),
            &units[5..]
        );
        assert_eq!(
            text(&path, Part::Directory)
                .encode_wide()
                .collect::<Vec<_>>(),
            &units[..4]
        );
        assert_eq!(text(OsStr::new("new 2"), Part::Name), "new 2");
    }
}
