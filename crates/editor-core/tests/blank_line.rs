use npp_editor_core::{Document, Selection};

#[test]
fn inserts_above_and_below_with_each_configured_ending_and_undo() {
    for ending in ["\r\n", "\r", "\n"] {
        let text = format!("a{ending}🦀{ending}c");
        for below in [false, true] {
            let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
            let caret = 1 + ending.len() + 4;
            let selection = Selection { anchor: 0, caret };
            doc.select(selection).unwrap();
            doc.insert_blank_line(below).unwrap();
            let expected = if below {
                format!("a{ending}🦀{ending}{ending}c")
            } else {
                format!("a{ending}{ending}🦀{ending}c")
            };
            assert_eq!(doc.text(), expected);
            let target = if below {
                caret + ending.len()
            } else {
                1 + ending.len()
            };
            assert_eq!(
                doc.selection(),
                Selection {
                    anchor: target,
                    caret: target
                }
            );
            doc.undo().unwrap();
            assert_eq!(doc.text(), text);
            assert_eq!(doc.selection(), selection);
            doc.redo().unwrap();
            assert_eq!(doc.text(), expected);
        }
    }
}

#[test]
fn first_last_and_empty_document_insertions() {
    for (text, caret, below, expected, target) in [
        ("abc", 1, false, "\r\nabc", 0),
        ("abc", 1, true, "abc\r\n", 5),
        ("", 0, false, "\r\n", 0),
        ("", 0, true, "\r\n", 2),
        ("a\r\n", 3, true, "a\r\n\r\n", 5),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.insert_blank_line(below).unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(doc.selection().caret, target);
    }
}

#[test]
fn invalid_mixed_ending_offsets_and_read_only_fail_atomically() {
    for below in [false, true] {
        let mut doc = Document::new(); // CRLF preference, pasted LF text.
        doc.insert("🦀\n🦀").unwrap();
        let caret = if below { 0 } else { 5 };
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        assert!(doc.insert_blank_line(below).is_err());
        assert_eq!(doc.text(), "🦀\n🦀");
        assert_eq!(doc.selection().caret, caret);
    }
    let mut doc = Document::from_bytes(b"a\r\nb").unwrap();
    doc.set_read_only(true);
    assert!(doc.insert_blank_line(true).is_err());
    assert_eq!(doc.text(), "a\r\nb");
    assert!(!doc.can_undo());
}
