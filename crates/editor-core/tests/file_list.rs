use npp_editor_core::{Document, Workspace};
use std::{ffi::OsString, path::Path};

#[test]
fn file_lists_follow_tabs_and_keep_untitled_names_without_dirty_markers() {
    let mut workspace = Workspace::default();
    assert!(workspace.file_list(false).is_empty());
    let first = workspace.add(Document::new());
    let mut saved = Document::new();
    let path = Path::new("folder").join("é😀.txt");
    saved.save_with(&path, |_, _| Ok(())).unwrap();
    saved.insert("unsaved changes").unwrap();
    saved.set_read_only(true);
    let saved_id = workspace.add(saved);
    let last = workspace.add(Document::new());
    assert_eq!(workspace.file_list(false), "new 1\r\né😀.txt\r\nnew 2\r\n");
    let mut expected = OsString::from("new 1\r\n");
    expected.push(&path);
    expected.push("\r\nnew 2\r\n");
    assert_eq!(workspace.file_list(true), expected);
    workspace.move_active(false);
    assert_eq!(workspace.file_list(false), "new 1\r\nnew 2\r\né😀.txt\r\n");
    assert_eq!(workspace.active(), Some(last));
    assert!(workspace.get(saved_id).unwrap().is_dirty());
    workspace.close(first, true).unwrap();
    assert_eq!(workspace.file_list(false), "new 2\r\né😀.txt\r\n");
}
