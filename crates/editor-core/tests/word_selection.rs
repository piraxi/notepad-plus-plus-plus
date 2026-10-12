use npp_editor_core::{Document, Selection};

#[test]
fn word_expansion_checks_both_sides_and_preserves_existing_selection() {
    let mut doc = Document::from_bytes("e\u{301}++ word".as_bytes()).unwrap();
    doc.set_read_only(true);
    for (caret, expected) in [
        (0, 0..3),
        (1, 0..3),
        (3, 0..3),
        (4, 4..4),
        (5, 5..5),
        (6, 6..10),
        (10, 6..10),
    ] {
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.expand_word_selection().unwrap();
        assert_eq!(doc.selection().range(), expected);
    }
    let reverse = Selection {
        anchor: 5,
        caret: 3,
    };
    doc.select(reverse).unwrap();
    doc.expand_word_selection().unwrap();
    assert_eq!(doc.selection(), reverse);
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}
