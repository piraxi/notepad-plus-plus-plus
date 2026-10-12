use npp_editor_core::{Document, IndentationStyle, Selection};
use std::num::NonZeroUsize;

fn width() -> NonZeroUsize {
    NonZeroUsize::new(4).unwrap()
}

#[test]
fn spaces_tab_uses_visual_column_after_selection_deletion() {
    for width in [2, 4, 8] {
        let mut doc = document("a\t🦀XYZ", 6, 8);
        doc.indent(
            true,
            false,
            NonZeroUsize::new(width).unwrap(),
            IndentationStyle::Spaces,
        )
        .unwrap();
        assert_eq!(doc.text(), format!("a\t🦀{}Z", " ".repeat(width - 1)));
        assert_eq!(doc.selection().caret, 6 + width - 1);
        doc.undo().unwrap();
        assert_eq!(doc.text(), "a\t🦀XYZ");
        assert_eq!(
            doc.selection(),
            Selection {
                anchor: 6,
                caret: 8
            }
        );
        doc.redo().unwrap();
        assert_eq!(doc.selection().caret, 6 + width - 1);
    }
}

#[test]
fn spaces_indentation_rewrites_only_affected_leading_whitespace() {
    let text = "\t a\tb\r\n\r\n c";
    let mut doc = document(text, 0, text.len());
    doc.indent(true, false, width(), IndentationStyle::Spaces)
        .unwrap();
    assert_eq!(doc.text(), "         a\tb\r\n\r\n     c");
    doc.indent(false, false, width(), IndentationStyle::Spaces)
        .unwrap();
    assert_eq!(doc.text(), "     a\tb\r\n\r\n c");
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.text(), "\t a\tb\r\n\r\n c");

    let mut doc = document("\t x", 1, 1);
    doc.indent(true, false, width(), IndentationStyle::Spaces)
        .unwrap();
    assert_eq!(doc.text(), "        x");
    assert_eq!(doc.selection().caret, 8);
    doc.indent(false, true, width(), IndentationStyle::Spaces)
        .unwrap();
    assert_eq!(doc.text(), "    x");
    assert_eq!(doc.selection().caret, 4);
}
fn document(text: &str, anchor: usize, caret: usize) -> Document {
    let mut doc = Document::new();
    doc.insert(text).unwrap();
    doc.select(Selection { anchor, caret }).unwrap();
    doc
}

#[test]
fn tab_indents_leading_space_and_replaces_inline_selection() {
    let mut doc = document("  abc", 1, 1);
    doc.indent(true, false, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "\tabc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 1,
            caret: 1
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), "  abc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 1,
            caret: 1
        }
    );
    let mut doc = document("a🦀bc", 1, 5);
    doc.indent(true, false, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "a\tbc");
    assert_eq!(doc.selection().caret, 2);
}

#[test]
fn backtab_dedents_or_moves_to_previous_column_without_editing() {
    let mut doc = document("\t  text", 2, 2);
    doc.indent(false, false, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "  text");
    assert_eq!(doc.selection().caret, 2);
    let mut doc = document("a🦀bcde", 9, 9);
    doc.indent(false, false, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "a🦀bcde");
    assert_eq!(doc.selection().caret, 7);
    doc.undo().unwrap();
    assert_eq!(doc.text(), ""); // Movement did not add undo history.
}

#[test]
fn multiline_indent_excludes_end_line_skips_empty_and_retains_direction() {
    let mut doc = document("a\r\n\r\n  b\rc", 9, 1);
    let before = doc.text().to_owned();
    doc.indent(true, false, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "\ta\r\n\r\n\t  b\rc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 11,
            caret: 0
        }
    );
    doc.undo().unwrap();
    assert_eq!(doc.text(), before);
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 9,
            caret: 1
        }
    );
}

#[test]
fn menu_indent_preserves_single_line_selection_and_rejects_read_only() {
    let mut doc = document("  abc", 2, 5);
    doc.indent(true, true, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "\t  abc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 3,
            caret: 6
        }
    );
    doc.indent(false, true, width(), IndentationStyle::Tabs)
        .unwrap();
    assert_eq!(doc.text(), "  abc");
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: 2,
            caret: 5
        }
    );
    doc.set_read_only(true);
    assert!(
        doc.indent(true, true, width(), IndentationStyle::Tabs)
            .is_err()
    );
    assert_eq!(doc.text(), "  abc");
}
