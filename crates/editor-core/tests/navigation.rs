use npp_editor_core::Document;

#[test]
fn line_navigation_clamps_and_handles_mixed_endings() {
    let mut doc = Document::new();
    doc.insert("é\r\nx\ry\n").unwrap();
    doc.set_read_only(true);
    for (line, expected) in [(-9, 0), (0, 0), (1, 0), (2, 4), (3, 6), (4, 8), (99, 8)] {
        doc.go_to_line(line);
        assert_eq!(doc.selection().caret, expected);
        assert_eq!(doc.selection().anchor, expected);
    }
    doc.set_read_only(false);
    doc.undo().unwrap();
    assert_eq!(doc.text(), "");
    doc.go_to_line(i64::MAX);
    assert_eq!(doc.selection().caret, 0);
}

#[test]
fn offsets_round_forward_out_of_unicode_and_crlf() {
    let mut doc = Document::new();
    doc.insert("é😀\r\nz").unwrap();
    doc.set_read_only(true);
    for (offset, expected) in [
        (-1, 0),
        (0, 0),
        (1, 2),
        (2, 2),
        (3, 6),
        (5, 6),
        (6, 6),
        (7, 8),
        (8, 8),
        (99, 9),
    ] {
        doc.go_to_offset(offset);
        assert_eq!(doc.selection().caret, expected);
        assert_eq!(doc.selection().anchor, expected);
    }
    doc.set_read_only(false);
    doc.undo().unwrap();
    assert!(doc.text().is_empty());
}
