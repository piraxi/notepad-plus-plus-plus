use npp_editor_core::{Document, Selection};

#[test]
fn platform_transform_restores_original_reversed_selection_on_undo() {
    let mut doc = Document::from_bytes("éABC".as_bytes()).unwrap();
    let original = Selection {
        anchor: 5,
        caret: 0,
    };
    doc.select(original).unwrap();
    doc.transform_selection(|_| "Éabc".to_owned()).unwrap();
    assert_eq!(doc.text(), "Éabc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 5
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "éABC");
    assert_eq!(doc.selection(), original);
}
