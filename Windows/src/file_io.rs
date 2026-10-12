use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::null,
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW, ReplaceFileW};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Toggle only the Windows read-only attribute, retaining other file flags.
pub fn toggle_file_read_only(path: &Path) -> io::Result<bool> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_READONLY,
        GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };
    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file path contains NUL",
        ));
    }
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: The path is NUL-terminated and live for both synchronous calls.
    unsafe {
        let attributes = GetFileAttributesW(path.as_ptr());
        if attributes == INVALID_FILE_ATTRIBUTES {
            return Err(io::Error::last_os_error());
        }
        if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a file, not a directory",
            ));
        }
        let read_only = attributes & FILE_ATTRIBUTE_READONLY == 0;
        let mut updated = (attributes ^ FILE_ATTRIBUTE_READONLY) & !FILE_ATTRIBUTE_NORMAL;
        if updated == 0 {
            updated = FILE_ATTRIBUTE_NORMAL;
        }
        if SetFileAttributesW(path.as_ptr(), updated) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(read_only)
    }
}

pub fn normalized_save_path(path: &Path) -> io::Result<PathBuf> {
    if path.as_os_str().encode_wide().any(|ch| ch == 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file path contains NUL",
        ));
    }
    match fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing file name"))?;
            Ok(fs::canonicalize(parent)?.join(name))
        }
        Err(e) => Err(e),
    }
}

struct Staging {
    directory: PathBuf,
    data: PathBuf,
    backup: PathBuf,
    preserve: bool,
}
impl Staging {
    fn create(parent: &Path) -> io::Result<Self> {
        loop {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let directory = parent.join(format!(".npp-rust-{}-{id}.save", std::process::id()));
            match fs::create_dir(&directory) {
                Ok(()) => {
                    return Ok(Self {
                        data: directory.join("new.tmp"),
                        backup: directory.join("original.bak"),
                        directory,
                        preserve: false,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.preserve {
            // Only remove our named files, never recursively delete a directory.
            let _ = fs::remove_file(&self.data);
            let _ = fs::remove_file(&self.backup);
            let _ = fs::remove_dir(&self.directory);
        }
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn install_new(source: &Path, destination: &Path) -> io::Result<()> {
    // SAFETY: Valid NUL-terminated paths; absence of REPLACE_EXISTING prevents
    // overwriting a file created by somebody else during the save.
    if unsafe {
        MoveFileExW(
            wide(source).as_ptr(),
            wide(destination).as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Flush new content before installation. Existing files use ReplaceFileW to
/// preserve creation time, DACL, and named streams. Its backup argument protects
/// the original during documented partial failures; recovery never overwrites
/// a newly appeared destination. This is not a crash-recovery journal.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let target = normalized_save_path(path)?;
    let exists = match fs::metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.permissions().readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "destination is not a writable file",
                ));
            }
            true
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    let mut staging = Staging::create(
        target
            .parent()
            .ok_or_else(|| io::Error::other("missing parent"))?,
    )?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging.data)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if !exists {
        return install_new(&staging.data, &target);
    }
    // SAFETY: Same-volume, stable NUL-terminated paths. Flags deliberately do
    // not ignore metadata merge errors. Backup resides in a newly owned directory.
    if unsafe {
        ReplaceFileW(
            wide(&target).as_ptr(),
            wide(&staging.data).as_ptr(),
            wide(&staging.backup).as_ptr(),
            0,
            null(),
            null(),
        )
    } != 0
    {
        return Ok(());
    }
    recover_failure(&mut staging, &target, io::Error::last_os_error())
}

fn recover_failure(staging: &mut Staging, target: &Path, error: io::Error) -> io::Result<()> {
    // A failed metadata query is not proof that there is no backup to preserve.
    staging.preserve = true;
    let backup_exists = staging.backup.try_exists().map_err(|query_error| io::Error::other(format!(
        "Save failed: {error}. Cannot inspect recovery backup: {query_error}. Recovery files retained in {}", staging.directory.display()
    )))?;
    if backup_exists {
        // Preserve recovery material even if restoring the original succeeds.
        let restored = install_new(&staging.backup, target).is_ok();
        return Err(io::Error::other(format!(
            "Save failed: {error}. Original restored: {restored}. Recovery files retained in {}",
            staging.directory.display()
        )));
    }
    staging.preserve = false;
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    #[test]
    fn toggling_readonly_preserves_other_attributes_and_content() {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_READONLY,
        };
        let fixture = Staging::create(&std::env::temp_dir()).unwrap();
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .attributes(FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_ARCHIVE)
            .open(&fixture.data)
            .unwrap()
            .write_all(b"retained")
            .unwrap();
        let before = fs::metadata(&fixture.data).unwrap().file_attributes();
        let enabled = toggle_file_read_only(&fixture.data).unwrap();
        let protected = fs::metadata(&fixture.data).unwrap().file_attributes();
        let disabled = toggle_file_read_only(&fixture.data).unwrap();
        let after = fs::metadata(&fixture.data).unwrap().file_attributes();
        assert!(enabled);
        assert!(!disabled);
        assert_eq!(protected, before | FILE_ATTRIBUTE_READONLY);
        assert_eq!(after, before);
        assert_eq!(fs::read(&fixture.data).unwrap(), b"retained");
        assert!(toggle_file_read_only(&fixture.directory).is_err());
        assert!(toggle_file_read_only(&fixture.backup).is_err());
        assert!(toggle_file_read_only(Path::new("bad\0path")).is_err());
    }
    #[test]
    fn replacement_preserves_named_stream_and_creation_time() {
        let fixture = Staging::create(&std::env::temp_dir()).unwrap();
        let path = &fixture.data;
        fs::write(path, b"original").unwrap();
        let stream = PathBuf::from(format!("{}:notes", path.display()));
        fs::write(&stream, b"retained stream").unwrap();
        let created = fs::metadata(path).unwrap().creation_time();
        atomic_write(path, b"replacement").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"replacement");
        assert_eq!(fs::read(stream).unwrap(), b"retained stream");
        assert_eq!(fs::metadata(path).unwrap().creation_time(), created);
        assert_eq!(fs::read_dir(&fixture.directory).unwrap().count(), 1);
    }

    #[test]
    fn new_install_never_overwrites_a_racing_destination() {
        let fixture = Staging::create(&std::env::temp_dir()).unwrap();
        fs::write(&fixture.data, b"new").unwrap();
        fs::write(&fixture.backup, b"created by another writer").unwrap();
        assert!(install_new(&fixture.data, &fixture.backup).is_err());
        assert_eq!(
            fs::read(&fixture.backup).unwrap(),
            b"created by another writer"
        );
        assert_eq!(fs::read(&fixture.data).unwrap(), b"new");
    }

    #[test]
    fn partial_failure_recovers_original_without_overwriting_new_files() {
        let fixture = Staging::create(&std::env::temp_dir()).unwrap();
        let mut staging = Staging::create(&fixture.directory).unwrap();
        fs::write(&staging.data, b"unsaved content").unwrap();
        fs::write(&staging.backup, b"original").unwrap();
        assert!(
            recover_failure(
                &mut staging,
                &fixture.data,
                io::Error::other("injected partial failure")
            )
            .is_err()
        );
        assert_eq!(fs::read(&fixture.data).unwrap(), b"original");
        assert!(staging.preserve);
        fs::write(&staging.backup, b"second original").unwrap();
        assert!(
            recover_failure(
                &mut staging,
                &fixture.data,
                io::Error::other("injected partial failure")
            )
            .is_err()
        );
        assert_eq!(fs::read(&fixture.data).unwrap(), b"original");
        assert_eq!(fs::read(&staging.backup).unwrap(), b"second original");
        // Explicitly discard only this test's recovery material.
        staging.preserve = false;
    }

    #[test]
    fn readonly_and_nul_paths_are_rejected_without_changing_content() {
        let fixture = Staging::create(&std::env::temp_dir()).unwrap();
        fs::write(&fixture.data, b"original").unwrap();
        let permissions = fs::metadata(&fixture.data).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&fixture.data, readonly).unwrap();
        let result = atomic_write(&fixture.data, b"changed");
        fs::set_permissions(&fixture.data, permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(&fixture.data).unwrap(), b"original");
        assert!(normalized_save_path(Path::new("invalid\0name")).is_err());
    }
    #[test]
    fn replacement_and_failed_locked_save() {
        let directory = std::env::temp_dir().join(format!("npp-save-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("document.txt");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        let locked = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        assert!(atomic_write(&path, b"must not replace").is_err());
        drop(locked);
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
