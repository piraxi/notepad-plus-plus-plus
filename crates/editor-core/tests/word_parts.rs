use npp_editor_core::{Document, Selection};

#[test]
fn word_parts_follow_source_case_acronym_separator_and_unicode_rules() {
    for (text, forward, backward) in [
        ("HTTPServer2_name", vec![4, 10, 11, 16], vec![12, 10, 4, 0]),
        ("someValue", vec![4, 9], vec![4, 0]),
        ("__foo__bar", vec![5, 10], vec![7, 2, 0]),
        ("é🦀中Foo", vec![9, 12], vec![9, 0]),
        ("a++ \r\nb", vec![1, 3, 6, 7], vec![6, 3, 1, 0]),
        ("\0x", vec![1, 2], vec![1, 0]),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.set_read_only(true);
        for caret in forward {
            doc.move_word_part(true, false).unwrap();
            assert_eq!(
                doc.selection(),
                Selection {
                    anchor: caret,
                    caret
                },
                "right {text:?}"
            );
        }
        let anchor = text.len();
        for caret in backward {
            doc.move_word_part(false, true).unwrap();
            assert_eq!(
                doc.selection(),
                Selection { anchor, caret },
                "left {text:?}"
            );
        }
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
    }
}

#[test]
fn empty_and_end_boundaries_are_clamped() {
    for text in ["", "_", "___", "a", "É", "\u{7f}"] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.move_word_part(false, false).unwrap();
        assert_eq!(doc.selection(), Selection::default());
        doc.move_word_part(true, false).unwrap();
        doc.move_word_part(true, false).unwrap();
        assert_eq!(doc.selection().caret, text.len());
    }
}
