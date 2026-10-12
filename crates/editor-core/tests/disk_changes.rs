use npp_editor_core::{Document, Encoding};
use std::{io, path::Path};

#[test]
fn disk_baseline_survives_edits_undo_and_failed_saves() {
    let mut doc = Document::from_bytes(b"original").unwrap();
    doc.insert("edited").unwrap();
    assert!(!doc.disk_content_changed(b"original"));
    assert!(doc.disk_content_changed(b"external"));
    doc.undo().unwrap();
    doc.redo().unwrap();
    assert!(
        doc.save_with(Path::new("fixture"), |_, _| Err(io::Error::other("failed")))
            .is_err()
    );
    assert!(!doc.disk_content_changed(b"original"));
    let bytes = doc.to_bytes().unwrap();
    doc.save_with(Path::new("fixture"), |_, _| Ok(())).unwrap();
    assert!(!doc.disk_content_changed(&bytes));
    assert!(doc.disk_content_changed(b"original"));
    doc.undo().unwrap();
    assert!(!doc.disk_content_changed(&bytes));
}

#[test]
fn baseline_compares_bytes_including_bom_and_explicit_legacy_encoding() {
    let mut doc = Document::from_bytes_as(&[0x80], Encoding::Windows1252).unwrap();
    assert!(!doc.disk_content_changed(&[0x80]));
    assert!(doc.disk_content_changed("€".as_bytes()));
    doc.reload_from_bytes(&[0xe9]).unwrap();
    assert!(!doc.disk_content_changed(&[0xe9]));
    let doc = Document::from_bytes(b"text").unwrap();
    assert!(doc.disk_content_changed(b"\xef\xbb\xbftext"));
}

#[test]
fn deleted_file_requires_save_even_after_undo_and_failed_save() {
    let mut doc = Document::from_bytes(b"only copy").unwrap();
    doc.save_with(Path::new("fixture"), |_, _| Ok(())).unwrap();
    doc.set_file_read_only(true);
    doc.mark_disk_deleted();
    assert!(doc.is_dirty());
    assert!(!doc.is_file_read_only());
    assert!(!doc.can_undo());
    doc.insert("edit ").unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "only copy");
    assert!(doc.is_dirty());
    assert!(
        doc.save_with(Path::new("fixture"), |_, _| Err(io::Error::other("failed")))
            .is_err()
    );
    assert!(doc.is_dirty());
    doc.save_with(Path::new("fixture"), |_, _| Ok(())).unwrap();
    assert!(!doc.is_dirty());
    doc.mark_disk_deleted();
    doc.reload_from_bytes(b"restored").unwrap();
    assert!(!doc.is_dirty());
}
