use npp_editor_core::{Document, EditError, Encoding};
use std::path::Path;

#[test]
fn explicit_legacy_encodings_decode_known_bytes() {
    for (encoding, bytes, text) in [
        (Encoding::Windows1252, &[0x80, 0x20, 0xe9][..], "€ é"),
        (
            Encoding::Windows1251,
            &[0xcf, 0xf0, 0xe8, 0xe2, 0xe5, 0xf2][..],
            "Привет",
        ),
        (Encoding::ShiftJis, &[0x93, 0xfa, 0x96, 0x7b][..], "日本"),
    ] {
        let doc = Document::from_bytes_as(bytes, encoding).unwrap();
        assert_eq!(doc.text(), text);
        assert_eq!(doc.to_bytes().unwrap(), bytes);
        assert!(!doc.is_dirty());
    }
    assert!(Document::from_bytes_as(&[0x82], Encoding::ShiftJis).is_err());
    assert!(Document::from_bytes(&[0xe9]).is_err());
}

#[test]
fn conversion_is_undoable_and_unmappable_characters_fail_without_changes() {
    let mut doc = Document::from_bytes("€ café".as_bytes()).unwrap();
    doc.set_encoding(Encoding::Windows1252).unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert_eq!(doc.encoding(), Encoding::Utf8);
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert_eq!(doc.to_bytes().unwrap(), b"\x80 caf\xe9");
    doc.insert("🦀").unwrap();
    let mut writer_called = false;
    assert!(
        doc.save_with(Path::new("never-written.txt"), |_, _| {
            writer_called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!writer_called);
    assert!(doc.path().is_none());
    assert!(doc.is_dirty());
    doc.set_encoding(Encoding::Utf8).unwrap();
    assert_eq!(
        doc.set_encoding(Encoding::Windows1252),
        Err(EditError::UnrepresentableEncoding)
    );
    assert_eq!(doc.encoding(), Encoding::Utf8);
}

#[test]
fn reload_retains_explicit_encoding_and_rejects_lossy_mapping() {
    let mut doc = Document::from_bytes_as(b"caf\xe9", Encoding::Windows1252).unwrap();
    doc.reload_from_bytes(&[0x80]).unwrap();
    assert_eq!(doc.text(), "€");
    assert_eq!(doc.encoding(), Encoding::Windows1252);
    assert!(Encoding::ShiftJis.encode("¥").is_err());
}
