use npp_editor_core::{Document, EditError, Selection, SelectionGesture};

fn caret(doc: &mut Document, position: usize) {
    doc.select(Selection {
        anchor: position,
        caret: position,
    })
    .unwrap();
}

#[test]
fn toggling_selects_backward_without_creating_history_or_editing_read_only_text() {
    let mut doc = Document::from_bytes("a🦀bc".as_bytes()).unwrap();
    let gesture = SelectionGesture::default();
    doc.track_selection_gesture(gesture.clone());
    doc.set_read_only(true);
    caret(&mut doc, 6);
    gesture.toggle(&mut doc).unwrap();
    assert!(gesture.is_started());
    caret(&mut doc, 1);
    gesture.toggle(&mut doc).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 6,
            caret: 1
        }
    );
    assert!(!gesture.is_started());
    assert!(!doc.can_undo());
    assert!(!doc.is_dirty());
}

#[test]
fn anchor_tracks_single_and_batch_edits_and_history_replay() {
    for batch in [false, true] {
        let mut doc = Document::from_bytes(b"0123456789").unwrap();
        let gesture = SelectionGesture::default();
        doc.track_selection_gesture(gesture.clone());
        caret(&mut doc, 8);
        gesture.toggle(&mut doc).unwrap();
        if batch {
            doc.apply_edits(&[(0..0, "xx".into()), (1..2, "BBB".into())])
                .unwrap();
        } else {
            doc.replace(0..0, "xxxx").unwrap();
        }
        doc.undo().unwrap();
        doc.redo().unwrap();
        caret(&mut doc, 0);
        gesture.toggle(&mut doc).unwrap();
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 12,
                caret: 0
            }
        );
    }
}

#[test]
fn source_excludes_byte_immediately_before_anchor_from_adjustment() {
    let mut doc = Document::from_bytes(b"012345").unwrap();
    let gesture = SelectionGesture::default();
    doc.track_selection_gesture(gesture.clone());
    caret(&mut doc, 4);
    gesture.toggle(&mut doc).unwrap();
    doc.replace(3..3, "xx").unwrap();
    caret(&mut doc, 0);
    gesture.toggle(&mut doc).unwrap();
    assert_eq!(doc.selection().anchor, 4);
}

#[test]
fn failed_edits_do_not_move_anchor_and_view_state_can_cross_documents() {
    let mut first = Document::from_bytes(b"0123456789").unwrap();
    let gesture = SelectionGesture::default();
    first.track_selection_gesture(gesture.clone());
    caret(&mut first, 8);
    gesture.toggle(&mut first).unwrap();
    first.set_read_only(true);
    assert_eq!(first.replace(0..1, "xx"), Err(EditError::ReadOnly));
    assert_eq!(
        first.apply_edits(&[(0..1, "xx".into()), (2..3, "yy".into())]),
        Err(EditError::ReadOnly)
    );
    first.set_read_only(false);
    caret(&mut first, 0);
    gesture.toggle(&mut first).unwrap();
    assert_eq!(first.selection().anchor, 8);
    caret(&mut first, 8);
    gesture.toggle(&mut first).unwrap();
    let mut second = Document::from_bytes("🦀".as_bytes()).unwrap();
    gesture.toggle(&mut second).unwrap();
    assert_eq!(
        second.selection(),
        Selection {
            anchor: 4,
            caret: 0
        }
    );
}
