use npp_editor_core::{CloseError, Document, Workspace};
use std::path::Path;

#[test]
fn names_survive_reordering_and_reuse_only_available_numbers() {
    let mut workspace = Workspace::default();
    let a = workspace.add(Document::new());
    let b = workspace.add(Document::new());
    let c = workspace.add(Document::new());
    assert_eq!(workspace.untitled_name(a), Some("new 1"));
    assert_eq!(workspace.untitled_name(b), Some("new 2"));
    assert_eq!(workspace.untitled_name(c), Some("new 3"));
    workspace.move_active(false);
    workspace.close(a, false).unwrap();
    assert_eq!(workspace.untitled_name(b), Some("new 2"));
    assert_eq!(workspace.untitled_name(c), Some("new 3"));
    assert_eq!(workspace.untitled_name(a), None);
    let d = workspace.add(Document::new());
    assert_ne!(d, a);
    assert_eq!(workspace.untitled_name(d), Some("new 1"));
    workspace
        .get_mut(b)
        .unwrap()
        .save_with(Path::new("saved.txt"), |_, _| Ok(()))
        .unwrap();
    assert_eq!(workspace.untitled_name(b), None);
    let e = workspace.add(Document::new());
    assert_eq!(workspace.untitled_name(e), Some("new 2"));
    workspace.close(b, false).unwrap();
    assert_eq!(workspace.untitled_name(e), Some("new 2"));
}

#[test]
fn failed_save_or_close_retains_name_and_named_files_do_not_consume_numbers() {
    let mut workspace = Workspace::default();
    let mut named = Document::new();
    named
        .save_with(Path::new("named.txt"), |_, _| Ok(()))
        .unwrap();
    let named = workspace.add(named);
    assert_eq!(workspace.untitled_name(named), None);
    let first = workspace.add(Document::new());
    workspace.get_mut(first).unwrap().insert("unsaved").unwrap();
    assert_eq!(
        workspace.close(first, false).err(),
        Some(CloseError::UnsavedChanges)
    );
    assert!(
        workspace
            .get_mut(first)
            .unwrap()
            .save_with(Path::new("fail.txt"), |_, _| Err(std::io::Error::other(
                "test failure"
            )))
            .is_err()
    );
    assert_eq!(workspace.untitled_name(first), Some("new 1"));
    let second = workspace.add(Document::new());
    assert_eq!(workspace.untitled_name(second), Some("new 2"));
}
