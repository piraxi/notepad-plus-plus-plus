use npp_editor_core::{Document, EditError, Selection};

fn key(text: &str) -> Result<Vec<u8>, EditError> {
    Ok(text.as_bytes().to_vec())
}

#[test]
fn endings_travel_with_lines_and_caret_follows_original_text() {
    let mut doc = Document::from_bytes(b"z\r\na\nb\r\n").unwrap();
    doc.select(Selection {
        anchor: 4,
        caret: 4,
    })
    .unwrap();
    doc.order_lines_locale(false, key).unwrap();
    assert_eq!(doc.text(), "a\nb\r\nz\r\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 1,
            caret: 1
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "z\r\na\nb\r\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 4
        }
    );
}

#[test]
fn unterminated_last_line_transfers_ending_and_preserves_reverse_selection() {
    let mut doc = Document::from_bytes(b"z\r\na").unwrap();
    doc.select(Selection {
        anchor: 4,
        caret: 1,
    })
    .unwrap();
    doc.order_lines_locale(false, key).unwrap();
    assert_eq!(doc.text(), "a\r\nz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 0
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "z\r\na");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 1
        }
    );
}

#[test]
fn selected_range_excludes_next_line_at_its_start() {
    let mut doc = Document::from_bytes(b"before\nz\na\nafter").unwrap();
    doc.select(Selection {
        anchor: 8,
        caret: 11,
    })
    .unwrap();
    doc.order_lines_locale(false, key).unwrap();
    assert_eq!(doc.text(), "before\na\nz\nafter");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 7,
            caret: 11
        }
    );
}

#[test]
fn ties_are_stable_and_final_empty_line_is_excluded() {
    let mut doc = Document::from_bytes(b"B\nb\na\n").unwrap();
    doc.select(Selection {
        anchor: 6,
        caret: 6,
    })
    .unwrap();
    doc.order_lines_locale(false, |s| key(&s.to_ascii_lowercase()))
        .unwrap();
    assert_eq!(doc.text(), "a\nB\nb\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 6,
            caret: 6
        }
    );
    doc.order_lines_locale(true, |s| key(&s.to_ascii_lowercase()))
        .unwrap();
    assert_eq!(doc.text(), "B\nb\na\n");
}

#[test]
fn failed_keys_and_read_only_edits_leave_document_unchanged() {
    let mut doc = Document::from_bytes(b"z\na").unwrap();
    let original = doc.selection();
    assert_eq!(
        doc.order_lines_locale(false, |s| {
            if s == "a" {
                Err(EditError::InvalidSortKey)
            } else {
                key(s)
            }
        }),
        Err(EditError::InvalidSortKey)
    );
    assert_eq!(doc.text(), "z\na");
    assert_eq!(doc.selection(), original);
    assert!(!doc.can_undo());
    doc.set_read_only(true);
    assert_eq!(doc.order_lines_locale(false, key), Err(EditError::ReadOnly));
    assert_eq!(doc.text(), "z\na");
    assert!(!doc.can_undo());
}
