use npp_editor_core::{Document, EditError, LineOrder, Selection};

#[test]
fn decimal_prefixes_empty_keys_and_equal_values_are_stable() {
    for (separator, ascending, descending) in [
        (
            '.',
            LineOrder::DecimalDotAscending,
            LineOrder::DecimalDotDescending,
        ),
        (
            ',',
            LineOrder::DecimalCommaAscending,
            LineOrder::DecimalCommaDescending,
        ),
    ] {
        let original =
            "2.5tail\nwords\n-1.25\n2.50\n \n10\n1e9".replace('.', &separator.to_string());
        let mut doc = Document::new();
        doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
            .unwrap();
        doc.insert(&original).unwrap();
        doc.order_lines(ascending).unwrap();
        assert_eq!(
            doc.text(),
            "words\n \n-1.25\n1e9\n2.5tail\n2.50\n10".replace('.', &separator.to_string())
        );
        doc.undo().unwrap();
        assert_eq!(doc.text(), original);
        doc.order_lines(descending).unwrap();
        assert_eq!(
            doc.text(),
            "10\n2.5tail\n2.50\n1e9\n-1.25\nwords\n ".replace('.', &separator.to_string())
        );
    }
}

#[test]
fn invalid_numeric_line_does_not_partially_sort_or_change_selection() {
    let mut doc = Document::new();
    doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
        .unwrap();
    doc.insert("skip\n2\n1\n-\nend").unwrap();
    let selection = Selection {
        anchor: 5,
        caret: 11,
    };
    doc.select(selection).unwrap();
    assert_eq!(
        doc.order_lines(LineOrder::DecimalDotAscending),
        Err(EditError::InvalidNumericLine(4))
    );
    assert_eq!(doc.text(), "skip\n2\n1\n-\nend");
    assert_eq!(doc.selection(), selection);
    doc.undo().unwrap();
    assert!(doc.text().is_empty());
}

#[test]
fn representable_subnormal_values_sort_without_range_errors() {
    let tiny = format!("{:.400}", f64::from_bits(1));
    let bigger = format!("{:.400}", f64::MIN_POSITIVE / 2.0);
    let mut doc = Document::new();
    doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
        .unwrap();
    doc.insert(&format!("{bigger}\n{tiny}\n0\n-{tiny}"))
        .unwrap();
    doc.order_lines(LineOrder::DecimalDotAscending).unwrap();
    assert_eq!(doc.text(), format!("-{tiny}\n0\n{tiny}\n{bigger}"));
}

#[test]
fn overflow_and_underflow_are_rejected_before_mutation() {
    for value in [
        "9".repeat(400),
        format!("0.{}1", "0".repeat(400)),
        ".".into(),
    ] {
        let original = format!("1\n{value}");
        let mut doc = Document::new();
        doc.convert_line_endings(npp_editor_core::LineEnding::Lf)
            .unwrap();
        doc.insert(&original).unwrap();
        assert_eq!(
            doc.order_lines(LineOrder::DecimalDotAscending),
            Err(EditError::InvalidNumericLine(2))
        );
        assert_eq!(doc.text(), original);
    }
}
