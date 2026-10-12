use npp_editor_core::{CloseScope, Document, Selection, Workspace};

#[test]
fn recent_file_remembers_disk_encoding_not_discarded_conversion() {
    use npp_editor_core::Encoding;
    use std::path::Path;
    let mut doc = Document::from_bytes_as(b"caf\xe9", Encoding::Windows1252).unwrap();
    doc.save_with(Path::new("legacy.txt"), |_, _| Ok(()))
        .unwrap();
    doc.set_encoding(Encoding::Utf8).unwrap();
    let mut workspace = Workspace::default();
    let id = workspace.add(doc);
    workspace.close(id, true).unwrap();
    let recent = workspace.latest_closed().unwrap();
    assert_eq!(recent.path, Path::new("legacy.txt"));
    assert_eq!(recent.encoding, Encoding::Windows1252);
}

#[test]
fn navigation_wraps_reordering_does_not_and_history_uses_saved_paths() {
    use std::path::Path;
    let mut workspace = Workspace::default();
    assert_eq!(workspace.activate_adjacent(true), None);
    assert!(!workspace.move_active(true));
    let mut first = Document::new();
    first
        .save_with(Path::new("one.txt"), |_, _| Ok(()))
        .unwrap();
    let a = workspace.add(first);
    let b = workspace.add(Document::new());
    assert_eq!(workspace.activate_adjacent(true), Some(a));
    assert_eq!(workspace.activate_adjacent(false), Some(b));
    assert!(!workspace.move_active(true));
    assert!(workspace.move_active(false));
    assert_eq!(
        workspace.buffers().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![b, a]
    );
    assert_eq!(workspace.active(), Some(b));
    workspace.get_mut(a).unwrap().insert("discarded").unwrap();
    assert!(workspace.close(a, false).is_err());
    assert!(workspace.latest_closed_path().is_none());
    workspace.close(a, true).unwrap();
    assert_eq!(workspace.latest_closed_path(), Some(Path::new("one.txt")));
    workspace.close(b, true).unwrap();
    assert_eq!(workspace.latest_closed_path(), Some(Path::new("one.txt")));
    let mut reopened = Document::new();
    reopened
        .save_with(Path::new("one.txt"), |_, _| Ok(()))
        .unwrap();
    workspace.add(reopened);
    assert!(workspace.latest_closed_path().is_none());
}

#[test]
fn close_groups_follow_tab_order_and_dirty_state() {
    let mut workspace = Workspace::default();
    let a = workspace.add(Document::new());
    let b = workspace.add(Document::new());
    let c = workspace.add(Document::new());
    workspace.get_mut(b).unwrap().insert("unsaved").unwrap();
    workspace.activate(b);
    assert_eq!(workspace.close_targets(CloseScope::All), vec![a, b, c]);
    assert_eq!(workspace.close_targets(CloseScope::Others), vec![a, c]);
    assert_eq!(workspace.close_targets(CloseScope::Left), vec![a]);
    assert_eq!(workspace.close_targets(CloseScope::Right), vec![c]);
    assert_eq!(workspace.close_targets(CloseScope::Unchanged), vec![a, c]);
}

#[test]
fn reload_validates_before_discard_and_preserves_unicode_position() {
    let mut doc = Document::from_bytes("abc\r\n🦀xy".as_bytes()).unwrap();
    doc.select(Selection {
        anchor: 10,
        caret: 10,
    })
    .unwrap();
    doc.insert("!").unwrap();
    let previous = doc.text().to_owned();
    assert!(doc.reload_from_bytes(&[0xff]).is_err());
    assert_eq!(doc.text(), previous);
    assert!(doc.can_undo());
    doc.reload_from_bytes("longer\n文xyz".as_bytes()).unwrap();
    assert_eq!(doc.selection().caret, "longer\n文xy".len());
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
    doc.reload_from_bytes(b"x").unwrap();
    assert_eq!(doc.selection().caret, 1);
}
