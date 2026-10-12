use npp_editor_core::{Document, EditError, Selection};

#[test]
fn selected_line_range_includes_endpoint_line_in_both_directions() {
    let mut doc = Document::new();
    doc.insert("é\r\nx\ny\rlast").unwrap();
    for (anchor, caret) in [(2, 4), (4, 2)] {
        doc.select(Selection { anchor, caret }).unwrap();
        assert_eq!(doc.selected_line_range(), 0..6);
        doc.cut_selected_lines().unwrap();
        assert_eq!(doc.text(), "y\rlast");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 0,
                caret: 0
            }
        );
        doc.undo().unwrap();
        assert_eq!(doc.text(), "é\r\nx\ny\rlast");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 6,
                caret: 0
            }
        );
    }
}

#[test]
fn final_line_empty_tail_and_read_only() {
    let mut doc = Document::new();
    doc.insert("one\nlast").unwrap();
    assert_eq!(doc.selected_line_range(), 4..8);
    doc.cut_selected_lines().unwrap();
    assert_eq!(doc.text(), "one\n");
    assert_eq!(doc.selected_line_range(), 4..4);
    doc.cut_selected_lines().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "one\nlast");
    doc.set_read_only(true);
    let selection = doc.selection();
    assert_eq!(doc.cut_selected_lines(), Err(EditError::ReadOnly));
    assert_eq!(doc.selection(), selection);
}
