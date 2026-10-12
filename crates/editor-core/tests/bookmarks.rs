use npp_editor_core::{Document, EditError, Selection};

fn lines(doc: &Document) -> Vec<usize> {
    doc.bookmarked_lines().collect()
}

#[test]
fn clear_selected_bookmarks_matches_end_minus_one_line_boundaries() {
    for (scope, expected) in [
        (0..3, vec![1, 2, 3]),
        (1..5, vec![2, 3]),
        (3..3, vec![0, 1, 2, 3]),
        (4..4, vec![0, 2, 3]),
        (0..0, vec![1, 2, 3]),
        (7..7, vec![0, 1, 2, 3]),
    ] {
        let mut doc = Document::from_bytes(b"a\r\nb\nc\n").unwrap();
        doc.invert_bookmarks();
        doc.set_read_only(true);
        doc.clear_bookmarks_in(scope).unwrap();
        assert_eq!(lines(&doc), expected);
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
    }
}

#[test]
fn failed_reload_retains_bookmarks_successful_reload_clears_them() {
    let mut doc = Document::from_bytes(b"one\ntwo\n").unwrap();
    doc.toggle_bookmark(1).unwrap();
    assert!(doc.reload_from_bytes(&[0xff]).is_err());
    assert_eq!(lines(&doc), vec![1]);
    assert_eq!(doc.text(), "one\ntwo\n");
    doc.reload_from_bytes(b"one\ntwo\n").unwrap();
    assert!(lines(&doc).is_empty());
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
}

#[test]
fn cut_collects_changed_line_endings_after_a_crlf_join() {
    let mut doc = Document::from_bytes(b"\ra\n\n").unwrap();
    doc.toggle_bookmark(0).unwrap();
    doc.toggle_bookmark(1).unwrap();
    assert_eq!(doc.bookmarked_text(), "\ra\n");
    assert_eq!(doc.cut_bookmarked_lines().unwrap(), "\r\na\n");
    assert_eq!(doc.text(), "");
    assert!(lines(&doc).is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.text(), "\ra\n\n");
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "");
}

#[test]
fn marked_line_deletion_matches_source_text_markers_and_cut_payload() {
    fn decode(hex: &str) -> String {
        if hex == "-" {
            return String::new();
        }
        String::from_utf8(
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let fixture = include_str!("fixtures/bookmark_delete.tsv");
    assert_eq!(fixture.lines().count(), 11_192);
    for (case, row) in fixture.lines().enumerate() {
        let fields: Vec<_> = row.split('\t').collect();
        let original = decode(fields[0]);
        let mask: usize = fields[1].parse().unwrap();
        let marked = fields[2] == "1";
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        for line in 0..doc.line_starts().len() {
            if mask & (1 << line) != 0 {
                doc.toggle_bookmark(line).unwrap();
            }
        }
        if marked {
            assert_eq!(
                doc.cut_bookmarked_lines().unwrap(),
                decode(fields[4]),
                "cut case {case}: {original:?} mask {mask}"
            );
        } else {
            doc.delete_bookmarked_lines(false).unwrap();
        }
        assert_eq!(
            doc.text(),
            decode(fields[3]),
            "case {case}: {original:?} mask {mask} marked {marked}"
        );
        let expected: Vec<usize> = fields[5..].iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(
            lines(&doc),
            expected,
            "markers case {case}: {original:?} mask {mask} marked {marked}"
        );
        if doc.can_undo() {
            doc.undo().unwrap();
            assert_eq!(doc.text(), original, "undo case {case}");
            assert!(!doc.can_undo());
            assert!(!doc.is_dirty());
            doc.redo().unwrap();
            assert_eq!(doc.text(), decode(fields[3]), "redo case {case}");
        }
    }
}

#[test]
fn marked_line_paste_matches_source_text_and_markers() {
    fn decode(hex: &str) -> String {
        if hex == "-" {
            return String::new();
        }
        String::from_utf8(
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let fixture = include_str!("fixtures/bookmark_paste.tsv");
    assert_eq!(fixture.lines().count(), 15_938);
    for (case, row) in fixture.lines().enumerate() {
        let fields: Vec<_> = row.split('\t').collect();
        let original = decode(fields[0]);
        let replacement = decode(fields[1]);
        let mask: usize = fields[2].parse().unwrap();
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        for line in 0..doc.line_starts().len() {
            if mask & (1 << line) != 0 {
                doc.toggle_bookmark(line).unwrap();
            }
        }
        doc.paste_to_bookmarked_lines(&replacement).unwrap();
        assert_eq!(
            doc.text(),
            decode(fields[3]),
            "case {case}: {original:?}, {replacement:?}, mask {mask}"
        );
        let expected: Vec<usize> = fields[4..].iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(
            lines(&doc),
            expected,
            "case {case}: {original:?}, {replacement:?}, mask {mask}"
        );
        if doc.can_undo() {
            doc.undo().unwrap();
            assert_eq!(doc.text(), original, "undo case {case}");
            assert!(!doc.can_undo(), "group case {case}");
            assert!(!doc.is_dirty(), "save point case {case}");
            doc.redo().unwrap();
            assert_eq!(doc.text(), decode(fields[3]), "redo case {case}");
        }
    }
}

#[test]
fn paste_visits_a_bookmark_moved_up_by_a_temporary_crlf_join() {
    let mut doc = Document::from_bytes(b"\ra\n").unwrap();
    doc.toggle_bookmark(1).unwrap();
    let before = Selection {
        anchor: 3,
        caret: 1,
    };
    doc.select(before).unwrap();
    doc.paste_to_bookmarked_lines("a").unwrap();
    assert_eq!(doc.text(), "a\ra\n");
    assert_eq!(lines(&doc), vec![0]);
    for _ in 0..3 {
        assert!(doc.undo().unwrap());
        assert_eq!(doc.text(), "\ra\n");
        assert_eq!(doc.selection(), before);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
        assert!(doc.redo().unwrap());
        assert_eq!(doc.text(), "a\ra\n");
        assert!(doc.is_dirty());
    }
}

#[test]
fn paste_marked_line_bodies_preserves_terminators_and_groups_undo() {
    let original = "one\r\ntwo\nlast";
    let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
    doc.toggle_bookmark(0).unwrap();
    doc.toggle_bookmark(2).unwrap();
    let before = Selection {
        anchor: original.len(),
        caret: 1,
    };
    doc.select(before).unwrap();
    doc.paste_to_bookmarked_lines("X\nY").unwrap();
    assert_eq!(doc.text(), "X\nY\r\ntwo\nX\nY");
    assert_eq!(lines(&doc), vec![1, 4]);
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 9,
            caret: 3
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), original);
    assert_eq!(doc.selection(), before);
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "X\nY\r\ntwo\nX\nY");
}

#[test]
fn paste_handles_identical_empty_and_read_only_line_bodies() {
    let mut doc = Document::from_bytes(b"a\na\n").unwrap();
    doc.toggle_bookmark(0).unwrap();
    doc.toggle_bookmark(1).unwrap();
    doc.paste_to_bookmarked_lines("a").unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty());
    doc.paste_to_bookmarked_lines("").unwrap();
    assert_eq!(doc.text(), "\n\n");
    doc.undo().unwrap();
    doc.clear_bookmarks();
    doc.toggle_bookmark(2).unwrap();
    doc.paste_to_bookmarked_lines("🦀").unwrap();
    assert_eq!(doc.text(), "a\na\n🦀");
    doc.set_read_only(true);
    assert_eq!(doc.paste_to_bookmarked_lines("x"), Err(EditError::ReadOnly));
    assert_eq!(doc.text(), "a\na\n🦀");
}

#[test]
fn marked_line_copy_preserves_mixed_endings_and_unterminated_tail() {
    let mut doc = Document::from_bytes("a\r\n🦀\nb\rtail".as_bytes()).unwrap();
    assert_eq!(doc.bookmarked_text(), "");
    for line in [0, 1, 3] {
        doc.toggle_bookmark(line).unwrap();
    }
    doc.set_read_only(true);
    assert_eq!(doc.bookmarked_text(), "a\r\n🦀\ntail");
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn remove_marked_or_unmarked_lines_is_one_undo_transaction() {
    let original = "a\r\n🦀\nb\rtail";
    for (marked, expected) in [(true, "a\r\nb\r"), (false, "🦀\ntail")] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        for line in [1, 3] {
            doc.toggle_bookmark(line).unwrap();
        }
        let selection = Selection {
            anchor: original.len(),
            caret: 3,
        };
        doc.select(selection).unwrap();
        doc.delete_bookmarked_lines(marked).unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(lines(&doc), if marked { vec![] } else { vec![0, 1] });
        assert!(doc.is_dirty());
        doc.undo().unwrap();
        assert_eq!(doc.text(), original);
        assert_eq!(doc.selection(), selection);
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
        if marked {
            assert!(lines(&doc).is_empty());
        }
        doc.redo().unwrap();
        assert_eq!(doc.text(), expected);
    }
}

#[test]
fn removing_empty_final_bookmark_clears_it_without_text_history() {
    let mut doc = Document::from_bytes(b"a\n").unwrap();
    doc.toggle_bookmark(1).unwrap();
    doc.set_read_only(true);
    assert_eq!(doc.delete_bookmarked_lines(true), Err(EditError::ReadOnly));
    assert_eq!(lines(&doc), vec![1]);
    doc.set_read_only(false);
    doc.delete_bookmarked_lines(true).unwrap();
    assert_eq!(doc.text(), "a\n");
    assert!(lines(&doc).is_empty());
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
    doc.delete_bookmarked_lines(false).unwrap();
    assert_eq!(doc.text(), "");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a\n");
}

#[test]
fn line_movement_matches_supplied_scintilla_oracle() {
    fn decode(hex: &str) -> String {
        if hex == "-" {
            return String::new();
        }
        String::from_utf8(
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect(),
        )
        .unwrap()
    }
    let fixture = include_str!("fixtures/bookmarks.tsv");
    assert_eq!(fixture.lines().count(), 19_590);
    for (case, row) in fixture.lines().enumerate() {
        let fields: Vec<_> = row.split('\t').collect();
        let original = decode(fields[0]);
        let range = fields[1].parse::<usize>().unwrap()..fields[2].parse::<usize>().unwrap();
        let replacement = decode(fields[3]);
        for (marked, expected) in fields[4..].iter().enumerate() {
            let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
            doc.toggle_bookmark(marked).unwrap();
            doc.replace_search_target(range.clone(), &replacement)
                .unwrap();
            let expected: isize = expected.parse().unwrap();
            let expected = if expected < 0 {
                vec![]
            } else {
                vec![expected as usize]
            };
            assert_eq!(
                lines(&doc),
                expected,
                "case {case}: {original:?} {range:?} -> {replacement:?}, marked {marked}"
            );
        }
    }
}

#[test]
fn inverse_marks_includes_empty_lines_without_changing_history_or_selection() {
    for text in ["", "a\r\n", "a\nb\nc"] {
        let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
        doc.set_read_only(true);
        doc.invert_bookmarks();
        assert_eq!(
            lines(&doc),
            (0..doc.line_starts().len()).collect::<Vec<_>>()
        );
        doc.invert_bookmarks();
        assert!(lines(&doc).is_empty());
        doc.toggle_bookmark(0).unwrap();
        doc.invert_bookmarks();
        assert_eq!(
            lines(&doc),
            (1..doc.line_starts().len()).collect::<Vec<_>>()
        );
        assert_eq!(doc.selection(), Selection::default());
        assert!(!doc.can_undo());
        assert!(!doc.is_dirty());
    }
}

#[test]
fn navigation_wraps_collapses_and_works_read_only_without_history() {
    let mut doc = Document::from_bytes(b"a\r\nb\nc\r").unwrap();
    doc.set_read_only(true);
    assert!(!doc.goto_bookmark(true).unwrap());
    doc.toggle_bookmark(1).unwrap();
    doc.toggle_bookmark(3).unwrap();
    assert!(doc.goto_bookmark(true).unwrap());
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 3,
            caret: 3
        }
    );
    doc.goto_bookmark(false).unwrap();
    assert_eq!(doc.selection().caret, 7);
    doc.goto_bookmark(true).unwrap();
    assert_eq!(doc.selection().caret, 3);
    doc.toggle_bookmark(1).unwrap();
    assert_eq!(lines(&doc), vec![3]);
    assert_eq!(doc.toggle_bookmark(4), Err(EditError::InvalidRange));
    doc.clear_bookmarks();
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn inserted_lines_shift_markers_and_undo_moves_them_back() {
    for (position, expected) in [(0, vec![1, 2]), (1, vec![0, 2])] {
        let mut doc = Document::from_bytes(b"a\nb").unwrap();
        doc.toggle_bookmark(0).unwrap();
        doc.toggle_bookmark(1).unwrap();
        doc.replace(position..position, "\n").unwrap();
        assert_eq!(lines(&doc), expected);
        doc.undo().unwrap();
        assert_eq!(lines(&doc), vec![0, 1]);
        doc.redo().unwrap();
        assert_eq!(lines(&doc), expected);
    }
}

#[test]
fn deleted_lines_merge_markers_and_history_does_not_restore_a_snapshot() {
    let mut doc = Document::from_bytes(b"a\nb\nc").unwrap();
    doc.toggle_bookmark(2).unwrap();
    doc.replace(2..4, "").unwrap();
    assert_eq!(lines(&doc), vec![1]);
    doc.clear_bookmarks();
    doc.undo().unwrap();
    assert!(lines(&doc).is_empty());
    doc.toggle_bookmark(1).unwrap();
    doc.replace(0..doc.text().len(), "new\ntext").unwrap();
    assert!(lines(&doc).is_empty());
}

#[test]
fn splitting_and_joining_crlf_updates_line_markers() {
    let mut doc = Document::from_bytes(b"a\r\nb").unwrap();
    doc.toggle_bookmark(1).unwrap();
    doc.replace(2..2, "X").unwrap();
    assert_eq!(lines(&doc), vec![2]);
    doc.undo().unwrap();
    assert_eq!(lines(&doc), vec![1]);
    doc.replace(2..3, "").unwrap();
    assert_eq!(lines(&doc), vec![1]);
    doc.undo().unwrap();
    assert_eq!(lines(&doc), vec![1]);
}

#[test]
fn batch_edits_and_replay_use_sequential_line_coordinates() {
    let mut doc = Document::from_bytes(b"a\nb\nc").unwrap();
    doc.toggle_bookmark(2).unwrap();
    doc.apply_edits(&[(0..0, "x\n".into()), (2..2, "y\n".into())])
        .unwrap();
    assert_eq!(lines(&doc), vec![4]);
    doc.undo().unwrap();
    assert_eq!(lines(&doc), vec![2]);
    doc.redo().unwrap();
    assert_eq!(lines(&doc), vec![4]);
}
