use npp_editor_core::Document;

#[test]
fn user_and_file_protection_are_independent_of_text_history() {
    let mut doc = Document::from_bytes(b"abc").unwrap();
    doc.insert("x").unwrap();
    for file in [false, true] {
        doc.set_file_read_only(file);
        doc.set_read_only(true);
        assert!(doc.is_read_only());
        assert!(doc.insert("!").is_err());
        assert!(doc.undo().is_err());
        doc.set_read_only(false);
        assert_eq!(doc.is_read_only(), file);
        assert_eq!(doc.is_file_read_only(), file);
        assert!(doc.is_dirty());
    }
    doc.set_file_read_only(false);
    doc.undo().unwrap();
    assert_eq!(doc.text(), "abc");
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "xabc");
}

#[test]
fn reloading_preserves_both_protection_sources() {
    let mut doc = Document::from_bytes(b"abc").unwrap();
    doc.set_read_only(true);
    doc.set_file_read_only(true);
    doc.reload_from_bytes(b"new").unwrap();
    assert!(doc.is_user_read_only());
    assert!(doc.is_file_read_only());
    assert_eq!(doc.text(), "new");
    assert!(!doc.is_dirty());
}
