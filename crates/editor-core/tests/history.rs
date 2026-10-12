use npp_editor_core::{Document, Encoding, LineEnding, SearchOptions, Selection};

#[test]
fn small_edits_in_large_document_store_only_changed_text() {
    let original = "x".repeat(2 * 1024 * 1024);
    let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
    for i in 0..1000 {
        doc.replace(i..i + 1, "y").unwrap();
    }
    assert_eq!(doc.history_text_bytes(), 1000);
    for _ in 0..1000 {
        assert!(doc.undo().unwrap());
    }
    assert_eq!(doc.text(), original);
    assert!(!doc.is_dirty());
    assert_eq!(doc.history_text_bytes(), 1000);
    for _ in 0..1000 {
        assert!(doc.redo().unwrap());
    }
    assert_eq!(&doc.text()[..1000], "y".repeat(1000));
}

#[test]
fn adjacent_deletions_and_insertions_replay_in_original_order() {
    let mut doc = Document::from_bytes("a🦀bc".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 7,
        caret: 1,
    })
    .unwrap();
    doc.apply_edits(&[
        (1..5, String::new()),
        (5..6, String::new()),
        (6..6, "文".into()),
    ])
    .unwrap();
    assert_eq!(doc.text(), "a文c");
    let selection = doc.selection();
    for _ in 0..3 {
        doc.undo().unwrap();
        assert_eq!(doc.text(), "a🦀bc");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 7,
                caret: 1
            }
        );
        doc.redo().unwrap();
        assert_eq!(doc.text(), "a文c");
        assert_eq!(doc.selection(), selection);
    }
}

#[test]
fn metadata_and_line_endings_do_not_snapshot_unchanged_content() {
    let text = format!("{}\r\nend", "a".repeat(100_000));
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    doc.set_encoding(Encoding::Utf16Be).unwrap();
    assert_eq!(doc.history_text_bytes(), 0);
    doc.convert_line_endings(LineEnding::Lf).unwrap();
    assert_eq!(doc.history_text_bytes(), 2);
    doc.undo().unwrap();
    assert_eq!(doc.text(), text);
    assert_eq!(doc.line_ending(), LineEnding::CrLf);
    doc.undo().unwrap();
    assert_eq!(doc.encoding(), Encoding::Utf8);
    doc.redo().unwrap();
    doc.redo().unwrap();
    assert_eq!(doc.line_ending(), LineEnding::Lf);
    assert_eq!(doc.encoding(), Encoding::Utf16Be);
}

#[test]
fn sparse_replace_all_has_sparse_history_and_branch_discards_redo() {
    let text = format!("before {} before", "-".repeat(100_000));
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    assert_eq!(
        doc.replace_matches("before", "after", SearchOptions::default())
            .unwrap(),
        2
    );
    assert_eq!(doc.history_text_bytes(), 12);
    doc.undo().unwrap();
    assert_eq!(doc.text(), text);
    doc.replace(0..1, "B").unwrap();
    assert!(!doc.can_redo());
    assert_eq!(doc.history_text_bytes(), 1);
}
