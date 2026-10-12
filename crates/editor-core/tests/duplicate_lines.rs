use npp_editor_core::{Document, DuplicateLines, EditError, Selection};

#[test]
fn consecutive_duplicates_include_endings_and_keep_unterminated_tail() {
    for (original, expected) in [
        ("a\na\nb\na\n", "a\nb\na\n"),
        ("a\na\na", "a"),
        ("a\r\na\na", "a\r\na"),
        ("\n\n\na\n", "\na\n"),
        ("🦀\r🦀\r🦀", "🦀"),
        ("A\na\n", "A\na\n"),
        ("a \na\n", "a \na\n"),
    ] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        doc.remove_duplicate_lines(DuplicateLines::Consecutive)
            .unwrap();
        assert_eq!(doc.text(), expected, "input {original:?}");
        if original != expected {
            doc.undo().unwrap();
            assert_eq!(doc.text(), original);
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            assert_eq!(doc.text(), expected);
        } else {
            assert!(!doc.can_undo());
        }
    }
}

#[test]
fn any_duplicates_keep_first_occurrence_and_use_configured_ending() {
    for (original, expected) in [
        ("a\nb\na\nc\nb", "a\nb\nc"),
        ("a\n\nb\n", "a\n\nb"),
        ("a\r\nb\r\na\r\n", "a\r\nb\r\n"),
        ("🦀\nA\n🦀\na", "🦀\nA\na"),
        ("a\r\na\na\r\n", "a\r\na\na\r\n"),
    ] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        doc.remove_duplicate_lines(DuplicateLines::Any).unwrap();
        assert_eq!(doc.text(), expected, "input {original:?}");
        if original != expected {
            doc.undo().unwrap();
            assert_eq!(doc.text(), original);
            assert!(!doc.can_undo());
        }
    }
}

#[test]
fn selected_scope_excludes_next_line_and_undo_restores_direction() {
    for mode in [DuplicateLines::Consecutive, DuplicateLines::Any] {
        let mut doc = Document::from_bytes(b"x\na\na\na\nx").unwrap();
        let selection = Selection {
            anchor: 6,
            caret: 3,
        };
        doc.select(selection).unwrap();
        doc.remove_duplicate_lines(mode).unwrap();
        assert_eq!(doc.text(), "x\na\na\nx");
        doc.undo().unwrap();
        assert_eq!(doc.text(), "x\na\na\na\nx");
        assert_eq!(doc.selection(), selection);
        doc.select(Selection {
            anchor: 3,
            caret: 4,
        })
        .unwrap();
        doc.remove_duplicate_lines(mode).unwrap();
        assert_eq!(doc.text(), "x\na\na\na\nx");
        assert!(!doc.can_undo());
    }
}

#[test]
fn readonly_rejection_does_not_change_text_selection_or_history() {
    for mode in [DuplicateLines::Consecutive, DuplicateLines::Any] {
        let mut doc = Document::from_bytes(b"a\na").unwrap();
        doc.set_read_only(true);
        assert_eq!(doc.remove_duplicate_lines(mode), Err(EditError::ReadOnly));
        assert_eq!(doc.text(), "a\na");
        assert_eq!(doc.selection(), Selection::default());
        assert!(!doc.can_undo());
    }
}
