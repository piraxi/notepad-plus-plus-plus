use npp_editor_core::{Document, EditError, LineOrder, Selection};

#[test]
fn mixed_selection_uses_configured_delimiter_and_retains_boundary() {
    let mut doc = Document::from_bytes(b"z\r\na\nb\r\nafter").unwrap();
    doc.select(Selection {
        anchor: 0,
        caret: 8,
    })
    .unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert_eq!(doc.text(), "a\nb\r\nz\r\nafter");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "z\r\na\nb\r\nafter");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 8
        }
    );
}

#[test]
fn nonmatching_line_endings_do_not_create_separate_sort_keys() {
    let mut doc = Document::new();
    doc.insert("b\na").unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert_eq!(doc.text(), "b\na");
    doc.undo().unwrap();
    assert!(doc.text().is_empty());
}

#[test]
fn full_document_sort_includes_final_empty_line() {
    let mut doc = Document::from_bytes(b"b\na\n").unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert_eq!(doc.text(), "\na\nb");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "b\na\n");
    doc.redo().unwrap();
    assert_eq!(doc.text(), "\na\nb");
}

#[test]
fn selection_expands_but_excludes_next_line_at_boundary() {
    let mut doc = Document::from_bytes(b"before\nc\nb\nafter").unwrap();
    doc.select(Selection {
        anchor: 11,
        caret: 8,
    })
    .unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert_eq!(doc.text(), "before\nb\nc\nafter");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 7,
            caret: 10
        }
    );
    doc.undo().unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 11,
            caret: 8
        }
    );
}

#[test]
fn utf16_order_matches_windows_instead_of_unicode_scalar_order() {
    let mut doc = Document::from_bytes("\u{e000}\n🦀\nA".as_bytes()).unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert_eq!(doc.text(), "A\n🦀\n\u{e000}");
    doc.order_lines(LineOrder::Descending).unwrap();
    assert_eq!(doc.text(), "\u{e000}\n🦀\nA");
}

#[test]
fn reverse_splits_on_configured_ending_and_readonly_refuses_changes() {
    let mut doc = Document::from_bytes(b"one\r\ntwo\nthree\rfour").unwrap();
    doc.order_lines(LineOrder::Reverse).unwrap();
    assert_eq!(doc.text(), "two\nthree\rfour\r\none");
    doc.set_read_only(true);
    assert_eq!(
        doc.order_lines(LineOrder::Reverse),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "two\nthree\rfour\r\none");
}

#[test]
fn selected_single_line_and_already_sorted_text_do_not_create_history() {
    let mut doc = Document::from_bytes(b"a\nb").unwrap();
    doc.order_lines(LineOrder::Ascending).unwrap();
    assert!(!doc.can_undo());
    doc.select(Selection {
        anchor: 0,
        caret: 1,
    })
    .unwrap();
    doc.order_lines(LineOrder::Reverse).unwrap();
    assert_eq!(doc.text(), "a\nb");
    assert!(!doc.can_undo());
}
