use crate::{Document, EditError, Selection};
use std::{collections::BTreeSet, ops::Range};

#[derive(Default)]
pub(crate) struct Bookmarks(pub BTreeSet<usize>);

impl Bookmarks {
    fn insert_line(&mut self, line: usize) {
        self.0 = self.0.iter().map(|&n| n + usize::from(n >= line)).collect();
    }
    fn remove_line(&mut self, line: usize) {
        self.0 = self
            .0
            .iter()
            .map(|&n| if n >= line { n.saturating_sub(1) } else { n })
            .collect();
    }
    // Port of CellBuffer line insertion/deletion and LineMarkers' merge rule.
    // Markers are transient: undo edits move them; history does not restore them.
    pub fn replace(
        &mut self,
        text: &str,
        range: Range<usize>,
        replacement: &str,
        line_position: usize,
    ) {
        let position = range.start;
        let bytes = text.as_bytes();
        let before = position.checked_sub(1).map(|p| bytes[p]);
        let after = bytes.get(range.end).copied();
        if !range.is_empty() {
            if range == (0..text.len()) {
                self.0.clear();
            } else {
                let mut line = line_position + 1;
                let mut ignore_lf = before == Some(b'\r') && bytes[position] == b'\n';
                if ignore_lf {
                    line += 1;
                }
                for i in range.clone() {
                    match bytes[i] {
                        b'\r' if bytes.get(i + 1) != Some(&b'\n') => self.remove_line(line),
                        b'\n' if ignore_lf => ignore_lf = false,
                        b'\n' => self.remove_line(line),
                        _ => (),
                    }
                }
                if before == Some(b'\r') && bytes.get(range.end) == Some(&b'\n') {
                    self.remove_line(line - 1);
                }
            }
        }
        if !replacement.is_empty() {
            // Only the CR immediately before the target can change the prefix's
            // line count when deletion joins it to (or separates it from) LF.
            let mut insertion_line = line_position;
            if before == Some(b'\r') {
                insertion_line += usize::from(after != Some(b'\n'));
                insertion_line -= usize::from(bytes.get(position) != Some(&b'\n'));
            }
            let mut line = insertion_line + 1;
            let at_start = position == 0
                || before == Some(b'\n')
                || (before == Some(b'\r') && after != Some(b'\n'));
            if before == Some(b'\r') && after == Some(b'\n') {
                self.insert_line(line);
                line += 1;
            }
            let mut previous = before;
            for byte in replacement.bytes() {
                if byte == b'\r' || (byte == b'\n' && previous != Some(b'\r')) {
                    self.insert_line(line - usize::from(at_start));
                    line += 1;
                }
                previous = Some(byte);
            }
            if after == Some(b'\n') && replacement.ends_with('\r') {
                self.remove_line(line - 1);
            }
        }
    }
}

pub(crate) fn line_at(text: &str, position: usize) -> usize {
    text.bytes()
        .enumerate()
        .take(position)
        .filter(|&(i, b)| b == b'\n' || (b == b'\r' && text.as_bytes().get(i + 1) != Some(&b'\n')))
        .count()
}

impl Document {
    pub fn bookmarked_lines(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.bookmarks.0.iter().copied()
    }
    pub fn toggle_bookmark(&mut self, line: usize) -> Result<(), EditError> {
        if line >= self.line_starts().len() {
            return Err(EditError::InvalidRange);
        }
        if !self.bookmarks.0.remove(&line) {
            self.bookmarks.0.insert(line);
        }
        Ok(())
    }
    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.0.clear();
    }
    /// FindReplaceDlg::clearMarks clears lines from selection minimum through
    /// maximum minus one. Empty selections inside a line clear that line;
    /// empty selections at a later line start do not clear the preceding line.
    pub fn clear_bookmarks_in(&mut self, scope: Range<usize>) -> Result<(), EditError> {
        self.validate(&scope)?;
        let starts = self.line_starts();
        let first = starts.partition_point(|&p| p <= scope.start) - 1;
        let last = starts.partition_point(|&p| p <= scope.end.saturating_sub(1)) - 1;
        self.bookmarks.0.retain(|&line| line < first || line > last);
        Ok(())
    }
    /// Concatenate marked physical lines in document order, retaining their
    /// original terminators and adding nothing to an unterminated final line.
    pub fn bookmarked_text(&self) -> String {
        let starts = self.line_starts();
        let mut text = String::new();
        for line in self.bookmarked_lines() {
            let end = starts.get(line + 1).copied().unwrap_or(self.text().len());
            text.push_str(&self.text()[starts[line]..end]);
        }
        text
    }

    /// Remove marked (or unmarked) complete lines in one undo transaction.
    /// Source deleteMarkedline clears a deleted line's bookmark even when that
    /// line is the empty final line and therefore has no text to remove.
    pub fn delete_bookmarked_lines(&mut self, marked: bool) -> Result<(), EditError> {
        self.remove_bookmarked_lines(marked, false).map(|_| ())
    }

    pub fn cut_bookmarked_lines(&mut self) -> Result<String, EditError> {
        self.remove_bookmarked_lines(true, true)
    }

    fn remove_bookmarked_lines(
        &mut self,
        marked: bool,
        capture: bool,
    ) -> Result<String, EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let count = self.line_starts().len();
        let mut removed = Vec::new();
        self.group_edits(|doc| {
            for line in (0..count).rev() {
                if doc.bookmarks.0.contains(&line) != marked {
                    continue;
                }
                let starts = doc.line_starts();
                let Some(&start) = starts.get(line) else {
                    continue;
                };
                let end = starts.get(line + 1).copied().unwrap_or(doc.text().len());
                if capture {
                    removed.push(doc.text()[start..end].to_owned());
                }
                doc.bookmarks.0.remove(&line);
                doc.apply_edits(&[(start..end, String::new())])
                    .expect("valid marked line deletion");
            }
        });
        Ok(removed.into_iter().rev().collect())
    }
    pub fn paste_to_bookmarked_lines(&mut self, text: &str) -> Result<(), EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let count = self.line_starts().len();
        self.group_edits(|doc| {
            for line in (0..count).rev() {
                if !doc.bookmarks.0.contains(&line) {
                    continue;
                }
                let starts = doc.line_starts();
                let start = starts[line];
                let end = starts.get(line + 1).copied().unwrap_or(doc.text().len());
                let body = doc.text()[start..end].trim_end_matches(['\r', '\n']);
                let range = start..start + body.len();
                let mut selection = doc.selection();
                selection.replace_target(range.clone(), text.len());
                // Ranges come from current physical lines; read-only was checked
                // before the group, so these operations cannot fail validation.
                doc.apply_search_replacements(&[(range, text.to_owned())])
                    .expect("valid marked line target");
                doc.select(selection).expect("mapped target selection");
            }
        });
        Ok(())
    }

    /// Toggle every physical line, including the final empty line.
    pub fn invert_bookmarks(&mut self) {
        self.bookmarks.0 = (0..self.line_starts().len())
            .filter(|line| !self.bookmarks.0.contains(line))
            .collect();
    }
    pub fn goto_bookmark(&mut self, forward: bool) -> Result<bool, EditError> {
        let current = self
            .line_starts()
            .partition_point(|&p| p <= self.selection().caret)
            - 1;
        let line = if forward {
            self.bookmarks
                .0
                .range(current + 1..)
                .next()
                .or_else(|| self.bookmarks.0.first())
        } else {
            self.bookmarks
                .0
                .range(..current)
                .next_back()
                .or_else(|| self.bookmarks.0.last())
        }
        .copied();
        let Some(line) = line else {
            return Ok(false);
        };
        let caret = self.line_starts()[line];
        self.select(Selection {
            anchor: caret,
            caret,
        })?;
        Ok(true)
    }
}
