use crate::{DecodeError, EncodeError, Encoding, LineEnding};
use std::{
    cell::OnceCell,
    error::Error,
    fmt,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub caret: usize,
}

impl Selection {
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }
    pub(crate) fn replace_target(&mut self, range: Range<usize>, inserted: usize) {
        for position in [&mut self.anchor, &mut self.caret] {
            if *position > range.start {
                *position = range.start + position.saturating_sub(range.end);
            }
        }
        let anchor_start = self.anchor < self.caret;
        let caret_start = self.caret < self.anchor;
        for (position, move_equal) in [
            (&mut self.anchor, anchor_start),
            (&mut self.caret, caret_start),
        ] {
            if *position > range.start || (*position == range.start && move_equal) {
                *position += inserted;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    InvalidRange,
    ReadOnly,
    EmptySearch,
    InvalidSearchEscape,
    InvalidRegex(String),
    UnrepresentableEncoding,
    InvalidNumericLine(usize),
    InvalidSortKey,
}
impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::InvalidRegex(error) = self {
            return write!(f, "Regular expression: {error}");
        }
        if let Self::InvalidNumericLine(line) = self {
            return write!(f, "Unable to perform numeric sorting due to line {line}.");
        }
        f.write_str(match self {
            Self::InvalidRange => "position is out of bounds or splits a UTF-8 character",
            Self::InvalidNumericLine(_) => unreachable!(),
            Self::InvalidRegex(_) => unreachable!(),
            Self::InvalidSortKey => "Could not create a locale sort key",
            Self::ReadOnly => "document is read-only",
            Self::EmptySearch => "search text must not be empty",
            Self::InvalidSearchEscape => "extended search contains an unpaired UTF-16 surrogate",
            Self::UnrepresentableEncoding => {
                "selected encoding cannot represent the document without loss"
            }
        })
    }
}
impl Error for EditError {}

#[derive(Debug)]
struct State {
    text: String,
    selection: Selection,
    encoding: Encoding,
    line_ending: LineEnding,
}

#[derive(Debug)]
struct Change {
    range: Range<usize>,
    text: String,
}

#[derive(Debug)]
struct HistoryEntry {
    steps: Vec<HistoryEntry>,
    changes: Vec<Change>,
    selection: Selection,
    encoding: Encoding,
    line_ending: LineEnding,
    text_revision: Arc<()>,
}

/// Owns text and edit history independently of window handles and rendering.
pub struct Document {
    pub(crate) bookmarks: crate::bookmarks::Bookmarks,
    state: State,
    text_revision: Arc<()>,
    saved_revision: Arc<()>,
    saved_encoding: Encoding,
    saved_digest: Option<blake3::Hash>,
    path: Option<PathBuf>,
    read_only: bool,
    file_read_only: bool,
    disk_deleted: bool,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    line_index: OnceCell<Vec<usize>>,
    selection_gesture: Option<crate::SelectionGesture>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self::new_with_line_ending(LineEnding::CrLf)
    }

    pub fn new_with_line_ending(ending: LineEnding) -> Self {
        Self::new_with_format(ending, Encoding::Utf8)
    }

    pub fn new_with_format(ending: LineEnding, encoding: Encoding) -> Self {
        let text_revision = Arc::new(());
        Self {
            bookmarks: crate::bookmarks::Bookmarks::default(),
            state: State {
                text: String::new(),
                selection: Selection::default(),
                encoding,
                line_ending: ending,
            },
            saved_revision: text_revision.clone(),
            text_revision,
            saved_encoding: encoding,
            saved_digest: None,
            path: None,
            read_only: false,
            file_read_only: false,
            disk_deleted: false,
            undo: Vec::new(),
            redo: Vec::new(),
            line_index: OnceCell::new(),
            selection_gesture: None,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (encoding, text) = Encoding::decode(bytes)?;
        Ok(Self::decoded(encoding, text, bytes))
    }

    pub fn from_bytes_as(bytes: &[u8], encoding: Encoding) -> Result<Self, DecodeError> {
        Ok(Self::decoded(encoding, encoding.decode_as(bytes)?, bytes))
    }

    fn decoded(encoding: Encoding, text: String, bytes: &[u8]) -> Self {
        Self {
            state: State {
                line_ending: LineEnding::detect(&text),
                text,
                selection: Selection::default(),
                encoding,
            },
            saved_encoding: encoding,
            saved_digest: Some(blake3::hash(bytes)),
            ..Self::new()
        }
    }

    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::open_as(path, None)
    }

    pub fn open_as(path: impl AsRef<Path>, encoding: Option<Encoding>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let mut document = match encoding {
            Some(encoding) => Self::from_bytes_as(&bytes, encoding),
            None => Encoding::detect_file(&bytes)
                .and_then(|(encoding, _)| Self::from_bytes_as(&bytes, encoding)),
        }
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        document.path = Some(path.to_owned());
        document.file_read_only = std::fs::metadata(path)?.permissions().readonly();
        Ok(document)
    }

    /// Identity of the current text/history state, for validating view caches.
    /// Compare tokens with `Arc::ptr_eq`, not value equality. Undo restores the
    /// original identity; selection-only changes do not invalidate it.
    pub fn text_revision_token(&self) -> Arc<()> {
        self.text_revision.clone()
    }

    pub fn text(&self) -> &str {
        &self.state.text
    }

    /// Replace content from a successful reload, clearing history and marking
    /// it saved. Caller owns discard confirmation. Decode errors leave all state
    /// unchanged. Preserve selection by line and Unicode scalar column.
    pub fn reload_from_bytes(&mut self, bytes: &[u8]) -> Result<(), DecodeError> {
        let mut replacement = match self.encoding() {
            encoding if encoding.requires_explicit_decode() => {
                Self::from_bytes_as(bytes, self.encoding())?
            }
            _ => Self::from_bytes(bytes)?,
        };
        let old_starts = self.line_starts();
        let new_starts = replacement.line_starts();
        let map = |position: usize| {
            let line = old_starts.partition_point(|&p| p <= position) - 1;
            let column = self.text()[old_starts[line]..position].chars().count();
            let line = line.min(new_starts.len() - 1);
            let start = new_starts[line];
            let end = new_starts
                .get(line + 1)
                .copied()
                .unwrap_or(replacement.text().len());
            let body = replacement.text()[start..end].trim_end_matches(['\r', '\n']);
            start
                + body
                    .char_indices()
                    .nth(column)
                    .map(|(offset, _)| offset)
                    .unwrap_or(body.len())
        };
        replacement.state.selection = Selection {
            anchor: map(self.selection().anchor),
            caret: map(self.selection().caret),
        };
        replacement.path = self.path.clone();
        replacement.read_only = self.read_only;
        replacement.file_read_only = self.file_read_only;
        *self = replacement;
        Ok(())
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Change a saved file's identity only after the filesystem rename succeeds.
    /// Text, savepoint, protection and history remain attached to the buffer.
    pub fn rename_with(
        &mut self,
        destination: &Path,
        rename: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let original = self
            .path
            .as_deref()
            .ok_or_else(|| std::io::Error::other("Save the document before renaming it"))?;
        rename(original, destination)?;
        self.path = Some(destination.to_owned());
        Ok(())
    }
    pub fn selection(&self) -> Selection {
        self.state.selection
    }
    pub fn encoding(&self) -> Encoding {
        self.state.encoding
    }
    /// Encoding of the last successfully loaded or saved bytes, which can differ
    /// from an unsaved conversion in the editor.
    pub fn saved_encoding(&self) -> Encoding {
        self.saved_encoding
    }
    /// Compare disk bytes against the last successful load/save. Hash original
    /// bytes rather than re-encoding text, preserving legacy byte aliases and BOMs.
    pub fn disk_content_changed(&self, bytes: &[u8]) -> bool {
        self.saved_digest
            .is_some_and(|digest| digest != blake3::hash(bytes))
    }
    pub fn line_ending(&self) -> LineEnding {
        self.state.line_ending
    }
    pub fn saved_fingerprint(&self) -> Option<crate::DiskFingerprint> {
        self.saved_digest.map(crate::DiskFingerprint)
    }
    pub fn is_read_only(&self) -> bool {
        self.read_only || self.file_read_only
    }
    pub fn is_user_read_only(&self) -> bool {
        self.read_only
    }
    pub fn is_file_read_only(&self) -> bool {
        self.file_read_only
    }
    /// Update the cached filesystem flag without altering user editing protection.
    pub fn set_file_read_only(&mut self, value: bool) {
        self.file_read_only = value;
    }
    /// Set user editing protection; this never changes filesystem permissions.
    pub fn set_read_only(&mut self, value: bool) {
        self.read_only = value;
    }
    pub fn is_dirty(&self) -> bool {
        self.disk_deleted
            || !Arc::ptr_eq(&self.text_revision, &self.saved_revision)
            || self.state.encoding != self.saved_encoding
    }
    /// The buffer may be the only remaining copy. Keep save/close protection
    /// until an explicit reload or successful save, independently of undo.
    pub fn mark_disk_deleted(&mut self) {
        if self.path.is_some() {
            self.disk_deleted = true;
            self.file_read_only = false;
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        self.state.encoding.encode(self.text())
    }

    pub(crate) fn validate(&self, range: &Range<usize>) -> Result<(), EditError> {
        if range.start > range.end
            || !self.text().is_char_boundary(range.start)
            || !self.text().is_char_boundary(range.end)
        {
            Err(EditError::InvalidRange)
        } else {
            Ok(())
        }
    }
    fn checkpoint(&mut self, changes: Vec<Change>) -> Result<(), EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let changes_text = !changes.is_empty();
        self.undo.push(HistoryEntry {
            steps: Vec::new(),
            changes,
            selection: self.selection(),
            encoding: self.encoding(),
            line_ending: self.line_ending(),
            text_revision: self.text_revision.clone(),
        });
        if changes_text {
            // A distinct identity prevents a new branch of equal text from
            // accidentally returning to an abandoned undo save point.
            self.text_revision = Arc::new(());
        }
        self.redo.clear();
        Ok(())
    }

    pub fn select(&mut self, selection: Selection) -> Result<(), EditError> {
        self.validate(&selection.range())?;
        self.state.selection = selection;
        Ok(())
    }

    /// A replace operation is one undo transaction, including its selection.
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Result<(), EditError> {
        self.validate(&range)?;
        if &self.text()[range.clone()] == text {
            return Ok(());
        }
        let caret = range.start + text.len();
        self.checkpoint(vec![Change {
            range: range.start..caret,
            text: self.text()[range.clone()].to_owned(),
        }])?;
        self.replace_text(range, text);
        self.state.selection = Selection {
            anchor: caret,
            caret,
        };
        Ok(())
    }

    pub fn insert(&mut self, text: &str) -> Result<(), EditError> {
        self.replace(self.selection().range(), text)
    }

    /// Attach the selection gesture owned by the editing view. It is transient
    /// view state, so history entries and saved files do not own a copy of it.
    pub fn track_selection_gesture(&mut self, gesture: crate::SelectionGesture) {
        self.selection_gesture = Some(gesture);
    }

    /// Apply ordered, non-overlapping replacements in original-document byte
    /// coordinates. Validate the entire transaction before changing any state.
    /// Selection endpoints inside replaced text move to the replacement end.
    pub fn apply_edits(&mut self, edits: &[(Range<usize>, String)]) -> Result<(), EditError> {
        self.apply_edit_transaction(edits, false)
    }

    /// Search replacements are delete/insert operations even when the resulting
    /// text is identical. Keep those operations in history and notify gestures.
    pub(crate) fn apply_search_replacements(
        &mut self,
        edits: &[(Range<usize>, String)],
    ) -> Result<(), EditError> {
        self.apply_edit_transaction(edits, true)
    }

    fn apply_edit_transaction(
        &mut self,
        edits: &[(Range<usize>, String)],
        retain_identical: bool,
    ) -> Result<(), EditError> {
        let mut previous_end = 0;
        for (range, _) in edits {
            self.validate(range)?;
            if range.start < previous_end {
                return Err(EditError::InvalidRange);
            }
            previous_end = range.end;
        }
        let edits: Vec<_> = edits
            .iter()
            .filter(|(range, text)| {
                (retain_identical && (!range.is_empty() || !text.is_empty()))
                    || &self.text()[range.clone()] != text
            })
            .collect();
        if edits.is_empty() {
            return Ok(());
        }
        let map = |position: usize| {
            let (mut old, mut new) = (0, 0);
            for (range, text) in &edits {
                if position <= range.start {
                    return new + position - old;
                }
                new += range.start - old + text.len();
                if position <= range.end {
                    return new;
                }
                old = range.end;
            }
            new + position - old
        };
        let selection = Selection {
            anchor: map(self.selection().anchor),
            caret: map(self.selection().caret),
        };
        if let [(range, replacement)] = edits.as_slice() {
            // Most keyboard commands produce one effective change. Preserve
            // the existing String allocation and update a populated line index
            // locally, while retaining batch endpoint mapping and validation.
            self.checkpoint(vec![Change {
                range: range.start..range.start + replacement.len(),
                text: self.text()[range.clone()].to_owned(),
            }])?;
            self.replace_text(range.clone(), replacement);
            self.state.selection = selection;
            return Ok(());
        }
        let mut output = String::with_capacity(self.text().len());
        let mut cursor = 0;
        let mut inverse = Vec::with_capacity(edits.len());
        let mut notifications = Vec::with_capacity(edits.len());
        for (range, replacement) in edits {
            output.push_str(&self.text()[cursor..range.start]);
            notifications.push((output.len(), range.len(), replacement.len()));
            inverse.push(Change {
                range: output.len()..output.len() + replacement.len(),
                text: self.text()[range.clone()].to_owned(),
            });
            output.push_str(replacement);
            cursor = range.end;
        }
        output.push_str(&self.text()[cursor..]);
        self.checkpoint(inverse)?;
        self.update_bookmarks(notifications.iter().scan(
            (0, 0),
            |(old_end, new_end), &(start, removed, inserted)| {
                let original_start = *old_end + start - *new_end;
                *old_end = original_start + removed;
                *new_end = start + inserted;
                Some((original_start..*old_end, &output[start..*new_end]))
            },
        ));
        if let Some(gesture) = &self.selection_gesture {
            for (position, removed, inserted) in notifications {
                gesture.replaced(position, removed, inserted);
            }
        }
        self.state.text = output;
        self.line_index.take();
        self.state.selection = selection;
        Ok(())
    }

    pub(crate) fn group_edits(&mut self, edit: impl FnOnce(&mut Self)) {
        let start = self.undo.len();
        edit(self);
        if self.undo.len() <= start + 1 {
            return;
        }
        let steps: Vec<_> = self.undo.drain(start..).collect();
        let first = &steps[0];
        self.undo.push(HistoryEntry {
            selection: first.selection,
            encoding: first.encoding,
            line_ending: first.line_ending,
            text_revision: first.text_revision.clone(),
            changes: Vec::new(),
            steps,
        });
    }

    pub fn undo(&mut self) -> Result<bool, EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let Some(previous) = self.undo.pop() else {
            return Ok(false);
        };
        let inverse = self.replay(previous);
        self.redo.push(inverse);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool, EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let Some(next) = self.redo.pop() else {
            return Ok(false);
        };
        let inverse = self.replay(next);
        self.undo.push(inverse);
        Ok(true)
    }

    pub fn set_encoding(&mut self, encoding: Encoding) -> Result<(), EditError> {
        if self.encoding() != encoding {
            encoding
                .encode(self.text())
                .map_err(|_| EditError::UnrepresentableEncoding)?;
            self.checkpoint(Vec::new())?;
            self.state.encoding = encoding;
        }
        Ok(())
    }

    pub fn convert_line_endings(&mut self, ending: LineEnding) -> Result<(), EditError> {
        let text = ending.convert(self.text());
        if text == self.text() && ending == self.line_ending() {
            return Ok(());
        }
        // Map both ends of a backwards or forwards selection through the conversion.
        let selection = self.selection();
        let anchor = ending.convert(&self.text()[..selection.anchor]).len();
        let caret = ending.convert(&self.text()[..selection.caret]).len();
        let mut edits = Vec::new();
        let mut chars = self.text().char_indices().peekable();
        while let Some((start, ch)) = chars.next() {
            let end = match ch {
                '\r' if chars.peek().is_some_and(|(_, ch)| *ch == '\n') => {
                    chars.next();
                    start + 2
                }
                '\r' | '\n' => start + 1,
                _ => continue,
            };
            if &self.text()[start..end] != ending.as_str() {
                edits.push((start..end, ending.as_str().to_owned()));
            }
        }
        if edits.is_empty() {
            self.checkpoint(Vec::new())?;
        } else {
            self.apply_edits(&edits)?;
        }
        self.state.line_ending = ending;
        self.state.selection = Selection { anchor, caret };
        Ok(())
    }

    /// Stored text payload only, excluding Vec/String headers and capacity.
    /// Useful for checking that history grows with edits rather than file size.
    pub fn history_text_bytes(&self) -> usize {
        fn size(entry: &HistoryEntry) -> usize {
            entry
                .changes
                .iter()
                .map(|change| change.text.len())
                .sum::<usize>()
                + entry.steps.iter().map(size).sum::<usize>()
        }
        self.undo.iter().chain(&self.redo).map(size).sum()
    }

    fn replay(&mut self, entry: HistoryEntry) -> HistoryEntry {
        let mut inverse = HistoryEntry {
            steps: Vec::with_capacity(entry.steps.len()),
            changes: Vec::with_capacity(entry.changes.len()),
            selection: self.selection(),
            encoding: self.encoding(),
            line_ending: self.line_ending(),
            text_revision: self.text_revision.clone(),
        };
        if !entry.steps.is_empty() {
            for step in entry.steps.into_iter().rev() {
                inverse.steps.push(self.replay(step));
            }
        } else if entry.changes.len() == 1 {
            let change = &entry.changes[0];
            inverse.changes.push(Change {
                range: change.range.start..change.range.start + change.text.len(),
                text: self.text()[change.range.clone()].to_owned(),
            });
            self.replace_text(change.range.clone(), &change.text);
        } else if !entry.changes.is_empty() {
            self.update_bookmarks(
                entry
                    .changes
                    .iter()
                    .map(|change| (change.range.clone(), change.text.as_str())),
            );
            let mut output = String::with_capacity(self.text().len());
            let mut cursor = 0;
            for change in entry.changes {
                output.push_str(&self.text()[cursor..change.range.start]);
                if let Some(gesture) = &self.selection_gesture {
                    gesture.replaced(output.len(), change.range.len(), change.text.len());
                }
                inverse.changes.push(Change {
                    range: output.len()..output.len() + change.text.len(),
                    text: self.text()[change.range.clone()].to_owned(),
                });
                output.push_str(&change.text);
                cursor = change.range.end;
            }
            output.push_str(&self.text()[cursor..]);
            self.state.text = output;
            self.line_index.take();
        }
        self.state.selection = entry.selection;
        self.state.encoding = entry.encoding;
        self.state.line_ending = entry.line_ending;
        self.text_revision = entry.text_revision;
        inverse
    }

    pub fn find_next(
        &self,
        needle: &str,
        start: usize,
        wrap: bool,
    ) -> Result<Option<Range<usize>>, EditError> {
        if needle.is_empty() {
            return Err(EditError::EmptySearch);
        }
        self.validate(&(start..start))?;
        let found = self.text()[start..]
            .find(needle)
            .map(|offset| start + offset)
            .or_else(|| {
                if wrap {
                    self.text().find(needle).filter(|offset| *offset < start)
                } else {
                    None
                }
            });
        Ok(found.map(|offset| offset..offset + needle.len()))
    }

    pub fn replace_all(&mut self, needle: &str, replacement: &str) -> Result<usize, EditError> {
        if needle.is_empty() {
            return Err(EditError::EmptySearch);
        }
        let edits: Vec<_> = self
            .text()
            .match_indices(needle)
            .map(|(offset, text)| (offset..offset + text.len(), replacement.to_owned()))
            .collect();
        let count = edits.len();
        if count > 0 && needle != replacement {
            self.apply_edits(&edits)?;
            let end = self.text().len();
            self.select(Selection {
                anchor: end,
                caret: end,
            })?;
        }
        Ok(count)
    }

    /// Navigate to a one-based line, clamping out-of-range requests to the document.
    pub fn go_to_line(&mut self, one_based_line: i64) {
        let line = usize::try_from(one_based_line.saturating_sub(1).max(0)).unwrap_or(usize::MAX);
        let position = self
            .line_starts()
            .get(line)
            .copied()
            .unwrap_or(self.text().len());
        self.select(Selection {
            anchor: position,
            caret: position,
        })
        .expect("line boundary");
    }

    /// Navigate by UTF-8 byte offset, rounding forward out of characters and CRLF.
    /// Navigation remains available in read-only documents and does not edit history.
    pub fn go_to_offset(&mut self, offset: i64) {
        let mut position = usize::try_from(offset.max(0))
            .unwrap_or(usize::MAX)
            .min(self.text().len());
        while !self.text().is_char_boundary(position) {
            position += 1;
        }
        if position > 0
            && self.text().as_bytes().get(position) == Some(&b'\n')
            && self.text().as_bytes()[position - 1] == b'\r'
        {
            position += 1;
        }
        self.select(Selection {
            anchor: position,
            caret: position,
        })
        .expect("character boundary");
    }

    /// A line's UTF-8 byte range, including its line ending when present.
    pub fn line_range(&self, line: usize) -> Option<Range<usize>> {
        let starts = self.line_starts();
        Some(*starts.get(line)?..starts.get(line + 1).copied().unwrap_or(self.text().len()))
    }

    /// Zero-based line and logical column. Tabs advance to the next stop;
    /// Unicode scalars each count as one column, matching SCI_GETCOLUMN.
    pub fn line_column(
        &self,
        position: usize,
        tab_width: std::num::NonZeroUsize,
    ) -> Result<(usize, usize), EditError> {
        self.validate(&(position..position))?;
        let starts = self.line_starts();
        let line = starts.partition_point(|&start| start <= position) - 1;
        let mut column = 0;
        for ch in self.text()[starts[line]..position].chars() {
            match ch {
                '\r' | '\n' => break,
                '\t' => column += tab_width.get() - column % tab_width.get(),
                _ => column += 1,
            }
        }
        Ok((line, column))
    }

    /// Zero-based line starts, recognizing CRLF, LF, and CR without rewriting text.
    pub fn line_starts(&self) -> &[usize] {
        self.line_index
            .get_or_init(|| Self::scan_lines(self.text()))
    }

    fn scan_lines(text: &str) -> Vec<usize> {
        let mut starts = vec![0];
        let mut iter = text.bytes().enumerate().peekable();
        while let Some((offset, byte)) = iter.next() {
            match byte {
                b'\r' if iter.peek().is_some_and(|(_, b)| *b == b'\n') => {
                    iter.next();
                    starts.push(offset + 2);
                }
                b'\r' | b'\n' => starts.push(offset + 1),
                _ => (),
            }
        }
        starts
    }

    fn update_bookmarks<'a>(&mut self, edits: impl Iterator<Item = (Range<usize>, &'a str)>) {
        if self.bookmarks.0.is_empty() {
            return;
        }
        let mut text = self.text().to_owned();
        let (mut old_end, mut new_end) = (0, 0);
        for (range, replacement) in edits {
            let start = new_end + range.start - old_end;
            let line = crate::bookmarks::line_at(&text, start);
            self.bookmarks
                .replace(&text, start..start + range.len(), replacement, line);
            text.replace_range(start..start + range.len(), replacement);
            old_end = range.end;
            new_end = start + replacement.len();
        }
    }

    fn replace_text(&mut self, range: Range<usize>, text: &str) {
        if !self.bookmarks.0.is_empty() {
            let line = self
                .line_starts()
                .partition_point(|&start| start <= range.start)
                - 1;
            self.bookmarks
                .replace(&self.state.text, range.clone(), text, line);
        }
        if let Some(gesture) = &self.selection_gesture {
            gesture.replaced(range.start, range.len(), text.len());
        }
        let Some(mut starts) = self.line_index.take() else {
            self.state.text.replace_range(range, text);
            return;
        };
        // Include both neighboring bytes: inserting/deleting between CR and LF
        // can merge or split a line ending outside the edited byte range.
        let first = starts.partition_point(|&offset| offset <= range.start.saturating_sub(1)) - 1;
        let suffix = starts.partition_point(|&offset| offset <= range.end.saturating_add(1));
        let scan_start = starts[first];
        let old_end = starts.get(suffix).copied().unwrap_or(self.text().len());
        let new_end = old_end - range.len() + text.len();
        let replace_end = (suffix + 1).min(starts.len());
        for offset in &mut starts[replace_end..] {
            *offset = *offset - range.len() + text.len();
        }
        self.state.text.replace_range(range, text);
        let rescanned = Self::scan_lines(&self.text()[scan_start..new_end]);
        starts.splice(
            first + 1..replace_end,
            rescanned
                .into_iter()
                .skip(1)
                .map(|offset| scan_start + offset),
        );
        let _ = self.line_index.set(starts);
    }

    /// Write a copy in the current encoding without changing document identity,
    /// savepoint, read-only flags, selection, or undo history.
    pub fn save_copy_with(
        &self,
        path: &Path,
        writer: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let bytes = self
            .to_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writer(path, &bytes)
    }

    /// Save using an injected platform writer. Only successful writes establish
    /// a savepoint. Platform adapters must preserve originals on failure.
    pub fn save_with(
        &mut self,
        path: &Path,
        writer: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        let bytes = self
            .to_bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writer(path, &bytes)?;
        self.path = Some(path.to_owned());
        self.saved_revision = self.text_revision.clone();
        self.saved_encoding = self.state.encoding;
        self.saved_digest = Some(blake3::hash(&bytes));
        self.disk_deleted = false;
        Ok(())
    }
}
