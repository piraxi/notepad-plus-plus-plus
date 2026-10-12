use npp_editor_core::{Document, EditError, Selection, WhitespaceConversion::*};
use std::num::NonZeroUsize;

fn width() -> NonZeroUsize {
    NonZeroUsize::new(4).unwrap()
}

#[test]
fn expands_tabs_at_unicode_columns_and_preserves_mixed_endings() {
    let original = "🦀\té\t\r\na\tb\r\tend\n";
    let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
    doc.convert_whitespace(TabsToSpaces, width()).unwrap();
    assert_eq!(doc.text(), "🦀   é   \r\na   b\r    end\n");
    doc.undo().unwrap();
    assert_eq!(doc.text(), original);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.text(), "🦀   é   \r\na   b\r    end\n");
}

#[test]
fn space_compression_obeys_stop_and_single_space_rules() {
    for (original, expected) in [
        ("   x", "   x"),
        ("    x", "\tx"),
        ("abc x", "abc x"),
        ("abc  x", "abc\t x"),
        ("a  \tx", "a\tx"),
        ("a   x", "a\tx"),
        ("🦀   x", "🦀\tx"),
        ("   \tx", "\tx"),
        ("        x", "\t\tx"),
    ] {
        let mut doc = Document::from_bytes(original.as_bytes()).unwrap();
        doc.convert_whitespace(SpacesToTabsAll, width()).unwrap();
        assert_eq!(doc.text(), expected, "input {original:?}");
    }
}

#[test]
fn leading_mode_matches_upstream_existing_tab_boundary() {
    let mut doc = Document::from_bytes(b"    a   b\n\t    c\n  \t    d").unwrap();
    doc.convert_whitespace(SpacesToTabsLeading, width())
        .unwrap();
    assert_eq!(doc.text(), "\ta   b\n\t    c\n\t\td");
}

#[test]
fn partial_backward_selection_expands_but_excludes_next_line() {
    let mut doc = Document::from_bytes(b"a\tb\n\tx\n\tz").unwrap();
    let selection = Selection {
        anchor: 7,
        caret: 1,
    };
    doc.select(selection).unwrap();
    doc.convert_whitespace(TabsToSpaces, width()).unwrap();
    assert_eq!(doc.text(), "a   b\n    x\n\tz");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 0,
            caret: 12
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.selection(), selection);
    assert_eq!(doc.text(), "a\tb\n\tx\n\tz");
}

#[test]
fn no_op_readonly_and_caret_mapping() {
    let mut doc = Document::from_bytes(b"no tabs").unwrap();
    let selection = Selection {
        anchor: 5,
        caret: 2,
    };
    doc.select(selection).unwrap();
    doc.convert_whitespace(TabsToSpaces, width()).unwrap();
    assert_eq!(doc.selection(), selection);
    assert!(!doc.can_undo());
    let mut doc = Document::from_bytes(b"a\tb").unwrap();
    doc.select(Selection {
        anchor: 2,
        caret: 2,
    })
    .unwrap();
    doc.set_read_only(true);
    assert_eq!(
        doc.convert_whitespace(TabsToSpaces, width()),
        Err(EditError::ReadOnly)
    );
    assert_eq!(doc.text(), "a\tb");
    doc.set_read_only(false);
    doc.convert_whitespace(TabsToSpaces, width()).unwrap();
    assert_eq!(doc.selection().caret, 4);
}
