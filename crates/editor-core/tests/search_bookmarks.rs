use npp_editor_core::{Document, EditError, SearchMode, SearchOptions, Selection};

#[test]
fn multiline_matches_mark_touched_lines_but_not_a_following_empty_line() {
    let mut doc = Document::from_bytes(b"a\r\nb\nc\r\na\r\nb\n").unwrap();
    let options = SearchOptions {
        mode: SearchMode::Extended,
        ..SearchOptions::default()
    };
    assert_eq!(
        doc.bookmark_matches_in("a\\r\\nb\\n", options, 0..doc.text().len())
            .unwrap(),
        2
    );
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![0, 1, 3, 4]);
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
}

#[test]
fn folded_scoped_matches_are_additive_and_preserve_reverse_selection() {
    let mut doc = Document::from_bytes("ß\nSS\nﬃ\n".as_bytes()).unwrap();
    doc.toggle_bookmark(3).unwrap();
    let before = Selection {
        anchor: 5,
        caret: 3,
    };
    doc.select(before).unwrap();
    doc.set_read_only(true);
    assert_eq!(
        doc.bookmark_matches_in("ss", SearchOptions::default(), 3..5)
            .unwrap(),
        1
    );
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(
        doc.bookmark_matches_in("ss", SearchOptions::default(), 0..2)
            .unwrap(),
        1
    );
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![0, 1, 3]);
    assert_eq!(doc.selection(), before);
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn matches_must_be_complete_nonoverlapping_and_valid() {
    let mut doc = Document::from_bytes(b"aaa\naa").unwrap();
    assert_eq!(
        doc.bookmark_matches_in("aa", SearchOptions::default(), 0..5)
            .unwrap(),
        1
    );
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![0]);
    assert_eq!(
        doc.bookmark_matches_in("", SearchOptions::default(), 0..6),
        Err(EditError::EmptySearch)
    );
    assert_eq!(
        doc.bookmark_matches_in("aa", SearchOptions::default(), 0..7),
        Err(EditError::InvalidRange)
    );
    assert_eq!(doc.bookmarked_lines().collect::<Vec<_>>(), vec![0]);
}
