use npp_editor_core::{Document, Encoding, Selection, Session, SessionFile, Workspace};

#[test]
fn captured_positions_restore_across_unicode_and_legacy_encodings() {
    for (encoding, text) in [
        (Encoding::Utf8, "a🦀\r\nb"),
        (Encoding::Utf8Bom, "a🦀\r\nb"),
        (Encoding::Utf16Le, "a🦀\r\nb"),
        (Encoding::Utf16Be, "a🦀\r\nb"),
        (Encoding::Windows1252, "aé\r\nb"),
        (Encoding::Windows1251, "aЖ\r\nb"),
        (Encoding::ShiftJis, "aあ\r\nb"),
    ] {
        let bytes = encoding.encode(text).unwrap();
        let mut original = Document::from_bytes_as(&bytes, encoding).unwrap();
        original
            .save_with(std::path::Path::new("fixture.txt"), |_, _| Ok(()))
            .unwrap();
        original
            .select(Selection {
                anchor: text.len(),
                caret: 1,
            })
            .unwrap();
        original.toggle_bookmark(1).unwrap();
        original.set_read_only(true);
        let file = SessionFile::from_document(&original).unwrap().unwrap();
        let mut restored = match file.explicit_encoding().unwrap() {
            Some(encoding) => Document::from_bytes_as(&bytes, encoding),
            None => Document::from_bytes(&bytes),
        }
        .unwrap();
        file.restore_document_state(&mut restored).unwrap();
        assert_eq!(restored.selection(), original.selection(), "{encoding:?}");
        assert_eq!(restored.bookmarked_lines().collect::<Vec<_>>(), vec![1]);
        assert_eq!(restored.encoding(), encoding);
        assert!(restored.is_user_read_only());
        assert!(!restored.is_dirty());
        assert!(!restored.can_undo());
        assert_eq!(restored.to_bytes().unwrap(), bytes);
    }
}

#[test]
fn session_state_is_additive_and_does_not_change_dirty_text_or_history() {
    let mut document = Document::from_bytes("a🦀\nb\nc".as_bytes()).unwrap();
    document.insert("x").unwrap();
    document.toggle_bookmark(0).unwrap();
    let mut file = SessionFile::new("unused".into());
    file.bookmarks = vec![1, 1, usize::MAX];
    file.attributes
        .insert("startPos".into(), "9223372036854775807".into());
    file.attributes.insert("endPos".into(), "3".into());
    file.restore_document_state(&mut document).unwrap();
    assert_eq!(
        document.selection(),
        Selection {
            anchor: document.text().len(),
            caret: 2
        }
    );
    assert_eq!(document.bookmarked_lines().collect::<Vec<_>>(), vec![0, 1]);
    assert!(document.is_dirty());
    assert!(document.undo().unwrap());
    assert_eq!(document.text(), "a🦀\nb\nc");
    assert!(!document.is_dirty());

    file.attributes.insert("startPos".into(), "-1".into());
    file.attributes.insert("endPos".into(), "invalid".into());
    file.restore_document_state(&mut document).unwrap();
    assert_eq!(document.selection(), Selection::default());
}

#[test]
fn session_opens_named_legacy_file_and_rejects_unsupported_codepages() {
    let path = std::env::temp_dir().join(format!("npp-session-core-{}.txt", std::process::id()));
    std::fs::write(&path, [0x82, 0xa0, b'x']).unwrap();
    let mut file = SessionFile::new(path.to_str().unwrap().into());
    file.attributes.insert("encoding".into(), "932".into());
    file.attributes.insert("startPos".into(), "2".into());
    file.attributes.insert("endPos".into(), "1".into());
    let document = file.open_document().unwrap();
    assert_eq!(document.path(), Some(path.as_path()));
    assert_eq!(document.text(), "あx");
    assert_eq!(
        document.selection(),
        Selection {
            anchor: 3,
            caret: 0
        }
    );
    assert!(!document.is_dirty());
    file.attributes.insert("encoding".into(), "12345".into());
    assert!(file.open_document().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), [0x82, 0xa0, b'x']);
    std::fs::remove_file(&path).unwrap();
    assert!(file.open_document().is_err());
}

#[test]
fn failed_legacy_position_mapping_leaves_existing_document_state_intact() {
    let mut document = Document::from_bytes_as(b"a", Encoding::ShiftJis).unwrap();
    document.insert("🦀").unwrap();
    document.toggle_bookmark(0).unwrap();
    let before = document.selection();
    let mut file = SessionFile::new("unused".into());
    file.attributes.insert("startPos".into(), "99".into());
    file.attributes.insert("userReadOnly".into(), "yes".into());
    assert!(file.restore_document_state(&mut document).is_err());
    assert_eq!(document.selection(), before);
    assert!(!document.is_user_read_only());
    assert_eq!(document.bookmarked_lines().collect::<Vec<_>>(), vec![0]);
    assert!(document.is_dirty());
    assert!(document.undo().unwrap());
    assert_eq!(document.text(), "a");
}

#[test]
fn workspace_capture_preserves_tab_order_and_source_selection_units() {
    use std::path::Path;
    let mut workspace = Workspace::default();
    let mut unicode = Document::from_bytes("a🦀\nb".as_bytes()).unwrap();
    unicode
        .save_with(Path::new("unicode.txt"), |_, _| Ok(()))
        .unwrap();
    unicode
        .select(Selection {
            anchor: 5,
            caret: 1,
        })
        .unwrap();
    unicode.toggle_bookmark(1).unwrap();
    let first = workspace.add(unicode);
    workspace.add(Document::new());
    let mut legacy = Document::from_bytes_as(&[0x82, 0xa0, b'x'], Encoding::ShiftJis).unwrap();
    legacy
        .save_with(Path::new("legacy.txt"), |_, _| Ok(()))
        .unwrap();
    legacy
        .select(Selection {
            anchor: 3,
            caret: 4,
        })
        .unwrap();
    legacy.set_read_only(true);
    workspace.add(legacy);
    let session = workspace.capture_session().unwrap();
    assert_eq!(session.main_view.active_index, 2);
    assert_eq!(session.main_view.files.len(), 2);
    assert_eq!(session.main_view.files[0].attributes["startPos"], "5");
    assert_eq!(session.main_view.files[0].attributes["endPos"], "1");
    assert_eq!(session.main_view.files[0].bookmarks, vec![1]);
    assert_eq!(session.main_view.files[1].attributes["encoding"], "932");
    assert_eq!(session.main_view.files[1].attributes["startPos"], "2");
    assert_eq!(session.main_view.files[1].attributes["endPos"], "3");
    assert_eq!(session.main_view.files[1].attributes["userReadOnly"], "yes");
    assert!(!workspace.get(first).unwrap().is_dirty());
    workspace.activate(first);
    workspace.move_active(true);
    workspace.move_active(true);
    let reordered = workspace.capture_session().unwrap();
    assert_eq!(reordered.main_view.files[1].filename(), Some("unicode.txt"));
}

#[test]
fn source_shaped_session_retains_views_positions_marks_folds_and_metadata() {
    let session = Session::from_xml(include_bytes!("fixtures/session.xml")).unwrap();
    assert_eq!(session.active_view, 1);
    assert_eq!(session.main_view.active_index, 1);
    assert_eq!(session.main_view.files.len(), 2);
    assert_eq!(session.sub_view.files.len(), 1);
    let file = &session.main_view.files[0];
    assert_eq!(file.filename(), Some("C:\\Work\\a&b.rs"));
    assert_eq!(file.attributes["startPos"], "90");
    assert_eq!(file.attributes["endPos"], "12");
    assert_eq!(
        file.attributes["originalFileLastModifTimestamp"],
        "4294967295"
    );
    assert_eq!(file.bookmarks, vec![0, 19]);
    assert_eq!(file.folds, vec![3]);
    assert_eq!(
        session.main_view.files[1].attributes["futureAttribute"],
        "kept"
    );
    assert_eq!(session.extra_nodes[0].name, "FileBrowser");
    let output = session.to_xml().unwrap();
    assert_eq!(Session::from_xml(&output).unwrap(), session);
}

#[test]
fn writer_escapes_paths_and_supports_empty_views() {
    let mut session = Session::default();
    let path = "C:\\日本語\\a&\"'<>🦀.txt";
    session.main_view.files.push(SessionFile::new(path.into()));
    let output = session.to_xml().unwrap();
    let xml = std::str::from_utf8(&output).unwrap();
    assert!(xml.contains("&amp;"));
    assert!(xml.contains("&quot;"));
    assert_eq!(Session::from_xml(&output).unwrap(), session);
    assert_eq!(
        Session::from_xml(b"<NotepadPlus><Session/></NotepadPlus>").unwrap(),
        Session::default()
    );
}

#[test]
fn malformed_sessions_are_rejected_before_a_caller_can_restore_files() {
    for xml in [
        "",
        "<NotepadPlus/>",
        "<Other><Session/></Other>",
        "<NotepadPlus><Session>",
        "<NotepadPlus><Session/><Session/></NotepadPlus>",
        "<NotepadPlus><Session><mainView/><mainView/></Session></NotepadPlus>",
        "<NotepadPlus><Session><mainView><File/></mainView></Session></NotepadPlus>",
        "<NotepadPlus><Session><mainView><File filename='&undefined;'/></mainView></Session></NotepadPlus>",
        "<!DOCTYPE NotepadPlus><NotepadPlus><Session/></NotepadPlus>",
    ] {
        assert!(Session::from_xml(xml.as_bytes()).is_err(), "{xml}");
    }
    assert!(Session::from_xml(&[0xff]).is_err());
}

#[test]
fn missing_negative_or_invalid_marker_lines_are_ignored_like_the_source() {
    assert_eq!(Session::from_xml(b"<NotepadPlus><Session activeView='-1'><mainView activeIndex='bad'/></Session></NotepadPlus>").unwrap(), Session::default());
    let session = Session::from_xml(b"<NotepadPlus><Session><mainView><File filename='x'><Mark/><Mark line='-1'/><Mark line='bad'/><Mark line='2'/><Fold line='-2'/><Fold line='3'/></File></mainView></Session></NotepadPlus>").unwrap();
    assert_eq!(session.main_view.files[0].bookmarks, vec![2]);
    assert_eq!(session.main_view.files[0].folds, vec![3]);
}
