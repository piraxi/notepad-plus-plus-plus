use npp_editor_core::{Document, Encoding};

#[test]
fn known_endian_bytes_round_trip_without_inserting_a_bom() {
    for (encoding, bytes) in [
        (
            Encoding::Utf16LeNoBom,
            &[0x41, 0, 0x3e, 0xd8, 0x80, 0xdd, 0x0a, 0][..],
        ),
        (
            Encoding::Utf16BeNoBom,
            &[0, 0x41, 0xd8, 0x3e, 0xdd, 0x80, 0, 0x0a][..],
        ),
    ] {
        let doc = Document::from_bytes_as(bytes, encoding).unwrap();
        assert_eq!(doc.text(), "A🦀\n");
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(!doc.is_dirty());
        assert_eq!(encoding.encode("").unwrap(), b"");
        assert_eq!(encoding.decode_as(b"").unwrap(), "");
        // Explicit no-BOM interpretation preserves a leading U+FEFF as text.
        let text = "\u{feff}A";
        assert_eq!(
            encoding.decode_as(&encoding.encode(text).unwrap()).unwrap(),
            text
        );
    }
}

#[test]
fn malformed_utf16_is_rejected_without_replacing_live_document() {
    for (encoding, invalid) in [
        (
            Encoding::Utf16LeNoBom,
            vec![
                vec![0x41],
                vec![0, 0xd8],
                vec![0, 0xdc],
                vec![0, 0xd8, 0x41, 0],
            ],
        ),
        (
            Encoding::Utf16BeNoBom,
            vec![
                vec![0x41],
                vec![0xd8, 0],
                vec![0xdc, 0],
                vec![0xd8, 0, 0, 0x41],
            ],
        ),
    ] {
        let mut doc = Document::from_bytes_as(&encoding.encode("old").unwrap(), encoding).unwrap();
        doc.insert("changed").unwrap();
        let original = doc.text().to_owned();
        for bytes in invalid {
            assert!(encoding.decode_as(&bytes).is_err());
            assert!(doc.reload_from_bytes(&bytes).is_err());
            assert_eq!(doc.text(), original);
            assert_eq!(doc.encoding(), encoding);
            assert!(doc.can_undo());
            assert!(doc.is_dirty());
        }
    }
}

#[test]
fn conversion_history_save_and_reload_retain_byte_order() {
    for encoding in [Encoding::Utf16LeNoBom, Encoding::Utf16BeNoBom] {
        let mut doc = Document::from_bytes("日本🦀".as_bytes()).unwrap();
        doc.set_encoding(encoding).unwrap();
        assert!(doc.is_dirty());
        doc.undo().unwrap();
        assert_eq!(doc.encoding(), Encoding::Utf8);
        assert!(!doc.is_dirty());
        doc.redo().unwrap();
        let expected = encoding.encode("日本🦀").unwrap();
        doc.save_with(std::path::Path::new("sample.txt"), |_, bytes| {
            assert_eq!(bytes, expected);
            Ok(())
        })
        .unwrap();
        assert_eq!(doc.saved_encoding(), encoding);
        assert!(!doc.disk_content_changed(&expected));
        doc.reload_from_bytes(&encoding.encode("new🦀").unwrap())
            .unwrap();
        assert_eq!(doc.text(), "new🦀");
        assert_eq!(doc.encoding(), encoding);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
    }
}
