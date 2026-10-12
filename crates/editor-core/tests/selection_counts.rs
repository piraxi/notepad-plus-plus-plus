use npp_editor_core::{Document, Selection};

#[test]
fn selected_characters_include_endings_and_exclude_following_line_at_boundary() {
    let mut doc = Document::from_bytes("é🦀\r\nx\ny\r".as_bytes()).unwrap();
    for (start, end, expected) in [
        (0, 0, (0, 1)),
        (0, 6, (2, 1)),
        (0, 7, (3, 1)),
        (0, 8, (4, 1)),
        (0, 9, (5, 2)),
        (0, 10, (6, 2)),
        (8, 12, (4, 2)),
        (12, 12, (0, 1)),
    ] {
        for selection in [
            Selection {
                anchor: start,
                caret: end,
            },
            Selection {
                anchor: end,
                caret: start,
            },
        ] {
            doc.select(selection).unwrap();
            assert_eq!(
                doc.selected_characters_and_lines(),
                expected,
                "range {start}..{end}"
            );
        }
    }
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn combining_marks_count_separately_and_empty_document_has_one_line() {
    let mut doc = Document::from_bytes("a\u{301}🦀".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 0,
        caret: doc.text().len(),
    })
    .unwrap();
    assert_eq!(doc.selected_characters_and_lines(), (3, 1));
    assert_eq!(Document::new().selected_characters_and_lines(), (0, 1));
}
