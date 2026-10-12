use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf, ptr::null_mut};
use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};

struct OwnedDrop(HDROP);
impl Drop for OwnedDrop {
    fn drop(&mut self) {
        // SAFETY: Owns the handle transferred by WM_DROPFILES; release exactly once.
        unsafe {
            DragFinish(self.0);
        }
    }
}

/// Takes ownership of a valid WM_DROPFILES payload and snapshots every path
/// before releasing the shell allocation. Does not impose a MAX_PATH buffer.
pub unsafe fn take_paths(handle: HDROP) -> Vec<PathBuf> {
    let owned = OwnedDrop(handle);
    let mut paths = Vec::new();
    // SAFETY: Caller transferred a valid HDROP. Query sizes before allocating,
    // and preserve UTF-16 paths as OsString rather than decoding lossily.
    unsafe {
        let count = DragQueryFileW(owned.0, u32::MAX, null_mut(), 0);
        for index in 0..count {
            let length = DragQueryFileW(owned.0, index, null_mut(), 0);
            if length == 0 {
                continue;
            }
            let Some(capacity) = length.checked_add(1) else {
                continue;
            };
            let mut text = vec![0u16; capacity as usize];
            let copied = DragQueryFileW(owned.0, index, text.as_mut_ptr(), capacity);
            if copied == length {
                paths.push(PathBuf::from(OsString::from_wide(&text[..copied as usize])));
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{mem::size_of, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::{System::Memory::*, UI::Shell::DROPFILES};

    #[test]
    fn shell_drop_extracts_multiple_unicode_and_long_paths() {
        let paths = vec![
            PathBuf::from("C:\\example\\日本 🦀.txt"),
            PathBuf::from(format!("C:\\{}\\long.txt", "segment\\".repeat(50))),
        ];
        let mut units = Vec::new();
        for path in &paths {
            units.extend(path.as_os_str().encode_wide());
            units.push(0);
        }
        units.push(0);
        // SAFETY: Construct a real shell-compatible movable DROPFILES block.
        // take_paths owns it after unlocking, and calls DragFinish to free it.
        unsafe {
            let block = GlobalAlloc(
                GMEM_MOVEABLE | GMEM_ZEROINIT,
                size_of::<DROPFILES>() + units.len() * 2,
            );
            assert!(!block.is_null());
            let pointer = GlobalLock(block).cast::<u8>();
            assert!(!pointer.is_null());
            let header = DROPFILES {
                pFiles: size_of::<DROPFILES>() as u32,
                fWide: 1,
                ..Default::default()
            };
            pointer.cast::<DROPFILES>().write(header);
            std::ptr::copy_nonoverlapping(
                units.as_ptr().cast::<u8>(),
                pointer.add(size_of::<DROPFILES>()),
                units.len() * 2,
            );
            GlobalUnlock(block);
            assert_eq!(take_paths(block as HDROP), paths);
        }
    }
}
