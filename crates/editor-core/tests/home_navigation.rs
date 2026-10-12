use npp_editor_core::{Document, Selection};

#[test]
fn end_and_document_boundaries_keep_anchor_without_editing() {
    let text = "a🦀\r\nb\rc\n";
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    doc.set_read_only(true);
    for (position, end) in [(0, 5), (5, 5), (6, 5), (7, 8), (9, 10), (11, 11)] {
        doc.select(Selection {
            anchor: 1,
            caret: position,
        })
        .unwrap();
        doc.move_line_end(true).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 1,
                caret: end
            }
        );
        doc.move_line_end(false).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: end,
                caret: end
            }
        );
    }
    doc.select(Selection {
        anchor: 7,
        caret: 1,
    })
    .unwrap();
    for (end, expected) in [(true, 11), (false, 0)] {
        doc.move_document_boundary(end, true).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 7,
                caret: expected
            }
        );
    }
    doc.move_document_boundary(true, false).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 11,
            caret: 11
        }
    );
    assert_eq!(doc.text(), text);
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
    let mut empty = Document::new();
    empty.move_line_end(true).unwrap();
    empty.move_document_boundary(true, true).unwrap();
    assert_eq!(empty.selection(), Selection::default());
}

#[test]
fn home_toggles_only_ascii_indentation_on_each_line() {
    for (text, caret, first, second) in [
        (" \t🦀 tail", 10, 2, 0),
        (" \t🦀 tail", 1, 2, 0),
        ("a\r\n \tb\rc\n", 6, 5, 3),
        ("a\r\n \t\r\n", 3, 5, 3),
        (" \u{a0}x", 4, 1, 0),
        (" \u{c}x", 3, 1, 0),
        ("", 0, 0, 0),
        ("a\n", 2, 2, 2),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.set_read_only(true);
        doc.move_home(false).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: first,
                caret: first
            }
        );
        doc.move_home(true).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: first,
                caret: second
            }
        );
        doc.move_home(true).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: first,
                caret: first
            }
        );
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
        assert_eq!(doc.text(), text);
    }
}
