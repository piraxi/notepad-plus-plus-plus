//! Preserve source line endings in Scintilla while mapping the legacy UI's
//! UTF-16 positions to the Rust document's UTF-8 positions.
use npp_editor_core::{Document, EditError, Selection};

pub struct Projection {
    pub text: String,
    offsets: Vec<usize>,
    boundaries: Vec<usize>,
}

impl Projection {
    pub fn new(source: &str) -> Self {
        let mut text = String::new();
        let mut offsets = vec![0];
        let mut boundaries = vec![0];
        let mut chars = source.char_indices().peekable();
        while let Some((start, ch)) = chars.next() {
            match ch {
                '\r' => {
                    text.push('\r');
                    if chars.peek().is_some_and(|(_, c)| *c == '\n') {
                        chars.next();
                        text.push('\n');
                        offsets.extend([start + 1, start + 2]);
                    } else {
                        offsets.push(start + 1);
                    }
                }
                '\n' => {
                    text.push('\n');
                    offsets.push(start + 1);
                }
                _ => {
                    text.push(ch);
                    if ch.len_utf16() == 2 {
                        offsets.push(start);
                    }
                    offsets.push(start + ch.len_utf8());
                }
            }
            boundaries.push(text.len());
        }
        Self {
            text,
            offsets,
            boundaries,
        }
    }

    pub fn byte_offset(&self, utf16: usize) -> usize {
        self.offsets[utf16.min(self.offsets.len() - 1)]
    }

    pub fn utf16_offset(&self, byte: usize) -> usize {
        self.offsets.partition_point(|&offset| offset < byte)
    }

    pub fn selection(&self, anchor: usize, caret: usize) -> Selection {
        Selection {
            anchor: self.byte_offset(anchor),
            caret: self.byte_offset(caret),
        }
    }

    #[cfg(test)]
    pub fn apply(&self, document: &mut Document, display: &str) -> Result<(), EditError> {
        self.apply_change(document, display, true)
    }

    pub fn apply_exact(&self, document: &mut Document, display: &str) -> Result<(), EditError> {
        self.apply_change(document, display, false)
    }

    fn apply_change(
        &self,
        document: &mut Document,
        display: &str,
        convert_endings: bool,
    ) -> Result<(), EditError> {
        if display == self.text {
            return Ok(());
        }
        let prefix = self
            .text
            .chars()
            .zip(display.chars())
            .take_while(|(a, b)| a == b)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        let prefix = self.boundaries[self.boundaries.partition_point(|&b| b <= prefix) - 1];
        let suffix = self.text[prefix..]
            .chars()
            .rev()
            .zip(display[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        let old_end = self.text.len() - suffix;
        let old_end = self.boundaries[self.boundaries.partition_point(|&b| b < old_end)];
        let suffix = self.text.len() - old_end;
        let start = self.byte_offset(self.text[..prefix].encode_utf16().count());
        let end = self.byte_offset(self.text[..self.text.len() - suffix].encode_utf16().count());
        let replacement = if convert_endings {
            document
                .line_ending()
                .convert(&display[prefix..display.len() - suffix])
        } else {
            display[prefix..display.len() - suffix].to_owned()
        };
        document.replace(start..end, &replacement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_preserves_untouched_mixed_endings_and_unicode() {
        let mut doc = Document::from_bytes("a\n🦀\rb\r\n".as_bytes()).unwrap();
        let projection = Projection::new(doc.text());
        assert_eq!(projection.text, "a\r\n🦀\r\nb\r\n");
        projection.apply(&mut doc, "a\r\n🦀!\r\nb\r\n").unwrap();
        assert_eq!(doc.text(), "a\n🦀!\rb\r\n");
        doc.undo().unwrap();
        assert_eq!(doc.text(), "a\n🦀\rb\r\n");
    }
    #[test]
    fn utf16_positions_map_without_splitting_characters() {
        let source = "🦀\na\r\nb";
        let projection = Projection::new(source);
        for byte in (0..=source.len()).filter(|&i| source.is_char_boundary(i)) {
            assert_eq!(projection.byte_offset(projection.utf16_offset(byte)), byte);
        }
        assert_eq!(projection.byte_offset(1), 0);
    }
    #[test]
    fn new_lines_use_document_ending() {
        let mut doc = Document::from_bytes(b"a\nb").unwrap();
        Projection::new(doc.text())
            .apply(&mut doc, "a\r\nb\r\nc")
            .unwrap();
        assert_eq!(doc.text(), "a\nb\nc");
    }
}
