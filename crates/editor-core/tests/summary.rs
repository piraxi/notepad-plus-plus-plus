use npp_editor_core::{Document, DocumentSummary, Encoding, Selection};

#[test]
fn utf8_summary_counts_scalars_and_source_word_delimiters() {
    let mut doc = Document::from_bytes("é🦀\r\na_b\nc\u{a0}d\r".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 8,
        caret: 0,
    })
    .unwrap();
    assert_eq!(
        doc.summary().unwrap(),
        DocumentSummary {
            characters: 8,
            words: 3,
            lines: 4,
            bytes: 17,
            selected_characters: 2,
            selected_bytes: 8,
            ranges: 1
        }
    );
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
}

#[test]
fn empty_and_bom_documents_use_editor_length() {
    let doc = Document::new();
    let summary = doc.summary().unwrap();
    assert_eq!(
        (
            summary.characters,
            summary.words,
            summary.lines,
            summary.bytes,
            summary.ranges
        ),
        (0, 0, 1, 0, 0)
    );
    let doc = Document::from_bytes(b"\xef\xbb\xbfx\n").unwrap();
    assert_eq!(doc.summary().unwrap().bytes, 2);
}

#[test]
fn source_non_utf8_paths_use_codepage_bytes_or_internal_utf8_bytes() {
    for (encoding, characters, bytes, selected_chars, selected_bytes) in [
        (Encoding::Windows1252, 1, 3, 1, 1),
        (Encoding::Utf16Le, 2, 4, 4, 2),
    ] {
        let mut doc =
            Document::from_bytes_as(&encoding.encode("é\r\n").unwrap(), encoding).unwrap();
        doc.select(Selection {
            anchor: 0,
            caret: 2,
        })
        .unwrap();
        let summary = doc.summary().unwrap();
        assert_eq!(
            (
                summary.characters,
                summary.bytes,
                summary.selected_characters,
                summary.selected_bytes
            ),
            (characters, bytes, selected_chars, selected_bytes)
        );
    }
}

#[test]
fn words_follow_literal_punctuation_set_not_unicode_whitespace() {
    let doc = Document::from_bytes(b"one_two #tag a\x0cb x\\y a+b 1.2").unwrap();
    assert_eq!(doc.summary().unwrap().words, 9);
}
