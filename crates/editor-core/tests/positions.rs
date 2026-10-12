use npp_editor_core::{Document, EditError};
use std::num::NonZeroUsize;

#[test]
fn columns_expand_tabs_and_count_unicode_scalars_before_line_endings() {
    let doc = Document::from_bytes("a\t🦀e\u{301}\r\nb".as_bytes()).unwrap();
    let width = NonZeroUsize::new(4).unwrap();
    for (position, line, column) in [
        (0, 0, 0),
        (1, 0, 1),
        (2, 0, 4),
        (6, 0, 5),
        (7, 0, 6),
        (9, 0, 7),
        (10, 0, 7),
        (11, 1, 0),
        (12, 1, 1),
    ] {
        assert_eq!(doc.line_column(position, width).unwrap(), (line, column));
    }
    assert_eq!(
        doc.line_column(2, NonZeroUsize::new(8).unwrap()).unwrap(),
        (0, 8)
    );
    assert_eq!(doc.line_column(3, width), Err(EditError::InvalidRange));
    assert_eq!(doc.line_column(13, width), Err(EditError::InvalidRange));
}

#[test]
fn line_ranges_preserve_endings_and_rebuild_after_edits_and_undo() {
    let mut doc = Document::from_bytes(b"a\r\nb\nc\r").unwrap();
    assert_eq!(doc.line_range(0), Some(0..3));
    assert_eq!(doc.line_range(1), Some(3..5));
    assert_eq!(doc.line_range(2), Some(5..7));
    assert_eq!(doc.line_range(3), Some(7..7));
    assert_eq!(doc.line_range(4), None);
    doc.replace(1..3, "").unwrap();
    assert_eq!(doc.line_range(0), Some(0..3));
    assert_eq!(
        doc.line_column(2, NonZeroUsize::new(4).unwrap()).unwrap(),
        (0, 2)
    );
    doc.undo().unwrap();
    assert_eq!(doc.line_range(3), Some(7..7));
    assert_eq!(Document::new().line_range(0), Some(0..0));
}
