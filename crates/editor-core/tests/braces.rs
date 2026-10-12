use npp_editor_core::{Document, Selection};

#[test]
fn nesting_unicode_and_preceding_brace_priority() {
    let mut doc = Document::from_bytes("🦀([x]){}".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 4,
        caret: 4,
    })
    .unwrap();
    assert_eq!(doc.matching_brace(), Some((4, 8)));
    doc.select(Selection {
        anchor: 5,
        caret: 5,
    })
    .unwrap();
    assert_eq!(doc.matching_brace(), Some((4, 8)));
    doc.go_to_matching_brace(false).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 8,
            caret: 8
        }
    );
    doc.select(Selection {
        anchor: 9,
        caret: 9,
    })
    .unwrap();
    assert_eq!(doc.matching_brace(), Some((8, 4)));
    doc.go_to_matching_brace(true).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 9
        }
    );
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn same_brace_depth_and_unmatched_braces() {
    let mut doc = Document::from_bytes(b"(a(b)c)").unwrap();
    assert_eq!(doc.matching_brace(), Some((0, 6)));
    doc.select(Selection {
        anchor: 7,
        caret: 7,
    })
    .unwrap();
    assert_eq!(doc.matching_brace(), Some((6, 0)));
    let mut unmatched = Document::from_bytes(b"(()").unwrap();
    assert!(!unmatched.go_to_matching_brace(false).unwrap());
    assert_eq!(unmatched.selection(), Selection::default());
    assert!(Document::new().matching_brace().is_none());
    assert!(
        Document::from_bytes(b"<x>")
            .unwrap()
            .matching_brace()
            .is_none()
    );
}

#[test]
fn readonly_navigation_is_allowed() {
    let mut doc = Document::from_bytes(b"[x]").unwrap();
    doc.set_read_only(true);
    assert!(doc.go_to_matching_brace(true).unwrap());
    assert_eq!(doc.selection().range(), 0..3);
}
