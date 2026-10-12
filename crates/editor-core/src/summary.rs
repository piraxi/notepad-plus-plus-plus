use crate::{Document, EncodeError, Encoding};

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentSummary {
    pub characters: usize,
    pub words: usize,
    pub lines: usize,
    pub bytes: usize,
    pub selected_characters: usize,
    pub selected_bytes: usize,
    pub ranges: usize,
}

impl Document {
    /// Status counts include CR and LF individually, but exclude a next line
    /// whose start is exactly the selection end.
    pub fn selected_characters_and_lines(&self) -> (usize, usize) {
        let range = self.selection().range();
        let starts = self.line_starts();
        let first = starts.partition_point(|&position| position <= range.start) - 1;
        let mut last = starts.partition_point(|&position| position <= range.end) - 1;
        if first != last && starts[last] == range.end {
            last -= 1;
        }
        (self.text()[range].chars().count(), last - first + 1)
    }

    pub fn summary(&self) -> Result<DocumentSummary, EncodeError> {
        let selected = self.selection().range();
        let selection = &self.text()[selected.clone()];
        let utf8 = matches!(self.encoding(), Encoding::Utf8 | Encoding::Utf8Bom);
        let legacy = matches!(
            self.encoding(),
            Encoding::Windows1252 | Encoding::Windows1251 | Encoding::ShiftJis
        );
        // Scintilla stores Unicode documents as UTF-8 even for UTF-16 files.
        // Legacy documents use their original codepage bytes, without a BOM.
        let byte_length = |text: &str| -> Result<usize, EncodeError> {
            if legacy {
                Ok(self.encoding().encode(text)?.len())
            } else {
                Ok(text.len())
            }
        };
        let bytes = byte_length(self.text())?;
        let selected_bytes = byte_length(selection)?;
        let lines = self.line_starts().len();
        let without_endings =
            |text: &str| text.chars().filter(|c| !matches!(c, '\r' | '\n')).count();
        let characters = if utf8 {
            without_endings(self.text())
        } else {
            bytes.saturating_sub((lines - 1) * self.line_ending().as_str().len())
        };
        let selected_characters = if utf8 {
            without_endings(selection)
        } else {
            let first = self.line_starts().partition_point(|&p| p <= selected.start);
            let last = self.line_starts().partition_point(|&p| p <= selected.end);
            let count =
                selected_bytes.wrapping_sub((last - first) * self.line_ending().as_str().len());
            if legacy { count } else { count.wrapping_mul(2) }
        };
        // The source counts matches of a negated character class, not Unicode
        // words or whitespace. Underscores, NBSP and form-feed remain in words.
        let delimiters = " \t\\.,;:!?()+\r\n-*/=][{}&~\"'`|@$%<>^";
        let words = self
            .text()
            .split(|c| delimiters.contains(c))
            .filter(|s| !s.is_empty())
            .count();
        Ok(DocumentSummary {
            characters,
            words,
            lines,
            bytes,
            selected_characters,
            selected_bytes,
            ranges: usize::from(!selected.is_empty()),
        })
    }
}
