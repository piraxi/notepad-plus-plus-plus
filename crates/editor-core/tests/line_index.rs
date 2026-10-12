use npp_editor_core::{Document, Encoding, LineEnding};

#[test]
fn single_batch_changes_preserve_index_and_selection_mapping() {
    use npp_editor_core::Selection;
    let original = "head\r\n🦀\nbody\rtail";
    for (range, replacement) in [
        (5..5, "x"),
        (4..5, ""),
        (5..6, ""),
        (6..10, "a\r\nb"),
        (0..0, "\n"),
    ] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        let selection = Selection {
            anchor: original.len(),
            caret: range.start,
        };
        doc.select(selection).unwrap();
        let _ = doc.line_starts();
        let mut expected = original.to_owned();
        expected.replace_range(range.clone(), replacement);
        doc.apply_edits(&[(range.clone(), replacement.to_owned())])
            .unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(doc.line_starts(), reference(&expected));
        assert_eq!(doc.selection().caret, range.start);
        assert_eq!(
            doc.selection().anchor,
            original.len() - range.len() + replacement.len()
        );
        doc.undo().unwrap();
        assert_eq!(doc.text(), original);
        assert_eq!(doc.selection(), selection);
        assert_eq!(doc.line_starts(), reference(original));
        doc.redo().unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(doc.line_starts(), reference(&expected));
    }
}

#[test]
fn large_single_change_with_noop_entries_retains_sparse_undo() {
    let text = "abc\r\n".repeat(100_000);
    let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
    let before = doc.line_starts().to_vec();
    doc.apply_edits(&[
        (0..1, "a".into()),
        (250_000..250_001, "z".into()),
        (499_995..499_996, "a".into()),
    ])
    .unwrap();
    assert_eq!(doc.line_starts(), before);
    assert_eq!(doc.history_text_bytes(), 1);
    doc.undo().unwrap();
    assert_eq!(doc.text(), text);
    assert_eq!(doc.line_starts(), before);
}

fn reference(text: &str) -> Vec<usize> {
    let mut result = vec![0];
    for (offset, ch) in text.char_indices() {
        if ch == '\n' || (ch == '\r' && !text[offset + 1..].starts_with('\n')) {
            result.push(offset + 1);
        }
    }
    result
}

#[test]
fn cached_index_matches_full_scan_across_every_small_unicode_edit() {
    for original in ["", "a", "a\r\nb\n文\rc", "\r\n\r\r\n", "🦀\n文", "tail\r"] {
        let boundaries: Vec<_> = (0..=original.len())
            .filter(|&i| original.is_char_boundary(i))
            .collect();
        for &start in &boundaries {
            for &end in boundaries.iter().filter(|&&end| end >= start) {
                for inserted in ["", "a", "\r", "\n", "\r\n", "🦀\n", "文\r\nX"] {
                    let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
                    assert_eq!(doc.line_starts(), reference(original));
                    let mut expected = original.to_owned();
                    expected.replace_range(start..end, inserted);
                    doc.replace(start..end, inserted).unwrap();
                    assert_eq!(
                        doc.line_starts(),
                        reference(&expected),
                        "{original:?} {start}..{end} -> {inserted:?}"
                    );
                    if expected != original {
                        doc.undo().unwrap();
                        assert_eq!(doc.line_starts(), reference(original));
                        doc.redo().unwrap();
                        assert_eq!(doc.line_starts(), reference(&expected));
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_queries_and_metadata_changes_reuse_index() {
    let mut doc = Document::from_bytes(b"a\nb\nc").unwrap();
    let pointer = doc.line_starts().as_ptr();
    assert_eq!(doc.line_starts().as_ptr(), pointer);
    doc.set_encoding(Encoding::Utf16Le).unwrap();
    assert_eq!(doc.line_starts().as_ptr(), pointer);
    doc.undo().unwrap();
    assert_eq!(doc.line_starts().as_ptr(), pointer);
}

#[test]
fn batches_conversion_and_reload_invalidate_old_index() {
    let mut doc = Document::from_bytes(b"a\r\nb\nc").unwrap();
    assert_eq!(doc.line_starts(), &[0, 3, 5]);
    doc.apply_edits(&[(0..1, "🦀".into()), (3..4, "x\ny".into())])
        .unwrap();
    assert_eq!(doc.line_starts(), reference(doc.text()));
    doc.undo().unwrap();
    assert_eq!(doc.line_starts(), &[0, 3, 5]);
    doc.convert_line_endings(LineEnding::Cr).unwrap();
    assert_eq!(doc.line_starts(), &[0, 2, 4]);
    doc.reload_from_bytes(b"new\nfile\n").unwrap();
    assert_eq!(doc.line_starts(), &[0, 4, 9]);
}
