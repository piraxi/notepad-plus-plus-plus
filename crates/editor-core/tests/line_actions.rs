use npp_editor_core::{Document, Selection};

fn document(text: &str, anchor: usize, caret: usize) -> Document {
    let mut doc = Document::new();
    doc.insert(text).unwrap();
    doc.select(Selection { anchor, caret }).unwrap();
    doc
}

#[test]
fn boundary_deletion_uses_caret_and_has_direction_specific_selection_behavior() {
    let mut doc = document("a\r\n🦀xy\rz", 0, 7);
    doc.delete_to_line_boundary(true).unwrap();
    assert_eq!(doc.text(), "a\r\n🦀\rz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 7,
            caret: 7
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a\r\n🦀xy\rz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 7
        }
    );
    doc.delete_to_line_boundary(false).unwrap();
    assert_eq!(doc.text(), "a\r\nxy\rz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 3
        }
    );
    doc.undo().unwrap();
    doc.select(Selection {
        anchor: 9,
        caret: 7,
    })
    .unwrap();
    doc.delete_to_line_boundary(false).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 5,
            caret: 3
        }
    );
}

#[test]
fn boundary_deletion_does_not_join_lines_or_add_noop_history() {
    for ending in ["\r\n", "\n", "\r"] {
        let text = format!("abc{ending}def");
        let start = 3 + ending.len();
        let mut doc = document(&text, start, start);
        doc.delete_to_line_boundary(false).unwrap();
        doc.select(Selection {
            anchor: 3,
            caret: 3,
        })
        .unwrap();
        doc.delete_to_line_boundary(true).unwrap();
        assert_eq!(doc.text(), text);
        doc.undo().unwrap();
        assert_eq!(doc.text(), "");
    }
    let mut doc = document("abc\r\ndef", 0, 4);
    doc.delete_to_line_boundary(true).unwrap();
    assert_eq!(doc.text(), "abc\r\ndef");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 4
        }
    );
    doc.delete_to_line_boundary(false).unwrap();
    assert_eq!(doc.text(), "\ndef");
}

#[test]
fn boundary_deletion_respects_read_only() {
    for rightwards in [false, true] {
        let mut doc = document("abc", 0, 1);
        doc.set_read_only(true);
        assert!(doc.delete_to_line_boundary(rightwards).is_err());
        assert_eq!(doc.text(), "abc");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 0,
                caret: 1
            }
        );
    }
}

#[test]
fn delete_targets_caret_line_and_maps_selection() {
    let mut doc = document("a\r\n🦀\rb\n", 0, 7);
    doc.delete_line().unwrap();
    assert_eq!(doc.text(), "a\r\nb\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 3
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a\r\n🦀\rb\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 7
        }
    );
    doc.redo().unwrap();
    assert_eq!(doc.text(), "a\r\nb\n");
}

#[test]
fn final_line_deletion_preserves_preceding_ending_and_empty_tail_is_noop() {
    let mut doc = document("a\r\nlast", 7, 7);
    doc.delete_line().unwrap();
    assert_eq!(doc.text(), "a\r\n");
    doc.delete_line().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a\r\nlast");
    let mut doc = document("only", 2, 2);
    doc.delete_line().unwrap();
    assert_eq!(doc.text(), "");
}

#[test]
fn transpose_retains_endings_and_places_caret_at_current_line_start() {
    let mut doc = document("first\r\n🦀\rlast", 0, 11);
    doc.transpose_line().unwrap();
    assert_eq!(doc.text(), "🦀\r\nfirst\rlast");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 6,
            caret: 6
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "first\r\n🦀\rlast");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 11
        }
    );
    let mut doc = document("a\n", 2, 2);
    doc.transpose_line().unwrap();
    assert_eq!(doc.text(), "\na");
    assert_eq!(doc.selection().caret, 1);
}

#[test]
fn first_line_transpose_is_noop_and_mutations_respect_read_only() {
    let mut doc = document("a\nb", 0, 0);
    doc.transpose_line().unwrap();
    assert_eq!(doc.text(), "a\nb");
    doc.select(Selection {
        anchor: 2,
        caret: 2,
    })
    .unwrap();
    doc.set_read_only(true);
    assert!(doc.transpose_line().is_err());
    assert!(doc.delete_line().is_err());
    assert_eq!(doc.text(), "a\nb");
}
