use npp_editor_core::{Document, Selection};

fn document(text: &str) -> Document {
    let mut doc = Document::new();
    doc.insert(text).unwrap();
    doc.select(Selection::default()).unwrap();
    doc
}

#[test]
fn modes_distinguish_ascii_blanks_and_preserve_content_and_endings() {
    for (blanks, expected) in [
        (false, "a\r\n \t\n🦀\r\u{a0}\n \t"),
        (true, "a\r\n🦀\r\u{a0}"),
    ] {
        let input = "\r\na\r\n\n \t\n🦀\r\u{a0}\n \t";
        let mut doc = document(input);
        doc.remove_empty_lines(blanks).unwrap();
        assert_eq!(doc.text(), expected);
        doc.undo().unwrap();
        assert_eq!(doc.text(), input);
        doc.redo().unwrap();
        assert_eq!(doc.text(), expected);
    }
}

#[test]
fn removes_final_empty_line_and_whitespace_only_document() {
    for text in ["a\n \n", "a\r\n \t\r\n\t"] {
        let mut doc = document(text);
        doc.remove_empty_lines(true).unwrap();
        assert_eq!(doc.text(), "a");
    }
    for text in ["a\r\n\n", "a\n", "a\r"] {
        let mut doc = document(text);
        doc.remove_empty_lines(false).unwrap();
        assert_eq!(doc.text(), "a");
    }
    for text in [" \t", "\r\n \t\r\n\t", ""] {
        let mut doc = document(text);
        doc.remove_empty_lines(true).unwrap();
        assert_eq!(doc.text(), "");
    }
}

#[test]
fn exact_selection_does_not_expand_partial_blank_line() {
    let mut doc = document("a\n  \n\nb\n");
    doc.select(Selection {
        anchor: 3,
        caret: 6,
    })
    .unwrap();
    doc.remove_empty_lines(true).unwrap();
    assert_eq!(doc.text(), "a\n  \nb\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 3,
            caret: 5
        }
    );
    doc.undo().unwrap();
    doc.select(Selection {
        anchor: 2,
        caret: 4,
    })
    .unwrap();
    doc.remove_empty_lines(true).unwrap();
    assert_eq!(doc.text(), "a\n  \n\nb\n");
}

#[test]
fn read_only_failure_and_noop_leave_document_unchanged() {
    let mut doc = document("a\n\nb");
    doc.set_read_only(true);
    assert!(doc.remove_empty_lines(false).is_err());
    assert_eq!(doc.text(), "a\n\nb");
    let mut doc = document("a\nb");
    doc.remove_empty_lines(true).unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "");
}

#[test]
fn large_empty_runs_and_alternating_blank_lines_restore_in_one_undo() {
    // Long adjacent newline runs used to be rescanned from every line start.
    // Exercise both one contiguous removal and many separated removals without
    // a machine-dependent timing assertion.
    for (input, expected, blanks) in [
        (
            format!("head\r\n{}tail", "\r\n".repeat(100_000)),
            "head\r\ntail".to_owned(),
            false,
        ),
        (
            format!("head\n{}", " \t\r\n".repeat(100_000)),
            "head".to_owned(),
            true,
        ),
        (
            "x\n \t\n".repeat(25_000),
            format!("{}x", "x\n".repeat(24_999)),
            true,
        ),
    ] {
        let mut doc = document(&input);
        doc.remove_empty_lines(blanks).unwrap();
        assert_eq!(doc.text(), expected);
        doc.undo().unwrap();
        assert_eq!(doc.text(), input);
        doc.redo().unwrap();
        assert_eq!(doc.text(), expected);
    }
}
