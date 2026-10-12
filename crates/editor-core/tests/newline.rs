use npp_editor_core::{Document, IndentationStyle, Selection};
use std::num::NonZeroUsize;

#[test]
fn newline_replaces_selection_and_indents_using_preference_in_one_transaction() {
    for ending in ["\r\n", "\n", "\r"] {
        for style in [IndentationStyle::Tabs, IndentationStyle::Spaces] {
            let text = format!("\t a🦀xx{ending}last");
            let mut doc = Document::from_bytes(text.as_bytes()).unwrap();
            let selection = Selection {
                anchor: 9,
                caret: 7,
            };
            doc.select(selection).unwrap();
            doc.insert_newline(true, NonZeroUsize::new(4).unwrap(), style)
                .unwrap();
            let indent = if style == IndentationStyle::Tabs {
                "\t "
            } else {
                "     "
            };
            assert_eq!(doc.text(), format!("\t a🦀{ending}{indent}{ending}last"));
            assert_eq!(doc.selection().caret, 7 + ending.len() + indent.len());
            doc.undo().unwrap();
            assert_eq!(doc.text(), text);
            assert_eq!(doc.selection(), selection);
        }
    }
}

#[test]
fn beginning_of_line_disabled_mode_and_existing_indent() {
    for (caret, enabled, expected, target) in [
        (0, true, "\r\n    abc", 2),
        (4, false, "    \r\nabc", 6),
        (2, true, "  \r\n  abc", 4),
    ] {
        let mut doc = Document::from_bytes(b"    abc").unwrap();
        doc.select(Selection {
            anchor: caret,
            caret,
        })
        .unwrap();
        doc.insert_newline(
            enabled,
            NonZeroUsize::new(4).unwrap(),
            IndentationStyle::Tabs,
        )
        .unwrap();
        assert_eq!(doc.text(), expected);
        assert_eq!(doc.selection().caret, target);
    }
    let mut doc = Document::new();
    doc.set_read_only(true);
    assert!(
        doc.insert_newline(true, NonZeroUsize::new(4).unwrap(), IndentationStyle::Tabs)
            .is_err()
    );
    assert_eq!(doc.text(), "");
}
