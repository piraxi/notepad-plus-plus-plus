use std::{
    io::{self, Read},
    path::Path,
};

/// Identity of loaded/saved bytes, independent of decoding and unsaved edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskFingerprint(pub(crate) blake3::Hash);

impl DiskFingerprint {
    /// Stream file bytes with bounded memory rather than loading another copy.
    pub fn read(path: &Path) -> io::Result<Self> {
        Self::read_cancellable(path, || false)
    }

    /// Cancellation is checked before opening and between bounded reads. It
    /// cannot interrupt a filesystem call already blocked in the operating system.
    pub fn read_cancellable(path: &Path, mut cancelled: impl FnMut() -> bool) -> io::Result<Self> {
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "disk check cancelled",
            ));
        }
        let mut file = std::fs::File::open(path)?;
        Self::read_stream(&mut file, cancelled)
    }

    fn read_stream(
        reader: &mut impl Read,
        mut cancelled: impl FnMut() -> bool,
    ) -> io::Result<Self> {
        let mut hasher = blake3::Hasher::new();
        let mut bytes = [0; 64 * 1024];
        loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "disk check cancelled",
                ));
            }
            match reader.read(&mut bytes) {
                Ok(0) => return Ok(Self(hasher.finalize())),
                Ok(length) => {
                    hasher.update(&bytes[..length]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_digest_matches_loaded_bytes_and_stops_between_chunks() {
        let bytes = vec![b'x'; 200_000];
        let mut cursor = io::Cursor::new(&bytes);
        let digest = DiskFingerprint::read_stream(&mut cursor, || false).unwrap();
        assert_eq!(
            Some(digest),
            crate::Document::from_bytes(&bytes)
                .unwrap()
                .saved_fingerprint()
        );
        let mut cursor = io::Cursor::new(&bytes);
        let mut checks = 0;
        let result = DiskFingerprint::read_stream(&mut cursor, || {
            checks += 1;
            checks > 1
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert_eq!(cursor.position(), 64 * 1024);
    }

    #[test]
    fn cancelled_check_does_not_open_path() {
        let result = DiskFingerprint::read_cancellable(Path::new(""), || true);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }
}
