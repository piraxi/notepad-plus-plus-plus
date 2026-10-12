use npp_editor_core::{Document, EditError, LineOrder, Selection};

#[test]
fn shuffle_preserves_every_item_and_is_repeatable_with_undo() {
    let text = "one\ntwo\ntwo\n🦀\n";
    let mut distinct = std::collections::HashSet::new();
    for seed in 0..64 {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.order_lines(LineOrder::Random(seed)).unwrap();
        let mut actual: Vec<_> = doc.text().split('\n').collect();
        let mut expected: Vec<_> = text.split('\n').collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        let output = doc.text().to_owned();
        distinct.insert(output.clone());
        if output != text {
            doc.undo().unwrap();
            assert_eq!(doc.text(), text);
            doc.redo().unwrap();
            assert_eq!(doc.text(), output);
        }
        let mut repeat = Document::from_bytes(text.as_bytes()).unwrap();
        repeat.order_lines(LineOrder::Random(seed)).unwrap();
        assert_eq!(repeat.text(), output);
    }
    assert!(distinct.len() > 20);
}

#[test]
fn selected_shuffle_keeps_surrounding_lines_and_configured_delimiter() {
    for seed in 0..32 {
        let mut doc = Document::from_bytes(b"before\r\na\nb\r\nc\r\nafter").unwrap();
        doc.select(Selection {
            anchor: 9,
            caret: 16,
        })
        .unwrap();
        doc.order_lines(LineOrder::Random(seed)).unwrap();
        assert!(
            doc.text() == "before\r\na\nb\r\nc\r\nafter"
                || doc.text() == "before\r\nc\r\na\nb\r\nafter"
        );
        assert_eq!(doc.selection().anchor, 8);
        assert_eq!(doc.selection().caret, 14);
    }
}

#[test]
fn read_only_shuffle_cannot_change_text_or_history() {
    let mut doc = Document::from_bytes(b"a\nb\nc\nd\ne").unwrap();
    doc.set_read_only(true);
    for seed in 0..16 {
        assert_eq!(
            doc.order_lines(LineOrder::Random(seed)),
            Err(EditError::ReadOnly)
        );
        assert_eq!(doc.text(), "a\nb\nc\nd\ne");
        assert!(!doc.can_undo());
    }
}
