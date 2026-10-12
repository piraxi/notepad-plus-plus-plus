use npp_editor_core::{Document, EditError, Selection};

#[test]
fn search_target_replacement_preserves_history_even_for_identical_text() {
    for replacement in ["🦀", "long", ""] {
        let mut doc = Document::from_bytes("a🦀z".as_bytes()).unwrap();
        let before = Selection {
            anchor: 5,
            caret: 1,
        };
        doc.select(before).unwrap();
        doc.replace_search_target(1..5, replacement).unwrap();
        let end = 1 + replacement.len();
        assert_eq!(doc.text(), format!("a{replacement}z"));
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: end,
                caret: end
            }
        );
        assert!(doc.is_dirty());
        doc.undo().unwrap();
        assert_eq!(doc.text(), "a🦀z");
        assert_eq!(doc.selection(), before);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
        doc.redo().unwrap();
        assert_eq!(doc.text(), format!("a{replacement}z"));
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: end,
                caret: end
            }
        );
        assert!(doc.is_dirty());
    }
}

#[test]
fn invalid_and_read_only_targets_leave_state_unchanged() {
    let mut doc = Document::from_bytes("a🦀z".as_bytes()).unwrap();
    assert_eq!(
        doc.replace_search_target(1..2, "x"),
        Err(EditError::InvalidRange)
    );
    doc.set_read_only(true);
    assert_eq!(
        doc.replace_search_target(1..5, "🦀"),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "a🦀z");
    assert_eq!(doc.selection(), Selection::default());
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
}
