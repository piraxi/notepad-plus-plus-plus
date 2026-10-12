use std::{
    io,
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::HWND,
    System::Com::*,
    UI::{
        Shell::*,
        WindowsAndMessaging::{SW_SHOW, SW_SHOWNORMAL},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Reveal,
    DefaultViewer,
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn has_association(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let path = wide(path);
    let mut command = [0u16; 260];
    let mut length = command.len() as u32;
    // SAFETY: All input and output buffers remain live through the shell query.
    unsafe {
        if AssocQueryStringW(
            ASSOCF_VERIFY | ASSOCF_INIT_IGNOREUNKNOWN,
            ASSOCSTR_COMMAND,
            PathFindExtensionW(path.as_ptr()),
            null(),
            command.as_mut_ptr(),
            &mut length,
        ) != 0
        {
            return false;
        }
    }
    let name = "notepad++.exe".encode_utf16().collect::<Vec<_>>();
    // Match the source's literal (case-sensitive) association exclusion.
    !command.windows(name.len()).any(|part| part == name)
}

struct Com;
impl Com {
    fn new() -> io::Result<Self> {
        // SAFETY: Initializes COM on the calling UI thread. Every successful
        // initialization, including S_FALSE, is balanced by the guard's drop.
        unsafe {
            let mut result = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
            if result == 0x80010106u32 as i32 {
                result = CoInitializeEx(null(), COINIT_MULTITHREADED as u32);
            }
            if result < 0 {
                return Err(hresult(result));
            }
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: This guard represents one successful initialization.
        unsafe {
            CoUninitialize();
        }
    }
}
fn hresult(value: i32) -> io::Error {
    io::Error::other(format!("Windows shell error 0x{:08X}", value as u32))
}

fn execute(owner: HWND, path: &Path, explore: bool) -> io::Result<()> {
    let path = wide(path);
    let verb: Vec<u16> = if explore { "explore" } else { "open" }
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: ShellExecute receives separate terminated filename and verb
    // arguments; paths are never interpolated into shell command text.
    let result = unsafe {
        ShellExecuteW(
            owner,
            verb.as_ptr(),
            path.as_ptr(),
            null(),
            null(),
            if explore { SW_SHOWNORMAL } else { SW_SHOW },
        )
    } as isize;
    if result <= 32 {
        Err(io::Error::other(format!(
            "Windows could not open the file (shell error {result})"
        )))
    } else {
        Ok(())
    }
}

pub fn launch(owner: HWND, path: &Path, action: Action) -> io::Result<()> {
    if action == Action::DefaultViewer {
        return execute(owner, path, false);
    }
    if !path.exists() {
        return execute(owner, path.parent().unwrap_or(path), true);
    }
    let _com = Com::new()?;
    let units = wide(path);
    let mut item = null_mut();
    // SAFETY: The shell initializes an owned PIDL, which is released after the
    // selection request on every successful parse path.
    unsafe {
        let parsed = SHParseDisplayName(units.as_ptr(), null_mut(), &mut item, 0, null_mut());
        if parsed < 0 {
            if parsed == 0x80070002u32 as i32 {
                return execute(owner, path.parent().unwrap_or(path), true);
            }
            return Err(hresult(parsed));
        }
        let opened = SHOpenFolderAndSelectItems(item, 0, null(), 0);
        CoTaskMemFree(item.cast());
        if opened < 0 {
            if opened == 0x80070002u32 as i32 {
                return execute(owner, path.parent().unwrap_or(path), true);
            }
            return Err(hresult(opened));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn com_guard_supports_existing_mta_and_parses_real_file_without_opening_it() {
        std::thread::spawn(|| {
            // SAFETY: This disposable thread owns its balanced COM initialization
            // and the shell PIDL. Parsing a path does not launch an application.
            unsafe {
                assert!(CoInitializeEx(null(), COINIT_MULTITHREADED as u32) >= 0);
                {
                    let _guard = Com::new().unwrap();
                    let units = wide(&std::env::current_exe().unwrap());
                    let mut item = null_mut();
                    assert!(
                        SHParseDisplayName(units.as_ptr(), null_mut(), &mut item, 0, null_mut())
                            >= 0
                    );
                    assert!(!item.is_null());
                    CoTaskMemFree(item.cast());
                }
                CoUninitialize();
            }
        })
        .join()
        .unwrap();
    }
}
