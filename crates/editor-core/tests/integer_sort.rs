use npp_editor_core::{Document, LineOrder};

#[test]
fn integer_chunks_signs_leading_zeros_and_arbitrary_precision() {
    let ordered = [
        "item-100",
        "item-02",
        "item-2",
        "item-0",
        "item000",
        "item0",
        "item002",
        "item2",
        "item9",
        "item10",
        "item999999999999999999999999999999999999",
    ];
    let original = ordered.iter().rev().copied().collect::<Vec<_>>().join("\n");
    let mut doc = Document::new();
    doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
        .unwrap();
    doc.insert(&original).unwrap();
    doc.order_lines(LineOrder::IntegerAscending).unwrap();
    assert_eq!(doc.text(), ordered.join("\n"));
    doc.order_lines(LineOrder::IntegerDescending).unwrap();
    assert_eq!(doc.text(), original);
    doc.undo().unwrap();
    assert_eq!(doc.text(), ordered.join("\n"));
    doc.undo().unwrap();
    assert_eq!(doc.text(), original);
}

#[test]
fn successive_numeric_chunks_and_non_numeric_minus_signs() {
    for (left, right) in [
        ("a2b9", "a2b10"),
        ("0-", "0-0"),
        ("-a2", "a10"),
        ("a002z", "a2a"),
        ("😀2", "😀10"),
    ] {
        let mut doc = Document::new();
        doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
            .unwrap();
        doc.convert_line_endings(npp_editor_core::LineEnding::CrLf)
            .unwrap();
        doc.insert(&format!("{right}\r\n{left}")).unwrap();
        doc.order_lines(LineOrder::IntegerAscending).unwrap();
        assert_eq!(doc.text(), format!("{left}\r\n{right}"));
    }
}
