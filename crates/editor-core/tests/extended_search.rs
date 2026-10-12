use npp_editor_core::{Document, EditError, SearchDirection, SearchMode, SearchOptions};

fn extended() -> SearchOptions {
    SearchOptions {
        mode: SearchMode::Extended,
        match_case: true,
        ..Default::default()
    }
}

#[test]
fn extended_escape_table_and_fixed_numeric_widths() {
    for (input, expected) in [
        (r"\r\n\t\0\\", "\r\n\t\0\\"),
        (r"\b01000001\o101\d065\x41\u0041", "AAAAA"),
        (r"\d999\o777\xff", "ϧǿÿ"),
        (r"\uD83E\uDD80", "🦀"),
        (r"\x414\u00412", "A4A2"),
        (r"\\n", r"\n"),
        (r"\q\xG0\u123\b01200000\o888", r"\q\xG0\u123\b01200000\o888"),
        ("end\\", "end\\"),
    ] {
        assert_eq!(
            SearchMode::Extended.decode(input).unwrap(),
            expected,
            "{input}"
        );
        assert_eq!(SearchMode::Normal.decode(input).unwrap(), input);
    }
    for input in [r"\uD800", r"\uDC00", r"\uD800A", r"\uD800\uD800"] {
        assert_eq!(
            SearchMode::Extended.decode(input),
            Err(EditError::InvalidSearchEscape)
        );
    }
}

#[test]
fn multiline_nul_unicode_and_backward_search_keep_byte_offsets() {
    let doc = Document::from_bytes("🦀\r\na\t\0🦀\r\n".as_bytes()).unwrap();
    assert_eq!(
        doc.search_matches(r"\r\n", extended()).unwrap(),
        vec![4..6, 13..15]
    );
    assert_eq!(
        doc.search_matches(r"\0\uD83E\uDD80", extended()).unwrap(),
        vec![8..13]
    );
    assert_eq!(
        doc.search(
            r"\r\n",
            13,
            SearchOptions {
                direction: SearchDirection::Backward,
                ..extended()
            }
        )
        .unwrap(),
        Some(4..6)
    );
    assert_eq!(
        doc.search_matches(r"\r\n", SearchOptions::default())
            .unwrap(),
        vec![]
    );
}

#[test]
fn replacements_decode_once_and_undo_together() {
    let mut doc = Document::from_bytes(b"a\nb\n").unwrap();
    assert_eq!(doc.replace_matches(r"\n", r"\\n\t", extended()).unwrap(), 2);
    assert_eq!(doc.text(), "a\\n\tb\\n\t");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a\nb\n");
    assert!(!doc.can_undo());
    assert_eq!(
        doc.replace_matches(r"\n", r"\uD800", extended()),
        Err(EditError::InvalidSearchEscape)
    );
    assert_eq!(doc.text(), "a\nb\n");
    assert!(!doc.is_dirty());
    doc.replace_matches("a", r"\0", extended()).unwrap();
    assert_eq!(doc.text(), "\0\nb\n");
}
