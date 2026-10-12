use npp_editor_core::{Document, SearchOptions};
use std::{io, path::Path};

fn save(doc: &mut Document) {
    doc.save_with(Path::new("save-point.txt"), |_, _| Ok(()))
        .unwrap();
}

#[test]
fn equal_content_does_not_imply_returning_to_save_point() {
    let mut doc = Document::from_bytes(b"a").unwrap();
    doc.replace(0..1, "b").unwrap();
    doc.replace(0..1, "a").unwrap();
    assert_eq!(doc.text(), "a");
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert!(doc.is_dirty());
    doc.redo().unwrap();
    assert!(doc.is_dirty());
}

#[test]
fn identical_replacement_leaves_save_point_and_undo_returns_to_it() {
    let mut doc = Document::from_bytes(b"word word").unwrap();
    doc.replace_matches("word", "word", SearchOptions::default())
        .unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert!(doc.is_dirty());
    save(&mut doc);
    assert!(!doc.is_dirty());
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.redo().unwrap();
    assert!(!doc.is_dirty());
}

#[test]
fn abandoning_saved_redo_branch_cannot_recreate_save_point_with_equal_bytes() {
    let mut doc = Document::from_bytes(b"a").unwrap();
    doc.replace(0..1, "b").unwrap();
    save(&mut doc);
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.replace(0..1, "b").unwrap();
    assert!(doc.is_dirty());
    assert!(!doc.can_redo());
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.redo().unwrap();
    assert!(doc.is_dirty());
    save(&mut doc);
    assert!(!doc.is_dirty());
}

#[test]
fn failed_save_and_save_copy_do_not_move_save_point() {
    let mut doc = Document::from_bytes(b"word").unwrap();
    doc.replace_matches("word", "word", SearchOptions::default())
        .unwrap();
    assert!(
        doc.save_with(Path::new("failed.txt"), |_, _| Err(io::Error::other(
            "fixture"
        )))
        .is_err()
    );
    assert!(doc.is_dirty());
    doc.save_copy_with(Path::new("copy.txt"), |_, _| Ok(()))
        .unwrap();
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty());
}

#[test]
fn reload_establishes_a_new_clean_history() {
    let mut doc = Document::from_bytes(b"word").unwrap();
    doc.replace_matches("word", "word", SearchOptions::default())
        .unwrap();
    doc.reload_from_bytes(b"word").unwrap();
    assert!(!doc.is_dirty());
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
}
