use npp_editor_core::{Document, Selection};

#[test]
fn paragraph_navigation_skips_ascii_blank_lines_and_preserves_anchor() {
    let text = "a\r\nb\n \t\rc\n\nd";
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    doc.set_read_only(true);
    for (position, up, down) in [
        (0, 0, 8),
        (1, 0, 8),
        (3, 0, 8),
        (5, 0, 8),
        (8, 0, 11),
        (9, 8, 11),
        (10, 8, 11),
        (11, 8, 12),
        (12, 11, 12),
    ] {
        for (direction, expected) in [(false, up), (true, down)] {
            doc.select(Selection {
                anchor: 3,
                caret: position,
            })
            .unwrap();
            doc.move_paragraph(direction, true).unwrap();
            assert_eq!(
                doc.selection(),
                Selection {
                    anchor: 3,
                    caret: expected
                },
                "{position}/{direction}"
            );
        }
    }
    assert_eq!(doc.text(), text);
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn empty_edges_and_unicode_whitespace_are_not_special_paragraphs() {
    for text in [
        "",
        " \t",
        "\r\n\n",
        "a\n\u{a0}\nb",
        "a\n\u{c}\nb",
        "🦀\nend",
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.move_paragraph(true, false).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: text.len(),
                caret: text.len()
            }
        );
        doc.move_paragraph(false, false).unwrap();
        assert_eq!(doc.selection(), Selection::default());
    }
}
