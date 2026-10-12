use npp_editor_core::{Document, EditError, Selection};

#[test]
fn word_deletion_follows_classes_whitespace_and_line_endings() {
    for (text, caret, rightwards, expected) in [
        ("one  two", 0, true, "two"),
        ("one  two", 5, false, "two"),
        ("one  two", 3, true, "onetwo"),
        ("one++ two", 3, true, "onetwo"),
        ("one++ two", 6, false, "onetwo"),
        ("e\u{301} 🦀x", 0, true, "🦀x"),
        ("e\u{301} 🦀x", 8, false, "e\u{301} x"),
        ("a\r\n\r\n  b", 1, true, "ab"),
        ("a\r\n\r\n  b", 7, false, "ab"),
        ("one\r\ntwo", 0, true, "\r\ntwo"),
        ("one\r\ntwo", 5, false, "onetwo"),
        ("x\u{200d}y", 0, true, "y"),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        let before = Selection {
            anchor: caret,
            caret,
        };
        doc.select(before).unwrap();
        doc.delete_to_word_start(rightwards).unwrap();
        assert_eq!(doc.text(), expected, "{text:?}, {caret}, {rightwards}");
        doc.undo().unwrap();
        assert_eq!(doc.text(), text);
        assert_eq!(doc.selection(), before);
        doc.redo().unwrap();
        assert_eq!(doc.text(), expected);
    }
}

#[test]
fn word_deletion_uses_caret_and_preserves_read_only_state() {
    let mut doc = Document::from_bytes(b"one two three").unwrap();
    doc.select(Selection {
        anchor: 0,
        caret: 7,
    })
    .unwrap();
    doc.delete_to_word_start(false).unwrap();
    assert_eq!(doc.text(), "one  three");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 4
        }
    );
    doc.undo().unwrap();
    doc.delete_to_word_start(true).unwrap();
    assert_eq!(doc.text(), "one twothree");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 7,
            caret: 7
        }
    );
    doc.undo().unwrap();
    doc.set_read_only(true);
    for rightwards in [false, true] {
        assert_eq!(
            doc.delete_to_word_start(rightwards),
            Err(EditError::ReadOnly)
        );
        assert_eq!(doc.text(), "one two three");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 0,
                caret: 7
            }
        );
    }
}

#[test]
fn word_navigation_preserves_anchor_and_read_only_history() {
    let mut doc = Document::from_bytes("one  e\u{301}++\r\n x".as_bytes()).unwrap();
    doc.set_read_only(true);
    for caret in [5, 8, 10, 13, 14, 14] {
        doc.move_word(true, false).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: caret,
                caret
            }
        );
    }
    for caret in [13, 10, 8, 5, 0, 0] {
        doc.move_word(false, true).unwrap();
        assert_eq!(doc.selection(), Selection { anchor: 14, caret });
    }
    doc.move_word(true, true).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 14,
            caret: 5
        }
    );
    // Non-extending movement advances from the caret, not the range's far end.
    doc.move_word(true, false).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 8,
            caret: 8
        }
    );
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn document_edges_do_not_create_undo_steps() {
    let mut doc = Document::new();
    doc.delete_to_word_start(false).unwrap();
    doc.delete_to_word_start(true).unwrap();
    assert!(!doc.can_undo());
}
