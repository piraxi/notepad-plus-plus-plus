use npp_editor_core::{Encoding, RecentFile, Workspace};
use serde::{Deserialize, Serialize};
use std::{
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

const ENCODINGS: &[Encoding] = &[
    Encoding::Utf8,
    Encoding::Utf8Bom,
    Encoding::Utf16Le,
    Encoding::Utf16Be,
    Encoding::Utf16LeNoBom,
    Encoding::Utf16BeNoBom,
    Encoding::Windows1252,
    Encoding::Windows1251,
    Encoding::ShiftJis,
];
#[derive(Serialize, Deserialize)]
struct Entry {
    path: Vec<u16>,
    encoding: usize,
}
fn path() -> io::Result<PathBuf> {
    let root =
        std::env::var_os("APPDATA").ok_or_else(|| io::Error::other("APPDATA is unavailable"))?;
    Ok(PathBuf::from(root)
        .join("NotepadPlusPlusRust")
        .join("recent-files.json"))
}
pub fn load(workspace: &mut Workspace) -> io::Result<()> {
    let entries: Vec<Entry> =
        serde_json::from_slice(&std::fs::read(path()?)?).map_err(io::Error::other)?;
    let entries = entries.into_iter().filter_map(|entry| {
        let encoding = *ENCODINGS.get(entry.encoding)?;
        if entry.path.is_empty() || entry.path.contains(&0) {
            return None;
        }
        Some(RecentFile {
            path: std::ffi::OsString::from_wide(&entry.path).into(),
            encoding,
        })
    });
    workspace.restore_recent_files(entries);
    Ok(())
}
pub fn save(workspace: &Workspace) -> io::Result<()> {
    let entries: Vec<_> = workspace
        .recent_files()
        .map(|entry| Entry {
            path: entry.path.as_os_str().encode_wide().collect(),
            encoding: ENCODINGS
                .iter()
                .position(|&encoding| encoding == entry.encoding)
                .unwrap_or(0),
        })
        .collect();
    let path = path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    crate::file_io::atomic_write(
        &path,
        &serde_json::to_vec_pretty(&entries).map_err(io::Error::other)?,
    )
}
