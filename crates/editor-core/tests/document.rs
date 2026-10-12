use npp_editor_core::{
    CloseError, Document, EditError, Encoding, LineEnding, Selection, Workspace,
};
use std::{io, path::Path};

#[test]
fn encodings_round_trip_unicode_and_mixed_endings() {
    let text = "hello\r\n世界\n🦀\rtail\0";
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf8Bom,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
    ] {
        let bytes = encoding.encode(text).unwrap();
        let document = Document::from_bytes(&bytes).unwrap();
        assert_eq!(document.text(), text);
        assert_eq!(document.encoding(), encoding);
        assert_eq!(document.to_bytes().unwrap(), bytes);
        assert!(!document.is_dirty());
    }
}

#[test]
fn invalid_input_is_rejected_without_lossy_replacement() {
    for bytes in [
        &[0xff][..],
        &[0xff, 0xfe, 0x41],
        &[0xff, 0xfe, 0, 0xd8],
        &[0xfe, 0xff, 0xdc, 0],
    ] {
        assert!(Document::from_bytes(bytes).is_err());
    }
}

#[test]
fn unicode_edits_and_selection_survive_undo_redo() {
    let mut doc = Document::from_bytes("a🦀z".as_bytes()).unwrap();
    let selection = Selection {
        anchor: 5,
        caret: 1,
    };
    doc.select(selection).unwrap();
    doc.insert("文").unwrap();
    assert_eq!(doc.text(), "a文z");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 4,
            caret: 4
        }
    );
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a🦀z");
    assert_eq!(doc.selection(), selection);
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "a文z");
    doc.undo().unwrap();
    doc.insert("different").unwrap();
    assert!(!doc.can_redo());
}

#[test]
fn invalid_ranges_do_not_mutate_history() {
    let mut doc = Document::from_bytes("🦀".as_bytes()).unwrap();
    for range in [
        1..4,
        0..5,
        std::ops::Range { start: 4, end: 0 },
        usize::MAX..usize::MAX,
    ] {
        assert_eq!(doc.replace(range, "x"), Err(EditError::InvalidRange));
    }
    assert_eq!(doc.text(), "🦀");
    assert!(!doc.can_undo());
}

#[test]
fn read_only_prevents_edits_and_history_changes() {
    let mut doc = Document::new();
    doc.insert("before").unwrap();
    doc.set_read_only(true);
    assert_eq!(doc.insert("after"), Err(EditError::ReadOnly));
    assert_eq!(
        doc.set_encoding(Encoding::Utf16Be),
        Err(EditError::ReadOnly)
    );
    assert_eq!(
        doc.convert_line_endings(LineEnding::Lf),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.undo(), Err(EditError::ReadOnly));
    assert_eq!(doc.text(), "before");
}

#[test]
fn line_endings_detect_convert_and_undo_as_one_transaction() {
    let mut doc = Document::from_bytes(b"a\r\nb\nc\rd\n").unwrap();
    assert_eq!(doc.line_ending(), LineEnding::Lf);
    assert_eq!(doc.line_starts(), vec![0, 3, 5, 7, 9]);
    doc.select(Selection {
        anchor: 9,
        caret: 3,
    })
    .unwrap();
    doc.convert_line_endings(LineEnding::CrLf).unwrap();
    assert_eq!(doc.text(), "a\r\nb\r\nc\r\nd\r\n");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 12,
            caret: 3
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.to_bytes().unwrap(), b"a\r\nb\nc\rd\n");
    assert_eq!(LineEnding::detect("a\rb\n"), LineEnding::Cr);
    assert_eq!(LineEnding::detect(""), LineEnding::CrLf);
}

#[test]
fn conversion_maps_every_unicode_boundary_to_a_valid_position() {
    let original = "🦀\r\n文\ra\n";
    for ending in [LineEnding::CrLf, LineEnding::Lf, LineEnding::Cr] {
        for position in (0..=original.len()).filter(|&p| original.is_char_boundary(p)) {
            let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
            doc.select(Selection {
                anchor: position,
                caret: position,
            })
            .unwrap();
            doc.convert_line_endings(ending).unwrap();
            assert!(doc.text().is_char_boundary(doc.selection().caret));
        }
    }
}

#[test]
fn literal_search_wraps_and_replace_all_is_one_undo_step() {
    let mut doc = Document::from_bytes("a🦀 a🦀".as_bytes()).unwrap();
    assert_eq!(doc.find_next("🦀", 0, false).unwrap(), Some(1..5));
    assert_eq!(doc.find_next("🦀", 5, false).unwrap(), Some(7..11));
    assert_eq!(doc.find_next("🦀", 11, false).unwrap(), None);
    assert_eq!(doc.find_next("a🦀", 1, true).unwrap(), Some(6..11));
    assert_eq!(doc.find_next("a🦀 a", 1, true).unwrap(), Some(0..7));
    assert_eq!(doc.find_next("", 0, true), Err(EditError::EmptySearch));
    assert_eq!(doc.find_next("a", 2, true), Err(EditError::InvalidRange));
    assert_eq!(doc.replace_all("🦀", "文").unwrap(), 2);
    assert_eq!(doc.text(), "a文 a文");
    doc.undo().unwrap();
    assert_eq!(doc.text(), "a🦀 a🦀");
    assert!(!doc.can_undo());
}

#[test]
fn failed_save_keeps_path_and_savepoint_unchanged() {
    let mut doc = Document::new();
    doc.insert("saved").unwrap();
    doc.save_with(Path::new("original.txt"), |_, bytes| {
        assert_eq!(bytes, b"saved");
        Ok(())
    })
    .unwrap();
    doc.insert(" changed").unwrap();
    let result = doc.save_with(Path::new("other.txt"), |_, _| {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "fixture"))
    });
    assert!(result.is_err());
    assert_eq!(doc.path(), Some(Path::new("original.txt")));
    assert!(doc.is_dirty());
    doc.undo().unwrap();
    assert!(!doc.is_dirty());
    doc.redo().unwrap();
    assert!(doc.is_dirty());
}

#[test]
fn encoding_changes_participate_in_savepoints_and_history() {
    let mut doc = Document::new();
    doc.set_encoding(Encoding::Utf8Bom).unwrap();
    assert!(doc.is_dirty());
    doc.save_with(Path::new("bom.txt"), |_, bytes| {
        assert_eq!(bytes, &[0xef, 0xbb, 0xbf]);
        Ok(())
    })
    .unwrap();
    assert!(!doc.is_dirty());
    doc.undo().unwrap();
    assert!(doc.is_dirty());
    doc.redo().unwrap();
    assert!(!doc.is_dirty());
}

#[test]
fn tabs_have_stable_ids_and_require_explicit_discard() {
    let mut workspace = Workspace::default();
    let a = workspace.add(Document::new());
    let b = workspace.add(Document::new());
    let c = workspace.add(Document::new());
    workspace.get_mut(b).unwrap().insert("unsaved").unwrap();
    assert!(matches!(
        workspace.close(b, false),
        Err(CloseError::UnsavedChanges)
    ));
    workspace.close(a, false).unwrap();
    assert_eq!(workspace.active(), Some(c));
    assert_eq!(workspace.get(b).unwrap().text(), "unsaved");
    workspace.activate(b);
    workspace.close(b, true).unwrap();
    assert_eq!(workspace.active(), Some(c));
    workspace.close(c, false).unwrap();
    assert_eq!(workspace.active(), None);
    assert!(!workspace.activate(a));
    assert_ne!(workspace.add(Document::new()), a);
}
