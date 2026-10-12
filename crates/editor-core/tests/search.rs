use npp_editor_core::{Document, EditError, SearchDirection, SearchOptions};

#[test]
fn replace_all_maps_selection_like_delete_then_insert_and_restores_history() {
    use npp_editor_core::Selection;
    for (anchor, caret, expected_anchor, expected_caret) in [
        (0, 0, 0, 0),
        (3, 3, 2, 2),
        (4, 4, 2, 2),
        (5, 5, 6, 6),
        (7, 7, 8, 8),
        (11, 11, 13, 13),
        (2, 4, 2, 2),
        (2, 9, 5, 8),
        (9, 2, 8, 5),
        (0, 11, 0, 13),
    ] {
        let mut doc = Document::from_bytes(b"a xx b xx c").unwrap();
        let before = Selection { anchor, caret };
        let after = Selection {
            anchor: expected_anchor,
            caret: expected_caret,
        };
        doc.select(before).unwrap();
        assert_eq!(
            doc.replace_matches("xx", "QQQ", SearchOptions::default())
                .unwrap(),
            2
        );
        assert_eq!(doc.text(), "a QQQ b QQQ c");
        assert_eq!(doc.selection(), after, "original selection: {before:?}");
        doc.undo().unwrap();
        assert_eq!(doc.text(), "a xx b xx c");
        assert_eq!(doc.selection(), before);
        doc.redo().unwrap();
        assert_eq!(doc.text(), "a QQQ b QQQ c");
        assert_eq!(doc.selection(), after);
    }
}

#[test]
fn replacing_identical_text_still_clamps_a_caret_inside_the_match() {
    let mut doc = Document::from_bytes(b"word").unwrap();
    doc.select(npp_editor_core::Selection {
        anchor: 2,
        caret: 2,
    })
    .unwrap();
    assert_eq!(
        doc.replace_matches("word", "word", SearchOptions::default())
            .unwrap(),
        1
    );
    assert_eq!(doc.text(), "word");
    assert_eq!(
        doc.selection(),
        npp_editor_core::Selection {
            anchor: 0,
            caret: 0
        }
    );
    assert!(doc.can_undo());
    assert!(doc.undo().unwrap());
    assert_eq!(
        doc.selection(),
        npp_editor_core::Selection {
            anchor: 2,
            caret: 2
        }
    );
    assert_eq!(doc.text(), "word");
    assert!(!doc.can_undo());
    assert!(doc.redo().unwrap());
    assert_eq!(
        doc.selection(),
        npp_editor_core::Selection {
            anchor: 0,
            caret: 0
        }
    );
}

#[test]
fn identical_search_replacements_form_one_transaction_and_discard_redo() {
    use npp_editor_core::Selection;
    for original in ["word word", "WORD word"] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        let before = Selection {
            anchor: 9,
            caret: 0,
        };
        doc.select(before).unwrap();
        assert_eq!(
            doc.replace_matches("word", "word", SearchOptions::default())
                .unwrap(),
            2
        );
        let after = doc.selection();
        assert_eq!(doc.text(), "word word");
        assert!(doc.undo().unwrap());
        assert_eq!(doc.text(), original);
        assert_eq!(doc.selection(), before);
        assert!(!doc.can_undo());
        assert!(doc.redo().unwrap());
        assert_eq!(doc.selection(), after);
        assert!(doc.undo().unwrap());
        assert!(doc.can_redo());
        assert_eq!(
            doc.replace_matches("absent", "word", SearchOptions::default())
                .unwrap(),
            0
        );
        assert!(doc.can_redo());
        doc.replace_matches("word", "word", SearchOptions::default())
            .unwrap();
        assert!(!doc.can_redo());
    }
}

#[test]
fn identical_search_replacement_obeys_read_only_without_moving_selection() {
    let mut doc = Document::from_bytes(b"word").unwrap();
    let before = npp_editor_core::Selection {
        anchor: 2,
        caret: 2,
    };
    doc.select(before).unwrap();
    doc.set_read_only(true);
    assert_eq!(
        doc.replace_matches("word", "word", SearchOptions::default()),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.selection(), before);
    assert!(!doc.can_undo());
}

#[test]
fn full_case_folding_preserves_source_offsets_and_rejects_partial_expansions() {
    let options = SearchOptions::default();
    let mut doc = Document::from_bytes("ß SS ẞ".as_bytes()).unwrap();
    assert_eq!(
        doc.search_matches("ss", options).unwrap(),
        vec![0..2, 3..5, 6..9]
    );
    assert_eq!(
        doc.search_matches("ß", options).unwrap(),
        vec![0..2, 3..5, 6..9]
    );
    assert_eq!(doc.search_matches("s", options).unwrap(), vec![3..4, 4..5]);
    assert_eq!(doc.replace_matches("ss", "X", options).unwrap(), 3);
    assert_eq!(doc.text(), "X X X");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "ß SS ẞ");
    for (text, needle, count) in [
        ("Σ σ ς", "σ", 3),
        ("ﬃ FFI ffi", "ffi", 3),
        ("İ i\u{307}", "i\u{307}", 2),
        ("µ μ Μ", "μ", 3),
    ] {
        let doc = Document::from_bytes(text.as_bytes()).unwrap();
        assert_eq!(doc.count_matches(needle, options).unwrap(), count);
    }
    let doc = Document::from_bytes("ß".as_bytes()).unwrap();
    assert!(
        doc.search_matches(
            "ss",
            SearchOptions {
                match_case: true,
                ..options
            }
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn streaming_navigation_matches_all_ranges_at_every_unicode_boundary() {
    for text in [
        "ababa",
        "é É élan _é 🦀É",
        "a\r\na\na",
        "",
        "İiİ",
        "ß SS ẞs ﬃ FFI Σς",
    ] {
        let doc = Document::from_bytes(text.as_bytes()).unwrap();
        for needle in ["a", "aba", "é", "İ", "\n", "absent", "ss", "ß", "ffi", "σ"] {
            for match_case in [false, true] {
                for whole_word in [false, true] {
                    for wrap in [false, true] {
                        let options = SearchOptions {
                            match_case,
                            whole_word,
                            wrap,
                            ..Default::default()
                        };
                        let matches = doc.search_matches(needle, options).unwrap();
                        for start in text.char_indices().map(|(i, _)| i).chain(Some(text.len())) {
                            for direction in [SearchDirection::Forward, SearchDirection::Backward] {
                                let expected = match direction {
                                    SearchDirection::Forward => matches
                                        .iter()
                                        .find(|m| m.start >= start)
                                        .or_else(|| if wrap { matches.first() } else { None }),
                                    SearchDirection::Backward => matches
                                        .iter()
                                        .rev()
                                        .find(|m| m.end <= start)
                                        .or_else(|| if wrap { matches.last() } else { None }),
                                }
                                .cloned();
                                assert_eq!(
                                    doc.search(
                                        needle,
                                        start,
                                        SearchOptions {
                                            direction,
                                            ..options
                                        }
                                    )
                                    .unwrap(),
                                    expected
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn expanded_folds_respect_selection_scope_and_reverse_undo_selection() {
    let mut doc = Document::from_bytes("ß ss ﬃ ffi".as_bytes()).unwrap();
    let options = SearchOptions::default();
    assert_eq!(
        doc.count_matches_in("ss", options, 0..1),
        Err(EditError::InvalidRange)
    );
    assert_eq!(doc.count_matches_in("ss", options, 3..4).unwrap(), 0);
    assert_eq!(doc.count_matches_in("ss", options, 3..5).unwrap(), 1);
    assert_eq!(doc.count_matches_in("ffi", options, 6..9).unwrap(), 1);
    let selection = npp_editor_core::Selection {
        anchor: 13,
        caret: 6,
    };
    doc.select(selection).unwrap();
    assert_eq!(
        doc.replace_matches_in("ffi", "x", options, 6..13).unwrap(),
        2
    );
    assert_eq!(doc.text(), "ß ss x x");
    assert_eq!(
        doc.selection(),
        npp_editor_core::Selection {
            anchor: 8,
            caret: 7
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "ß ss ﬃ ffi");
    assert_eq!(doc.selection(), selection);
}

#[test]
fn count_is_nonoverlapping_respects_options_and_leaves_state_unchanged() {
    let mut doc = Document::from_bytes(b"aaa AAA aa_x").unwrap();
    let options = SearchOptions::default();
    assert_eq!(doc.count_matches("aa", options).unwrap(), 3);
    assert_eq!(
        doc.count_matches(
            "aa",
            SearchOptions {
                match_case: true,
                ..options
            }
        )
        .unwrap(),
        2
    );
    assert_eq!(
        doc.count_matches(
            "aa",
            SearchOptions {
                whole_word: true,
                ..options
            }
        )
        .unwrap(),
        0
    );
    assert_eq!(doc.count_matches("", options), Err(EditError::EmptySearch));
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
    assert_eq!(
        doc.count_matches("aa", options).unwrap(),
        doc.replace_matches("aa", "b", options).unwrap()
    );
}

#[test]
#[allow(clippy::single_range_in_vec_init)] // Each range is one expected match.
fn whole_word_uses_scintilla_class_transitions_and_combining_marks() {
    let options = SearchOptions {
        whole_word: true,
        match_case: true,
        ..Default::default()
    };
    for (text, needle, expected) in [
        ("e\u{301} e", "e", vec![4..5]),
        ("e\u{301} e", "e\u{301}", vec![0..3]),
        ("a+b ++ +", "+", vec![1..2, 7..8]),
        ("a+b ++ +", "++", vec![4..6]),
        ("a b", " ", vec![]),
        (" a b ", "a b", vec![1..4]),
        (" a b ", " a", vec![]),
        ("a_ a‿a", "a", vec![3..4, 7..8]),
        ("a\u{200d}a", "a", vec![0..1, 4..5]),
        ("a🦀a", "🦀", vec![1..5]),
        ("a\u{2028}a", "a", vec![0..1, 4..5]),
    ] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        assert_eq!(
            doc.search_matches(needle, options).unwrap(),
            expected,
            "{text:?}/{needle:?}"
        );
        assert_eq!(doc.count_matches(needle, options).unwrap(), expected.len());
        assert_eq!(
            doc.replace_matches(needle, "X", options).unwrap(),
            expected.len()
        );
        if !expected.is_empty() {
            doc.undo().unwrap();
            assert_eq!(doc.text(), text);
        }
    }
}

#[test]
fn unicode_matches_keep_original_byte_offsets() {
    let doc = Document::from_bytes("é É élan _é 🦀É".as_bytes()).unwrap();
    let options = SearchOptions {
        whole_word: true,
        ..Default::default()
    };
    assert_eq!(
        doc.search_matches("é", options).unwrap(),
        vec![0..2, 3..5, 20..22]
    );
    assert_eq!(
        doc.search_matches(
            "é",
            SearchOptions {
                match_case: true,
                ..options
            }
        )
        .unwrap(),
        vec![0..2]
    );
}

#[test]
fn directions_wrap_and_overlaps() {
    let doc = Document::from_bytes(b"ababa").unwrap();
    let options = SearchOptions::default();
    assert_eq!(doc.search("aba", 1, options).unwrap(), Some(2..5));
    assert_eq!(doc.search("aba", 5, options).unwrap(), Some(0..3));
    let backward = SearchOptions {
        direction: SearchDirection::Backward,
        ..options
    };
    assert_eq!(doc.search("aba", 3, backward).unwrap(), Some(0..3));
    assert_eq!(doc.search("aba", 0, backward).unwrap(), Some(2..5));
    assert_eq!(
        doc.search(
            "aba",
            0,
            SearchOptions {
                wrap: false,
                ..backward
            }
        )
        .unwrap(),
        None
    );
    assert_eq!(doc.search("", 0, options), Err(EditError::EmptySearch));
}

#[test]
fn replacement_is_nonoverlapping_literal_and_undoable() {
    let mut doc = Document::from_bytes(b"aaa AAA").unwrap();
    assert_eq!(
        doc.replace_matches("aa", "$1aa", SearchOptions::default())
            .unwrap(),
        2
    );
    assert_eq!(doc.text(), "$1aaa $1aaA");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "aaa AAA");
    assert!(!doc.can_undo());
    doc.set_read_only(true);
    assert_eq!(
        doc.replace_matches("aa", "b", SearchOptions::default()),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "aaa AAA");
}
