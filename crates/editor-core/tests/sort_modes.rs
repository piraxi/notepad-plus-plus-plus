use npp_editor_core::{Document, LineOrder, Selection};

#[test]
fn case_insensitive_sort_preserves_spelling_endings_and_undo() {
    let mut doc = Document::new();
    let original = "z\r\nB\na\rb\r\nÉ\né";
    doc.insert(original).unwrap();
    doc.order_lines(LineOrder::CaseInsensitiveAscending)
        .unwrap();
    assert_eq!(doc.text(), "B\na\rb\r\nz\r\nÉ\né");
    doc.undo().unwrap();
    assert_eq!(doc.text(), original);
    doc.order_lines(LineOrder::CaseInsensitiveDescending)
        .unwrap();
    assert_eq!(doc.text(), "É\né\r\nz\r\nB\na\rb");
}

#[test]
fn length_is_utf16_units_and_selection_excludes_next_line_start() {
    let mut doc = Document::new();
    doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
        .unwrap();
    doc.insert("keep\nabc\n😀\nx\nend").unwrap();
    doc.select(Selection {
        anchor: 16,
        caret: 5,
    })
    .unwrap();
    doc.order_lines(LineOrder::LengthAscending).unwrap();
    assert_eq!(doc.text(), "keep\nx\n😀\nabc\nend");
    doc.undo().unwrap();
    doc.order_lines(LineOrder::LengthDescending).unwrap();
    assert_eq!(doc.text(), "keep\nabc\n😀\nx\nend");
}
