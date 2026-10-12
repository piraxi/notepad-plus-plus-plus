use npp_editor_core::{Document, EditError, SearchMode, SearchOptions, Selection};

#[test]
fn scope_requires_complete_matches_and_document_word_boundaries() {
    let doc = Document::from_bytes(b"xcat catz cat").unwrap();
    let options = SearchOptions::default();
    assert_eq!(doc.count_matches_in("cat", options, 1..7).unwrap(), 1);
    assert_eq!(doc.count_matches_in("cat", options, 2..8).unwrap(), 1);
    assert_eq!(doc.count_matches_in("cat", options, 4..7).unwrap(), 0);
    assert_eq!(
        doc.count_matches_in(
            "cat",
            SearchOptions {
                whole_word: true,
                ..options
            },
            1..8
        )
        .unwrap(),
        0
    );
    assert_eq!(doc.count_matches_in("cat", options, 1..1).unwrap(), 0);
}

#[test]
fn unicode_scope_grows_and_shrinks_with_backward_selection_and_undo() {
    let original = "🦀 a a a";
    for replacement in ["long🦀", ""] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        let selection = Selection {
            anchor: 8,
            caret: 5,
        };
        doc.select(selection).unwrap();
        assert_eq!(
            doc.replace_matches_in(
                "a",
                replacement,
                SearchOptions::default(),
                selection.range()
            )
            .unwrap(),
            2
        );
        assert_eq!(doc.text(), format!("🦀 {replacement} {replacement} a"));
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 6 + replacement.len(),
                caret: 5 + replacement.len()
            }
        );
        let after = doc.selection();
        doc.undo().unwrap();
        assert_eq!(doc.text(), original);
        assert_eq!(doc.selection(), selection);
        assert!(!doc.can_undo());
        doc.redo().unwrap();
        assert_eq!(doc.selection(), after);
    }
}

#[test]
fn invalid_scopes_and_empty_selection_never_edit_other_text() {
    let mut doc = Document::from_bytes("🦀aaa".as_bytes()).unwrap();
    let options = SearchOptions::default();
    // Reversed bounds are intentional invalid API input, not an iteration range.
    for scope in [1..4, 0..8, std::ops::Range { start: 6, end: 5 }] {
        assert_eq!(
            doc.count_matches_in("a", options, scope.clone()),
            Err(EditError::InvalidRange)
        );
        assert_eq!(
            doc.replace_matches_in("a", "b", options, scope),
            Err(EditError::InvalidRange)
        );
    }
    assert_eq!(doc.replace_matches_in("a", "b", options, 4..4).unwrap(), 0);
    assert_eq!(doc.text(), "🦀aaa");
    assert!(!doc.can_undo());
    doc.set_read_only(true);
    assert_eq!(
        doc.replace_matches_in("a", "b", options, 4..6),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "🦀aaa");
}

#[test]
fn extended_multiline_matches_cannot_cross_scope_end() {
    let mut doc = Document::from_bytes(b"a\r\na\r\na").unwrap();
    let options = SearchOptions {
        mode: SearchMode::Extended,
        ..Default::default()
    };
    assert_eq!(doc.count_matches_in(r"\r\n", options, 1..5).unwrap(), 1);
    assert_eq!(
        doc.replace_matches_in(r"\r\n", r"\t", options, 1..5)
            .unwrap(),
        1
    );
    assert_eq!(doc.text(), "a\ta\r\na");
}
