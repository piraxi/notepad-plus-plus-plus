use npp_editor_core::{CaseChange, Document, EditError, Selection, TrimMode};

#[test]
fn invert_case_handles_unicode_expansion_and_restores_backward_selection() {
    let mut doc = Document::from_bytes("[aBßİǅ🦀1]".as_bytes()).unwrap();
    let original = Selection {
        anchor: doc.text().len() - 1,
        caret: 1,
    };
    doc.select(original).unwrap();
    doc.change_case(CaseChange::Invert).unwrap();
    assert_eq!(doc.text(), "[AbSSi\u{307}ǆ🦀1]");
    assert_eq!(doc.selection().range(), 1..doc.text().len() - 1);
    doc.undo().unwrap();
    assert_eq!(doc.text(), "[aBßİǅ🦀1]");
    assert_eq!(doc.selection(), original);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "[AbSSi\u{307}ǆ🦀1]");
}

#[test]
fn invert_case_noops_and_read_only_failure_preserve_document() {
    let mut doc = Document::from_bytes("🦀 123".as_bytes()).unwrap();
    doc.change_case(CaseChange::Invert).unwrap();
    doc.select(Selection {
        anchor: 0,
        caret: doc.text().len(),
    })
    .unwrap();
    doc.change_case(CaseChange::Invert).unwrap();
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
    let mut doc = Document::from_bytes(b"Hello").unwrap();
    let selection = Selection {
        anchor: 0,
        caret: 5,
    };
    doc.select(selection).unwrap();
    doc.set_read_only(true);
    assert_eq!(
        doc.change_case(CaseChange::Invert),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "Hello");
    assert_eq!(doc.selection(), selection);
    assert!(!doc.can_undo());
}

#[test]
fn trim_preserves_endings_and_non_ascii_spaces() {
    let mut doc = Document::from_bytes(" \ta \t\r\n\tb \n\u{a0}c\u{a0}\r\t ".as_bytes()).unwrap();
    doc.trim_lines(TrimMode::Both).unwrap();
    assert_eq!(doc.text(), "a\r\nb\n\u{a0}c\u{a0}\r");
    doc.undo().unwrap();
    assert_eq!(doc.text(), " \ta \t\r\n\tb \n\u{a0}c\u{a0}\r\t ");
    assert!(!doc.can_undo());
}

#[test]
fn trim_expands_partial_selection_but_excludes_next_line_start() {
    let mut doc = Document::from_bytes(b" a \n b \n c ").unwrap();
    doc.select(Selection {
        anchor: 8,
        caret: 1,
    })
    .unwrap();
    doc.trim_lines(TrimMode::Both).unwrap();
    assert_eq!(doc.text(), "a\nb\n c ");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 4
        }
    );
    doc.undo().unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 8,
            caret: 1
        }
    );
}

#[test]
fn trim_no_selection_keeps_caret_near_text_and_noops_keep_history() {
    let mut doc = Document::from_bytes(b"  abc  \n").unwrap();
    doc.select(Selection {
        anchor: 4,
        caret: 4,
    })
    .unwrap();
    doc.trim_lines(TrimMode::Leading).unwrap();
    assert_eq!(doc.selection().caret, 2);
    assert_eq!(doc.text(), "abc  \n");
    doc.trim_lines(TrimMode::Trailing).unwrap();
    assert_eq!(doc.text(), "abc\n");
    doc.trim_lines(TrimMode::Both).unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "abc  \n");
}

#[test]
fn unicode_case_expansion_selection_and_history() {
    let mut doc = Document::from_bytes("aßéz".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 5,
        caret: 1,
    })
    .unwrap();
    doc.change_case(CaseChange::Upper).unwrap();
    assert_eq!(doc.text(), "aSSÉz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 1,
            caret: 5
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "aßéz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 5,
            caret: 1
        }
    );
}

#[test]
fn duplicate_caret_line_and_unterminated_final_line() {
    let mut doc = Document::from_bytes(b"ab\ncd").unwrap();
    doc.select(Selection {
        anchor: 0,
        caret: 4,
    })
    .unwrap();
    doc.duplicate_line().unwrap();
    assert_eq!(doc.text(), "ab\ncd\ncd");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 4
        }
    );
    doc.undo().unwrap();
    doc.select(Selection {
        anchor: 2,
        caret: 2,
    })
    .unwrap();
    doc.duplicate_line().unwrap();
    assert_eq!(doc.text(), "ab\nab\ncd");
    assert_eq!(doc.selection().caret, 5);
}

#[test]
fn batch_edits_validate_before_mutating_and_respect_read_only() {
    let mut doc = Document::from_bytes("a🦀b".as_bytes()).unwrap();
    assert_eq!(
        doc.apply_edits(&[(0..1, "x".into()), (2..5, "bad".into())]),
        Err(EditError::InvalidRange)
    );
    assert_eq!(
        doc.apply_edits(&[(0..5, "x".into()), (1..5, "bad".into())]),
        Err(EditError::InvalidRange)
    );
    assert_eq!(doc.text(), "a🦀b");
    assert!(!doc.can_undo());
    doc.set_read_only(true);
    assert_eq!(doc.duplicate_line(), Err(EditError::ReadOnly));
    assert_eq!(
        doc.apply_edits(&[(0..1, "x".into())]),
        Err(EditError::ReadOnly)
    );
}
