use crate::{Document, EditError, Selection};
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub enum CaseChange {
    Upper,
    Lower,
    Invert,
}
#[derive(Clone, Copy, Debug)]
pub enum TrimMode {
    Leading,
    Trailing,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhitespaceConversion {
    TabsToSpaces,
    SpacesToTabsLeading,
    SpacesToTabsAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateLines {
    Consecutive,
    Any,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndentationStyle {
    #[default]
    Tabs,
    Spaces,
}

impl Document {
    /// Move the selected stream block one line, following MoveSelectedLines.
    /// The resulting whole-line selection has its caret at the block start.
    pub fn move_lines(&mut self, down: bool) -> Result<(), EditError> {
        let selection = self.selection().range();
        let starts = self.line_starts();
        let first = starts.partition_point(|&p| p <= selection.start) - 1;
        let mut last = starts.partition_point(|&p| p <= selection.end) - 1;
        if first != last && selection.end == starts[last] {
            last -= 1;
        }
        if (!down && first == 0) || (down && last + 1 == starts.len()) {
            return Ok(());
        }
        let start = starts[first];
        let end = starts.get(last + 1).copied().unwrap_or(self.text().len());
        let ending = self.line_ending().as_str();
        let (range, replacement, moved_start, moved_end) = if down {
            let next = last + 1;
            let next_end = starts.get(next + 1).copied().unwrap_or(self.text().len());
            if next + 1 == starts.len() {
                let body_end = self.line_content(starts, last).end;
                let next_body = &self.text()[end..next_end];
                let block = &self.text()[start..body_end];
                let moved_start = start + next_body.len() + ending.len();
                (
                    start..next_end,
                    format!("{next_body}{ending}{block}"),
                    moved_start,
                    moved_start + block.len(),
                )
            } else {
                let next_text = &self.text()[end..next_end];
                let block = &self.text()[start..end];
                let moved_start = start + next_text.len();
                (
                    start..next_end,
                    format!("{next_text}{block}"),
                    moved_start,
                    moved_start + block.len(),
                )
            }
        } else {
            let previous = starts[first - 1];
            if last + 1 == starts.len() {
                let previous_end = self.line_content(starts, first - 1).end;
                let block = &self.text()[start..end];
                let previous_body = &self.text()[previous..previous_end];
                (
                    previous..end,
                    format!("{block}{ending}{previous_body}"),
                    previous,
                    previous + block.len() + ending.len(),
                )
            } else {
                let block = &self.text()[start..end];
                let previous_text = &self.text()[previous..start];
                (
                    previous..end,
                    format!("{block}{previous_text}"),
                    previous,
                    previous + block.len(),
                )
            }
        };
        self.apply_edits(&[(range, replacement)])?;
        self.select(Selection {
            anchor: moved_end,
            caret: moved_start,
        })
    }

    /// Insert a configured newline, optionally applying maintainIndentation's
    /// basic mode to the resulting line (language-aware indentation is separate).
    pub fn insert_newline(
        &mut self,
        basic_indent: bool,
        width: std::num::NonZeroUsize,
        style: IndentationStyle,
    ) -> Result<(), EditError> {
        let range = self.selection().range();
        let starts = self.line_starts();
        let line = starts.partition_point(|&p| p <= range.start) - 1;
        let prefix = &self.text()[starts[line]..range.start];
        let mut replacement = self.line_ending().as_str().to_owned();
        let mut end = range.end;
        let mut caret = range.start + replacement.len();
        if basic_indent && !prefix.is_empty() {
            let width = width.get();
            let columns = prefix
                .bytes()
                .take_while(|ch| matches!(ch, b' ' | b'\t'))
                .fold(0, |col, ch| {
                    col + if ch == b'\t' { width - col % width } else { 1 }
                });
            if columns > 0 {
                let existing = self.text()[end..]
                    .bytes()
                    .take_while(|ch| matches!(ch, b' ' | b'\t'))
                    .count();
                let existing_columns =
                    self.text()[end..end + existing].bytes().fold(0, |col, ch| {
                        col + if ch == b'\t' { width - col % width } else { 1 }
                    });
                // SetLineIndentation leaves equal-width whitespace untouched.
                let indent = if existing_columns == columns {
                    self.text()[end..end + existing].to_owned()
                } else if style == IndentationStyle::Spaces {
                    " ".repeat(columns)
                } else {
                    format!(
                        "{}{}",
                        "\t".repeat(columns / width),
                        " ".repeat(columns % width)
                    )
                };
                if existing == 0 {
                    caret += indent.len();
                }
                end += existing;
                replacement.push_str(&indent);
            }
        }
        self.apply_edits(&[(range.start..end, replacement)])?;
        self.select(Selection {
            anchor: caret,
            caret,
        })
    }

    /// Insert Blank Line Above/Below using the supplied view helper's configured
    /// EOL-width arithmetic. Invalid offsets in mixed-EOL UTF-8 are rejected.
    pub fn insert_blank_line(&mut self, below: bool) -> Result<(), EditError> {
        let starts = self.line_starts();
        let line = starts.partition_point(|&p| p <= self.selection().caret) - 1;
        let ending = self.line_ending().as_str();
        let position = if below {
            if line + 1 == starts.len() {
                self.text().len()
            } else {
                self.line_content(starts, line).end + ending.len()
            }
        } else if line == 0 {
            0
        } else {
            starts[line]
                .checked_sub(ending.len())
                .ok_or(EditError::InvalidRange)?
        };
        self.apply_edits(&[(position..position, ending.to_owned())])?;
        let target_line = line + usize::from(below);
        let target = self
            .line_starts()
            .get(target_line)
            .copied()
            .unwrap_or(self.text().len());
        self.select(Selection {
            anchor: target,
            caret: target,
        })
    }

    /// Join selected complete lines as IDM_EDIT_JOIN_LINES/SCI_LINESJOIN do.
    /// Preserve indentation and add a separator only when the preceding byte
    /// is not an ASCII space. A caret or single-line selection is a no-op.
    pub fn join_lines(&mut self) -> Result<(), EditError> {
        let selection = self.selection().range();
        let starts = self.line_starts();
        let first = starts.partition_point(|&p| p <= selection.start) - 1;
        let mut last = starts.partition_point(|&p| p <= selection.end) - 1;
        if first != last && selection.end == starts[last] {
            last -= 1;
        }
        if first == last {
            return Ok(());
        }
        let start = starts[first];
        let mut end = self.line_content(starts, last).end;
        let mut edits = Vec::new();
        let mut previous_space = false;
        for line in first..last {
            let content = self.line_content(starts, line);
            if !content.is_empty() {
                previous_space = self.text().as_bytes()[content.end - 1] == b' ';
            }
            let replacement = if previous_space {
                String::new()
            } else {
                " ".to_owned()
            };
            let ending = content.end..starts[line + 1];
            end = end - ending.len() + replacement.len();
            edits.push((ending, replacement));
            previous_space = true;
        }
        self.apply_edits(&edits)?;
        self.select(Selection {
            anchor: start,
            caret: end,
        })
    }

    /// Remove empty lines within the exact selection, or the whole document for
    /// an empty selection. Mirrors removeEmptyLine's two regex passes without
    /// requiring a regex engine; blank means ASCII space/tab only.
    pub fn remove_empty_lines(&mut self, include_blanks: bool) -> Result<(), EditError> {
        let selection = self.selection().range();
        let scope = if selection.is_empty() {
            0..self.text().len()
        } else {
            selection
        };
        let bytes = self.text().as_bytes();
        let starts = self.line_starts();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let first = starts.partition_point(|&start| start < scope.start);
        let mut scanned_until = scope.start;
        for &start in &starts[first..] {
            if start >= scope.end {
                break;
            }
            if start < scanned_until {
                continue;
            }
            let mut end = start;
            if include_blanks {
                while end < scope.end && matches!(bytes[end], b' ' | b'\t') {
                    end += 1;
                }
            }
            let whitespace_end = end;
            while end < scope.end && matches!(bytes[end], b'\r' | b'\n') {
                end += 1;
            }
            if end > whitespace_end {
                scanned_until = end;
                if let Some(previous) = ranges.last_mut().filter(|range| range.end == start) {
                    previous.end = end;
                } else {
                    ranges.push(start..end);
                }
            }
        }
        // The second upstream pass is allowed only when the selection reaches
        // the end of its end line, which for a stream range is document EOF.
        if scope.end == bytes.len() {
            let last = *starts.last().unwrap();
            let tail_is_blank = last == bytes.len()
                || (include_blanks && bytes[last..].iter().all(|ch| matches!(ch, b' ' | b'\t')));
            if tail_is_blank && last >= scope.start {
                let mut start = last;
                loop {
                    let before = start;
                    // Ignore ranges removed by the first pass when finding the
                    // trailing line ending in the resulting document.
                    while ranges.last().is_some_and(|range| range.end >= start) {
                        start = start.min(ranges.pop().unwrap().start);
                    }
                    while start > scope.start && matches!(bytes[start - 1], b'\r' | b'\n') {
                        start -= 1;
                    }
                    if start == before {
                        break;
                    }
                }
                if start < last || (include_blanks && last < bytes.len()) {
                    ranges.push(start..bytes.len());
                }
            }
        }
        let edits: Vec<_> = ranges
            .into_iter()
            .map(|range| (range, String::new()))
            .collect();
        self.apply_edits(&edits)
    }

    /// Stream-selection indentation, following Editor::Indent and the supplied
    /// IDM_EDIT_INS_TAB/RMV_TAB single-line override.
    pub fn indent(
        &mut self,
        forwards: bool,
        line_command: bool,
        width: std::num::NonZeroUsize,
        style: IndentationStyle,
    ) -> Result<(), EditError> {
        let width = width.get();
        let original = self.selection();
        let range = original.range();
        let starts = self.line_starts();
        let first = starts.partition_point(|&p| p <= range.start) - 1;
        let last = starts.partition_point(|&p| p <= range.end) - 1;
        let indentation = |text: &str| {
            let mut columns = 0;
            let mut bytes = 0;
            for ch in text.bytes().take_while(|ch| matches!(ch, b' ' | b'\t')) {
                columns += if ch == b'\t' {
                    width - columns % width
                } else {
                    1
                };
                bytes += 1;
            }
            (bytes, columns)
        };
        let padding = |columns: usize| {
            if style == IndentationStyle::Spaces {
                return " ".repeat(columns);
            }
            format!(
                "{}{}",
                "\t".repeat(columns / width),
                " ".repeat(columns % width)
            )
        };
        if first == last {
            let start = starts[first];
            let end = self.line_content(starts, first).end;
            let (old_bytes, old_columns) = indentation(&self.text()[start..end]);
            if line_command {
                let columns = if forwards {
                    old_columns + width
                } else {
                    old_columns.saturating_sub(width)
                };
                if columns == old_columns {
                    return Ok(());
                }
                let replacement = padding(columns);
                let old_end = start + old_bytes;
                let new_end = start + replacement.len();
                // The menu helper preserves endpoints inside retained indentation.
                let map = |pos: usize| {
                    if pos >= old_end {
                        new_end + pos - old_end
                    } else if new_end < old_end {
                        pos.min(new_end)
                    } else {
                        pos
                    }
                };
                let selection = Selection {
                    anchor: map(original.anchor),
                    caret: map(original.caret),
                };
                self.apply_edits(&[(start..old_end, replacement)])?;
                return self.select(selection);
            }
            if forwards {
                // Delete the single-line selection before deciding whether Tab
                // lies inside indentation, as Scintilla does.
                let mut line = self.text()[start..range.start].to_owned();
                line.push_str(&self.text()[range.end..end]);
                let (bytes, columns) = indentation(&line);
                let caret = range.start - start;
                let target;
                if caret <= bytes {
                    let replacement = padding(columns + width - columns % width);
                    target = start + replacement.len();
                    line.replace_range(..bytes, &replacement);
                } else {
                    let column = line[..caret].chars().fold(0, |column, ch| {
                        column
                            + if ch == '\t' {
                                width - column % width
                            } else {
                                1
                            }
                    });
                    let inserted = match style {
                        IndentationStyle::Tabs => "\t".to_owned(),
                        IndentationStyle::Spaces => " ".repeat(width - column % width),
                    };
                    target = range.start + inserted.len();
                    line.insert_str(caret, &inserted);
                }
                self.apply_edits(&[(start..end, line)])?;
                return self.select(Selection {
                    anchor: target,
                    caret: target,
                });
            }
            if original.caret <= start + old_bytes {
                let columns = old_columns.saturating_sub(width);
                let replacement = if columns == old_columns {
                    self.text()[start..start + old_bytes].to_owned()
                } else {
                    padding(columns)
                };
                let target = start + replacement.len();
                self.apply_edits(&[(start..start + old_bytes, replacement)])?;
                return self.select(Selection {
                    anchor: target,
                    caret: target,
                });
            }
            // Outside indentation Shift+Tab moves left without deleting text.
            let (_, column) =
                self.line_column(original.caret, std::num::NonZeroUsize::new(width).unwrap())?;
            let target_column = column.saturating_sub(1) / width * width;
            let mut target = start;
            let mut current_column = 0;
            for (offset, ch) in self.text()[start..original.caret].char_indices() {
                let position = start + offset;
                if current_column > target_column {
                    break;
                }
                target = position;
                current_column += if ch == '\t' {
                    width - current_column % width
                } else {
                    1
                };
            }
            return self.select(Selection {
                anchor: target,
                caret: target,
            });
        }
        let bottom = if range.end == starts[last] {
            last - 1
        } else {
            last
        };
        let scope_start = starts[first];
        let scope_end = starts.get(bottom + 1).copied().unwrap_or(self.text().len());
        let mut edits = Vec::new();
        let mut new_scope_end = scope_end;
        for line in first..=bottom {
            let content = self.line_content(starts, line);
            if forwards && content.is_empty() {
                continue;
            }
            let (bytes, columns) = indentation(&self.text()[content.clone()]);
            let next = if forwards {
                columns + width
            } else {
                columns.saturating_sub(width)
            };
            if next == columns {
                continue;
            }
            let replacement = padding(next);
            new_scope_end = new_scope_end - bytes + replacement.len();
            edits.push((content.start..content.start + bytes, replacement));
        }
        self.apply_edits(&edits)?;
        let selection = if original.anchor <= original.caret {
            Selection {
                anchor: scope_start,
                caret: new_scope_end,
            }
        } else {
            Selection {
                anchor: new_scope_end,
                caret: scope_start,
            }
        };
        self.select(selection)
    }

    /// Match the supplied duplicate-line commands, including their distinct
    /// treatment of line endings. No selection means the whole document.
    pub fn remove_duplicate_lines(&mut self, mode: DuplicateLines) -> Result<(), EditError> {
        let starts = self.line_starts();
        let selection = self.selection().range();
        let (first, mut last) = if selection.is_empty() {
            (0, starts.len() - 1)
        } else {
            (
                starts.partition_point(|&p| p <= selection.start) - 1,
                starts.partition_point(|&p| p <= selection.end) - 1,
            )
        };
        if !selection.is_empty() && first != last && selection.end == starts[last] {
            last -= 1;
        }
        if first == last {
            return Ok(());
        }
        let line_range =
            |line: usize| starts[line]..starts.get(line + 1).copied().unwrap_or(self.text().len());
        if mode == DuplicateLines::Any {
            // Upstream splits using the configured EOL, not every possible EOL.
            let scope = starts[first]..line_range(last).end;
            let entire_tail = last == starts.len() - 1;
            let ending = self.line_ending().as_str();
            let mut lines: Vec<_> = self.text()[scope.clone()].split(ending).collect();
            if !entire_tail && lines.last() == Some(&"") {
                lines.pop();
            }
            let count = lines.len();
            let mut seen = std::collections::HashSet::new();
            lines.retain(|line| seen.insert(*line));
            if lines.len() == count {
                return Ok(());
            }
            let mut replacement = lines.join(ending);
            if !entire_tail {
                replacement.push_str(ending);
            }
            return self.apply_edits(&[(scope, replacement)]);
        }

        let mut kept = Vec::new();
        let mut removed = Vec::new();
        for line in first..=last {
            let duplicate = kept.last().is_some_and(|&previous| {
                self.text()[line_range(previous)] == self.text()[line_range(line)]
            });
            if duplicate {
                removed.push(line);
            } else {
                kept.push(line);
            }
        }
        // An unterminated final line differs from its predecessor by its EOL.
        // Upstream keeps that last copy by deleting the preceding complete line.
        if last == starts.len() - 1 && kept.len() > 1 && !self.text()[line_range(last)].is_empty() {
            let previous = kept[kept.len() - 2];
            if self.text()[self.line_content(starts, previous)] == self.text()[line_range(last)] {
                removed.push(previous);
            }
        }
        removed.sort_unstable();
        let edits: Vec<_> = removed
            .into_iter()
            .map(|line| (line_range(line), String::new()))
            .collect();
        self.apply_edits(&edits)
    }

    /// Convert at tab stops, counting Unicode scalars as upstream wsTabConvert
    /// does. Partial selections expand to lines; a next-line start is excluded.
    pub fn convert_whitespace(
        &mut self,
        mode: WhitespaceConversion,
        tab_width: std::num::NonZeroUsize,
    ) -> Result<(), EditError> {
        let width = tab_width.get();
        let starts = self.line_starts();
        let selected = self.selection().range();
        let first = if selected.is_empty() {
            0
        } else {
            starts.partition_point(|&p| p <= selected.start) - 1
        };
        let end_line = if selected.is_empty() {
            starts.len() - 1
        } else {
            starts.partition_point(|&p| p <= selected.end) - 1
        };
        let excludes_end = !selected.is_empty() && selected.end == starts[end_line];
        let last = if excludes_end { end_line - 1 } else { end_line };
        let mut edits = Vec::new();
        for line in first..=last {
            let range = self.line_content(starts, line);
            let body = &self.text()[range.clone()];
            let mut offset = 0;
            let mut column = 0;
            let mut leading = true;
            while offset < body.len() {
                let character = body[offset..].chars().next().unwrap();
                let to_stop = width - column % width;
                if mode == WhitespaceConversion::TabsToSpaces && character == '\t' {
                    edits.push((
                        range.start + offset..range.start + offset + 1,
                        " ".repeat(to_stop),
                    ));
                } else if character == ' '
                    && (mode == WhitespaceConversion::SpacesToTabsAll
                        || mode == WhitespaceConversion::SpacesToTabsLeading && leading)
                {
                    let spaces = body[offset..].bytes().take_while(|&b| b == b' ').count();
                    let following = body.as_bytes().get(offset + to_stop);
                    if spaces >= to_stop && (to_stop > 1 || matches!(following, Some(b' ' | b'\t')))
                    {
                        edits.push((
                            range.start + offset..range.start + offset + to_stop,
                            "\t".into(),
                        ));
                        offset += to_stop;
                        column += to_stop;
                        continue;
                    }
                    if spaces < to_stop && body.as_bytes().get(offset + spaces) == Some(&b'\t') {
                        edits.push((
                            range.start + offset..range.start + offset + spaces + 1,
                            "\t".into(),
                        ));
                        offset += spaces + 1;
                        column += to_stop;
                        continue;
                    }
                }
                // Match upstream's leading-only boundary, including existing tabs
                // and residual spaces that were not consumed by a conversion.
                leading = false;
                column += if character == '\t' { to_stop } else { 1 };
                offset += character.len_utf8();
            }
        }
        let changed = !edits.is_empty();
        self.apply_edits(&edits)?;
        if changed && !selected.is_empty() {
            let starts = self.line_starts();
            self.select(Selection {
                anchor: starts[first],
                caret: if excludes_end {
                    starts[end_line]
                } else {
                    self.line_content(starts, last).end
                },
            })?;
        }
        Ok(())
    }

    /// Match the brace immediately before the caret, falling back to the brace
    /// at the caret. This is plain-text matching; lexer style filtering is not
    /// available until syntax styling is implemented.
    pub fn matching_brace(&self) -> Option<(usize, usize)> {
        let bytes = self.text().as_bytes();
        let caret = self.selection().caret;
        let is_brace = |byte: u8| b"[](){}".contains(&byte);
        let at = if caret > 0 && is_brace(bytes[caret - 1]) {
            caret - 1
        } else if bytes.get(caret).copied().is_some_and(is_brace) {
            caret
        } else {
            return None;
        };
        let (other, forward) = match bytes[at] {
            b'(' => (b')', true),
            b'[' => (b']', true),
            b'{' => (b'}', true),
            b')' => (b'(', false),
            b']' => (b'[', false),
            b'}' => (b'{', false),
            _ => unreachable!(),
        };
        let mut depth = 1;
        let mut visit = |position: usize| {
            if bytes[position] == bytes[at] {
                depth += 1;
            } else if bytes[position] == other {
                depth -= 1;
            }
            depth == 0
        };
        let opposite = if forward {
            (at + 1..bytes.len()).find(|&i| visit(i))
        } else {
            (0..at).rev().find(|&i| visit(i))
        }?;
        Some((at, opposite))
    }

    pub fn go_to_matching_brace(&mut self, select_pair: bool) -> Result<bool, EditError> {
        let Some((at, opposite)) = self.matching_brace() else {
            return Ok(false);
        };
        let selection = if select_pair {
            Selection {
                anchor: at.min(opposite),
                caret: at.max(opposite) + 1,
            }
        } else {
            Selection {
                anchor: opposite,
                caret: opposite,
            }
        };
        self.select(selection)?;
        Ok(true)
    }
    /// Sort complete selected lines, or the whole document without a selection.
    /// UTF-16 comparison matches Windows wstring ordering in the supplied source.
    /// Like ScintillaEditView::sortLines, split and join using the configured EOL.
    pub fn order_lines(&mut self, order: LineOrder) -> Result<(), EditError> {
        let starts = self.line_starts();
        let selected = self.selection().range();
        let (first, last) = if selected.is_empty() {
            (0, starts.len() - 1)
        } else {
            let first = starts.partition_point(|&p| p <= selected.start) - 1;
            let mut last = starts.partition_point(|&p| p <= selected.end) - 1;
            if first != last && selected.end == starts[last] {
                last -= 1;
            }
            (first, last)
        };
        if first == last {
            return Ok(());
        }
        let range = starts[first]..starts.get(last + 1).copied().unwrap_or(self.text().len());
        let ending = self.line_ending().as_str();
        let through_end = last == starts.len() - 1;
        let mut lines: Vec<_> = self.text()[range.clone()]
            .split(ending)
            .map(str::to_owned)
            .collect();
        if !through_end && lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        match order {
            LineOrder::Ascending => lines.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16())),
            LineOrder::Descending => lines.sort_by(|a, b| b.encode_utf16().cmp(a.encode_utf16())),
            LineOrder::CaseInsensitiveAscending => {
                lines.sort_by_cached_key(|line| case_insensitive_sort_key(line))
            }
            LineOrder::CaseInsensitiveDescending => {
                lines.sort_by_cached_key(|line| std::cmp::Reverse(case_insensitive_sort_key(line)))
            }
            LineOrder::LengthAscending => {
                lines.sort_by_cached_key(|line| line.encode_utf16().count())
            }
            LineOrder::LengthDescending => {
                lines.sort_by_cached_key(|line| std::cmp::Reverse(line.encode_utf16().count()))
            }
            LineOrder::Reverse => lines.reverse(),
            LineOrder::Random(seed) => crate::random_sort::shuffle(&mut lines, seed),
            LineOrder::DecimalDotAscending
            | LineOrder::DecimalDotDescending
            | LineOrder::DecimalCommaAscending
            | LineOrder::DecimalCommaDescending => {
                let comma = matches!(
                    order,
                    LineOrder::DecimalCommaAscending | LineOrder::DecimalCommaDescending
                );
                let descending = matches!(
                    order,
                    LineOrder::DecimalCommaDescending | LineOrder::DecimalDotDescending
                );
                let mut keyed = Vec::with_capacity(lines.len());
                for (index, line) in lines.into_iter().enumerate() {
                    let key = crate::decimal_sort::key(&line, comma)
                        .map_err(|_| EditError::InvalidNumericLine(first + index + 1))?;
                    keyed.push((line, key));
                }
                keyed.sort_by(|a, b| {
                    let result = a.1.partial_cmp(&b.1).expect("finite decimal keys");
                    if descending { result.reverse() } else { result }
                });
                lines = keyed.into_iter().map(|(line, _)| line).collect();
            }
            LineOrder::IntegerAscending | LineOrder::IntegerDescending => {
                let mut keyed: Vec<_> = lines
                    .into_iter()
                    .map(|line| {
                        let key: Vec<u16> = line.encode_utf16().collect();
                        (line, key)
                    })
                    .collect();
                keyed.sort_by(|a, b| {
                    let compared = crate::integer_sort::compare(&a.1, &b.1);
                    if matches!(order, LineOrder::IntegerDescending) {
                        compared.reverse()
                    } else {
                        compared
                    }
                });
                lines = keyed.into_iter().map(|(line, _)| line).collect();
            }
        }
        let mut joined = lines.join(ending);
        if !through_end {
            joined.push_str(ending);
        }
        self.apply_edits(&[(range, joined)])?;
        if !selected.is_empty() {
            let updated = self.line_starts();
            self.select(Selection {
                anchor: updated.get(first).copied().unwrap_or(self.text().len()),
                caret: if last < updated.len() {
                    self.line_content(updated, last).end
                } else {
                    self.text().len()
                },
            })?;
        }
        Ok(())
    }
    fn line_content(&self, starts: &[usize], line: usize) -> Range<usize> {
        let start = starts[line];
        let end = starts.get(line + 1).copied().unwrap_or(self.text().len());
        let body = self.text()[start..end].trim_end_matches(['\r', '\n']);
        start..start + body.len()
    }

    /// SortLocale's stream-selection behavior. Platform adapters supply locale
    /// keys; all keys are computed before text or history is changed.
    pub fn order_lines_locale(
        &mut self,
        descending: bool,
        mut key: impl FnMut(&str) -> Result<Vec<u8>, EditError>,
    ) -> Result<(), EditError> {
        let selection = self.selection();
        let selected = selection.range();
        let starts = self.line_starts();
        let first = if selected.is_empty() {
            0
        } else {
            starts.partition_point(|&p| p <= selected.start) - 1
        };
        let end_position = if selected.is_empty() {
            self.text().len()
        } else {
            selected.end
        };
        let mut last = starts.partition_point(|&p| p <= end_position) - 1;
        if starts[last] == end_position {
            let Some(previous) = last.checked_sub(1) else {
                return Ok(());
            };
            last = previous;
        }
        if last <= first {
            return Ok(());
        }
        let range = starts[first]..starts.get(last + 1).copied().unwrap_or(self.text().len());
        let missing_eol = last == starts.len() - 1;
        let caret_line = starts.partition_point(|&p| p <= selection.caret) - 1;
        let caret_offset = selection.caret - starts[caret_line];
        let mut lines = Vec::with_capacity(last - first + 1);
        for line in first..=last {
            let body = self.line_content(starts, line);
            let content = self.line_range(line).expect("valid line");
            let sort_key = if body.is_empty() {
                Vec::new()
            } else {
                key(&self.text()[body])?
            };
            lines.push((line, content, sort_key));
        }
        lines.sort_by(|a, b| {
            let order = a.2.cmp(&b.2);
            if descending { order.reverse() } else { order }
        });
        let mut output = String::with_capacity(range.len() + self.line_ending().as_str().len());
        let mut caret = selection.caret;
        for (line, content, _) in lines {
            if line == caret_line {
                caret = range.start + output.len() + caret_offset;
            }
            output.push_str(&self.text()[content]);
            if missing_eol && line == last {
                output.push_str(self.line_ending().as_str());
            }
        }
        if missing_eol {
            if output.ends_with('\n') {
                output.pop();
            }
            if output.ends_with('\r') {
                output.pop();
            }
        }
        let end = range.start + output.len();
        self.apply_edits(&[(range.clone(), output)])?;
        let restored = if selected.is_empty() {
            caret = caret.min(self.text().len());
            while !self.text().is_char_boundary(caret) {
                caret -= 1;
            }
            Selection {
                anchor: caret,
                caret,
            }
        } else if selection.anchor < selection.caret {
            Selection {
                anchor: range.start,
                caret: end,
            }
        } else {
            Selection {
                anchor: end,
                caret: range.start,
            }
        };
        self.select(restored)
    }

    /// Like Notepad++ doTrim: no selection means the whole document; a
    /// partial selection expands to complete lines, excluding a next line
    /// whose start is exactly the selection end. Only space and tab are trimmed.
    pub fn trim_lines(&mut self, mode: TrimMode) -> Result<(), EditError> {
        let starts = self.line_starts();
        let selected = self.selection().range();
        let scope = if selected.is_empty() {
            0..self.text().len()
        } else {
            let first = starts.partition_point(|&p| p <= selected.start) - 1;
            let last = starts.partition_point(|&p| p <= selected.end) - 1;
            let end = if selected.end == starts[last] {
                selected.end
            } else {
                selected.end.max(self.line_content(starts, last).end)
            };
            starts[first]..end
        };
        let mut edits = Vec::new();
        for line in 0..starts.len() {
            let range = self.line_content(starts, line);
            if range.start < scope.start || range.start >= scope.end {
                continue;
            }
            let text = &self.text()[range.clone()];
            let leading = text.len() - text.trim_start_matches([' ', '\t']).len();
            let trailing = text.trim_end_matches([' ', '\t']).len();
            let left = if matches!(mode, TrimMode::Leading | TrimMode::Both) {
                leading
            } else {
                0
            };
            let right = if matches!(mode, TrimMode::Trailing | TrimMode::Both) {
                trailing.max(left)
            } else {
                text.len()
            };
            if left > 0 {
                edits.push((range.start..range.start + left, String::new()));
            }
            if right < text.len() {
                edits.push((range.start + right..range.end, String::new()));
            }
        }
        let old_length = self.text().len();
        self.apply_edits(&edits)?;
        let removed = old_length - self.text().len();
        if removed > 0 && !selected.is_empty() {
            self.select(Selection {
                anchor: scope.start,
                caret: scope.end - removed,
            })?;
        }
        Ok(())
    }

    /// Only selected text is converted; empty selection is a no-op. Rust's
    /// Unicode mappings can expand a scalar (for example ß -> SS).
    pub fn change_case(&mut self, case: CaseChange) -> Result<(), EditError> {
        self.transform_selection(|text| match case {
            CaseChange::Upper => text.to_uppercase(),
            CaseChange::Lower => text.to_lowercase(),
            CaseChange::Invert => {
                let mut result = String::with_capacity(text.len());
                for character in text.chars() {
                    if character.is_lowercase() {
                        result.extend(character.to_uppercase());
                    } else {
                        result.extend(character.to_lowercase());
                    }
                }
                result
            }
        })
    }

    /// Apply a platform text transform as one undoable edit. Like the source
    /// case commands, an empty selection is unchanged and the result is selected
    /// forwards, including when the original selection was reversed.
    pub fn transform_selection(
        &mut self,
        transform: impl FnOnce(&str) -> String,
    ) -> Result<(), EditError> {
        let range = self.selection().range();
        if range.is_empty() {
            return Ok(());
        }
        let text = &self.text()[range.clone()];
        let replacement = transform(text);
        let end = range.start + replacement.len();
        let start = range.start;
        self.apply_edits(&[(range, replacement)])?;
        self.select(Selection {
            anchor: start,
            caret: end,
        })
    }

    /// SCI_LINEDUPLICATE duplicates the caret's line, even with a selection.
    /// Insert the document's preferred EOL before the duplicate's content,
    /// leaving any original line ending after the duplicated content intact.
    pub fn duplicate_line(&mut self) -> Result<(), EditError> {
        let starts = self.line_starts();
        let original = self.selection();
        let line = starts.partition_point(|&p| p <= original.caret) - 1;
        let range = self.line_content(starts, line);
        let inserted = format!(
            "{}{}",
            self.line_ending().as_str(),
            &self.text()[range.clone()]
        );
        let length = inserted.len();
        let position = range.end;
        self.apply_edits(&[(position..position, inserted)])?;
        let shift = |offset| {
            if offset >= position {
                offset + length
            } else {
                offset
            }
        };
        self.select(Selection {
            anchor: shift(original.anchor),
            caret: shift(original.caret),
        })
    }

    /// Whole-line clipboard commands include the line containing the selection's
    /// end even when that endpoint is exactly at the start of a line.
    pub fn selected_line_range(&self) -> std::ops::Range<usize> {
        let selection = self.selection();
        let starts = self.line_starts();
        let first = starts.partition_point(|&p| p <= selection.anchor.min(selection.caret)) - 1;
        let last = starts.partition_point(|&p| p <= selection.anchor.max(selection.caret)) - 1;
        starts[first]..starts.get(last + 1).copied().unwrap_or(self.text().len())
    }

    /// SCI_LINECUT selects whole lines before deletion; undo restores that range.
    /// Call only after successfully publishing the selected line text to a clipboard.
    pub fn cut_selected_lines(&mut self) -> Result<(), EditError> {
        if self.is_read_only() {
            return Err(EditError::ReadOnly);
        }
        let range = self.selected_line_range();
        self.select(Selection {
            anchor: range.end,
            caret: range.start,
        })?;
        self.replace(range, "")
    }

    /// SCI_LINEDELETE removes the caret line, not every selected line. Deleting
    /// an unterminated final line leaves the preceding line ending intact.
    pub fn delete_line(&mut self) -> Result<(), EditError> {
        let starts = self.line_starts();
        let line = starts.partition_point(|&p| p <= self.selection().caret) - 1;
        let start = starts[line];
        let end = starts.get(line + 1).copied().unwrap_or(self.text().len());
        self.apply_edits(&[(start..end, String::new())])
    }

    /// Expand an empty selection using SCI_WORDSTART/ENDPOSITION with onlyWordCharacters.
    pub fn expand_word_selection(&mut self) -> Result<(), EditError> {
        let selection = self.selection();
        if selection.anchor == selection.caret {
            let range = crate::word_classes::word_range(self.text(), selection.caret);
            self.select(Selection {
                anchor: range.start,
                caret: range.end,
            })?;
        }
        Ok(())
    }

    /// SCI_PARAUP/DOWN treat only space/tab-only lines as paragraph separators.
    pub fn move_paragraph(&mut self, down: bool, extend: bool) -> Result<(), EditError> {
        let selection = self.selection();
        let starts = self.line_starts();
        let mut line = starts.partition_point(|&start| start <= selection.caret) - 1;
        let white = |line| {
            self.text()[self.line_content(starts, line)]
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t'))
        };
        let caret = if down {
            while line < starts.len() && !white(line) {
                line += 1;
            }
            while line < starts.len() && white(line) {
                line += 1;
            }
            starts.get(line).copied().unwrap_or(self.text().len())
        } else {
            // Use an exclusive upper bound to represent the source's line -1.
            let mut bound = line + usize::from(selection.caret != starts[line]);
            while bound > 0 && white(bound - 1) {
                bound -= 1;
            }
            while bound > 0 && !white(bound - 1) {
                bound -= 1;
            }
            starts[bound]
        };
        self.select(Selection {
            anchor: if extend { selection.anchor } else { caret },
            caret,
        })
    }

    /// SCI_LINEEND targets the content end of the caret's physical line.
    pub fn move_line_end(&mut self, extend: bool) -> Result<(), EditError> {
        let selection = self.selection();
        let starts = self.line_starts();
        let line = starts.partition_point(|&start| start <= selection.caret) - 1;
        let caret = self.line_content(starts, line).end;
        self.select(Selection {
            anchor: if extend { selection.anchor } else { caret },
            caret,
        })
    }

    /// SCI_DOCUMENTSTART/END, retaining the current anchor for Extend.
    pub fn move_document_boundary(&mut self, end: bool, extend: bool) -> Result<(), EditError> {
        let caret = if end { self.text().len() } else { 0 };
        self.select(Selection {
            anchor: if extend {
                self.selection().anchor
            } else {
                caret
            },
            caret,
        })
    }

    /// SCI_VCHOME toggles between the first non-space/tab and the line start.
    pub fn move_home(&mut self, extend: bool) -> Result<(), EditError> {
        let selection = self.selection();
        let starts = self.line_starts();
        let line = starts.partition_point(|&start| start <= selection.caret) - 1;
        let content = self.line_content(starts, line);
        let indentation = self.text()[content.clone()]
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let first_text = content.start + indentation;
        let caret = if selection.caret == first_text {
            content.start
        } else {
            first_text
        };
        self.select(Selection {
            anchor: if extend { selection.anchor } else { caret },
            caret,
        })
    }

    /// SCI_WORDPARTLEFT/RIGHT navigate ASCII case/digit groups and non-ASCII runs.
    pub fn move_word_part(&mut self, right: bool, extend: bool) -> Result<(), EditError> {
        let selection = self.selection();
        let caret = crate::word_parts::boundary(self.text(), selection.caret, right);
        self.select(Selection {
            anchor: if extend { selection.anchor } else { caret },
            caret,
        })
    }

    /// SCI_WORDLEFT/RIGHT move from the caret, retaining the anchor for Extend.
    /// Navigation works in read-only documents and creates no undo record.
    pub fn move_word(&mut self, rightwards: bool, extend: bool) -> Result<(), EditError> {
        let selection = self.selection();
        let caret = crate::word_classes::next_word_start(self.text(), selection.caret, rightwards);
        self.select(Selection {
            anchor: if extend { selection.anchor } else { caret },
            caret,
        })
    }

    /// SCI_DELWORDLEFT/RIGHT delete from the caret rather than deleting the
    /// selection. Leftward deletion maps both endpoints; rightward collapses it.
    pub fn delete_to_word_start(&mut self, rightwards: bool) -> Result<(), EditError> {
        let caret = self.selection().caret;
        let boundary = crate::word_classes::next_word_start(self.text(), caret, rightwards);
        let range = if rightwards {
            caret..boundary
        } else {
            boundary..caret
        };
        self.apply_edits(&[(range, String::new())])?;
        if rightwards {
            self.select(Selection {
                anchor: caret,
                caret,
            })?;
        }
        Ok(())
    }

    /// SCI_DELLINELEFT/RIGHT operate from the caret, regardless of the selected
    /// range. Rightward deletion collapses the selection; leftward deletion maps
    /// its endpoints. Neither command joins adjacent lines at their boundaries.
    pub fn delete_to_line_boundary(&mut self, rightwards: bool) -> Result<(), EditError> {
        let caret = self.selection().caret;
        let starts = self.line_starts();
        let line = starts.partition_point(|&p| p <= caret) - 1;
        let content = self.line_content(starts, line);
        let range = if rightwards {
            // A caret inside CRLF is beyond LineEnd; upstream deletes nothing.
            caret..content.end.max(caret)
        } else {
            content.start..caret
        };
        self.apply_edits(&[(range, String::new())])?;
        if rightwards {
            self.select(Selection {
                anchor: caret,
                caret,
            })?;
        }
        Ok(())
    }

    /// SCI_LINETRANSPOSE swaps the caret line's body with its predecessor,
    /// retaining the line-ending slots and moving the caret to its line start.
    pub fn transpose_line(&mut self) -> Result<(), EditError> {
        let starts = self.line_starts();
        let line = starts.partition_point(|&p| p <= self.selection().caret) - 1;
        if line == 0 {
            return Ok(());
        }
        let previous = self.line_content(starts, line - 1);
        let current = self.line_content(starts, line);
        let target = current.start - previous.len() + current.len();
        let edits = [
            (previous.clone(), self.text()[current.clone()].to_owned()),
            (current, self.text()[previous].to_owned()),
        ];
        self.apply_edits(&edits)?;
        self.select(Selection {
            anchor: target,
            caret: target,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LineOrder {
    Ascending,
    Descending,
    CaseInsensitiveAscending,
    CaseInsensitiveDescending,
    LengthAscending,
    LengthDescending,
    IntegerAscending,
    IntegerDescending,
    DecimalDotAscending,
    DecimalDotDescending,
    DecimalCommaAscending,
    DecimalCommaDescending,
    Reverse,
    /// A reproducible shuffle; platform callers supply the seed.
    Random(u64),
}

// The supplied application does not change the CRT locale. _wcsicmp in its
// default C locale folds ASCII capitals and compares NUL-terminated UTF-16.
fn case_insensitive_sort_key(line: &str) -> Vec<u16> {
    line.encode_utf16()
        .take_while(|&unit| unit != 0)
        .map(|unit| {
            if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
                unit + 32
            } else {
                unit
            }
        })
        .collect()
}
