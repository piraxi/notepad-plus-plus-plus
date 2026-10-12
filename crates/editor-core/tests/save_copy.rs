use npp_editor_core::{Document, Selection};
use std::{io, path::Path};

#[test]
fn copy_retains_identity_savepoint_and_history_on_success_and_failure() {
    let mut doc = Document::new();
    doc.insert("saved\r\n").unwrap();
    doc.save_with(Path::new("original.txt"), |_, _| Ok(()))
        .unwrap();
    let baseline = doc.saved_fingerprint();
    doc.insert("é😀").unwrap();
    doc.select(Selection {
        anchor: 1,
        caret: 3,
    })
    .unwrap();
    doc.set_read_only(true);
    doc.set_file_read_only(true);
    doc.save_copy_with(Path::new("copy.txt"), |path, bytes| {
        assert_eq!(path, Path::new("copy.txt"));
        assert_eq!(bytes, "saved\r\né😀".as_bytes());
        Ok(())
    })
    .unwrap();
    assert!(
        doc.save_copy_with(Path::new("copy.txt"), |_, _| Err(io::Error::other(
            "write failed"
        )))
        .is_err()
    );
    assert_eq!(doc.path(), Some(Path::new("original.txt")));
    assert_eq!(doc.saved_fingerprint(), baseline);
    assert!(doc.is_dirty());
    assert!(doc.is_user_read_only() && doc.is_file_read_only());
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 1,
            caret: 3
        }
    );
    doc.set_read_only(false);
    doc.set_file_read_only(false);
    doc.undo().unwrap();
    assert_eq!(doc.text(), "saved\r\n");
    assert!(!doc.is_dirty());
}
