use npp_editor_core::{Document, Selection};

fn document(text: &str, selection: Selection) -> Document {
    let mut doc = Document::new();
    doc.insert(text).unwrap();
    doc.select(selection).unwrap();
    doc
}

#[test]
fn joins_mixed_endings_preserving_tabs_and_indentation() {
    let text = "a \r\n  b\t\r\n\n🦀\rnext";
    let selection = Selection {
        anchor: text.len(),
        caret: 1,
    };
    let mut doc = document(text, selection);
    doc.join_lines().unwrap();
    assert_eq!(doc.text(), "a   b\t 🦀 next");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: doc.text().len()
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), text);
    assert_eq!(doc.selection(), selection);
    doc.redo().unwrap();
    assert_eq!(doc.text(), "a   b\t 🦀 next");
}

#[test]
fn excludes_unselected_end_line_and_keeps_last_ending() {
    let mut doc = document(
        "a\r\nb\nc",
        Selection {
            anchor: 1,
            caret: 5,
        },
    );
    doc.join_lines().unwrap();
    assert_eq!(doc.text(), "a b\nc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 3
        }
    );
    let mut doc = document(
        "\n\n x",
        Selection {
            anchor: 0,
            caret: 4,
        },
    );
    doc.join_lines().unwrap();
    assert_eq!(doc.text(), "  x");
}

#[test]
fn caret_single_line_and_read_only_behaviors() {
    for selection in [
        Selection::default(),
        Selection {
            anchor: 0,
            caret: 2,
        },
    ] {
        let mut doc = document("a\nb", selection);
        doc.join_lines().unwrap();
        assert_eq!(doc.text(), "a\nb");
        assert_eq!(doc.selection(), selection);
        doc.undo().unwrap();
        assert_eq!(doc.text(), "");
    }
    let mut doc = document(
        "a\nb",
        Selection {
            anchor: 0,
            caret: 3,
        },
    );
    doc.set_read_only(true);
    assert!(doc.join_lines().is_err());
    assert_eq!(doc.text(), "a\nb");
}
