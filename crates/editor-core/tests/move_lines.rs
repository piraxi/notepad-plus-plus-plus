use npp_editor_core::{Document, Selection};

#[test]
fn moves_blocks_with_endings_and_restores_original_selection() {
    let text = "a\r\n🦀\rb\nc\n";
    for down in [false, true] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        let selection = Selection {
            anchor: 4 + 3,
            caret: 10,
        };
        doc.select(selection).unwrap();
        doc.move_lines(down).unwrap();
        assert_eq!(
            doc.text(),
            if down {
                "a\r\nc\n🦀\rb\n"
            } else {
                "🦀\rb\na\r\nc\n"
            }
        );
        assert!(doc.selection().anchor > doc.selection().caret);
        doc.undo().unwrap();
        assert_eq!(doc.text(), text);
        assert_eq!(doc.selection(), selection);
    }
}

#[test]
fn moving_across_final_line_transfers_termination() {
    for (text, caret, down, expected, selected) in [
        ("a\nb", 0, true, "b\na", 2..3),
        ("a\nb", 2, false, "b\na", 0..2),
        ("a\n", 0, true, "\na", 1..2),
        ("a\n", 2, false, "\na", 0..1),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.move_lines(down).unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(doc.selection().range(), selected);
        doc.undo().unwrap();
        assert_eq!(doc.text(), text);
    }
}

#[test]
fn boundaries_and_read_only_do_not_mutate() {
    for (caret, down) in [(0, false), (2, true)] {
        let mut doc = Document::from_bytes(b"a\nb").unwrap();
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.move_lines(down).unwrap();
        assert_eq!(doc.text(), "a\nb");
        assert!(!doc.can_undo());
    }
    let mut doc = Document::from_bytes(b"a\nb").unwrap();
    doc.set_read_only(true);
    assert!(doc.move_lines(true).is_err());
    assert_eq!(doc.text(), "a\nb");
}
