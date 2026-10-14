use crate::{Document, EditError};
use std::{borrow::Cow, ops::Range};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Normal,
    Extended,
    Regex,
}

impl SearchMode {
    /// Notepad++ Extended syntax uses fixed-width numeric escapes, interpreted
    /// as UTF-16 units. Unknown and incomplete escapes remain literal text.
    pub fn decode(self, input: &str) -> Result<Cow<'_, str>, EditError> {
        if self != Self::Extended || !input.contains('\\') {
            return Ok(Cow::Borrowed(input));
        }
        let input: Vec<u16> = input.encode_utf16().collect();
        let mut output = Vec::with_capacity(input.len());
        let mut i = 0;
        while i < input.len() {
            let unit = input[i];
            i += 1;
            if unit != b'\\' as u16 || i == input.len() {
                output.push(unit);
                continue;
            }
            let escape = input[i];
            i += 1;
            let simple = match escape {
                114 => Some(13), // r
                110 => Some(10), // n
                116 => Some(9),  // t
                48 => Some(0),   // 0
                92 => Some(92),  // backslash
                _ => None,
            };
            if let Some(decoded) = simple {
                output.push(decoded);
                continue;
            }
            let (size, base) = match escape {
                98 => (8, 2),
                111 => (3, 8),
                100 => (3, 10),
                120 => (2, 16),
                117 => (4, 16),
                _ => (0, 0),
            };
            let number = if size > 0 {
                input.get(i..i + size).and_then(|digits| {
                    digits.iter().try_fold(0u16, |value, &digit| {
                        char::from_u32(digit as u32)?
                            .to_digit(base)
                            .map(|digit| value * base as u16 + digit as u16)
                    })
                })
            } else {
                None
            };
            if let Some(number) = number {
                output.push(number);
                i += size;
            } else {
                output.extend([b'\\' as u16, escape]);
            }
        }
        String::from_utf16(&output)
            .map(Cow::Owned)
            .map_err(|_| EditError::InvalidSearchEscape)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub match_case: bool,
    pub whole_word: bool,
    pub wrap: bool,
    pub direction: SearchDirection,
}
impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            mode: SearchMode::Normal,
            match_case: false,
            whole_word: false,
            wrap: true,
            direction: SearchDirection::Forward,
        }
    }
}

pub(crate) fn prepare_needle(
    needle: &str,
    options: SearchOptions,
) -> Result<Cow<'_, str>, EditError> {
    let needle = options.mode.decode(needle)?;
    if needle.is_empty() {
        return Err(EditError::EmptySearch);
    }
    Ok(needle)
}

/// Iterate candidate starting positions without allocating a list of matches.
/// Matching can extend past the candidate range, which matters for overlaps.
pub(crate) fn match_ranges<'a>(
    text: &'a str,
    needle: &'a str,
    options: SearchOptions,
    candidates: Range<usize>,
) -> impl DoubleEndedIterator<Item = Range<usize>> + 'a {
    let base = candidates.start;
    let folded = (!options.match_case).then(|| crate::case_fold::text(needle));
    text[candidates]
        .char_indices()
        .filter_map(move |(offset, _)| {
            let start = base + offset;
            let end = if options.match_case {
                if !text[start..].starts_with(needle) {
                    return None;
                }
                start + needle.len()
            } else {
                let mut haystack = text[start..].char_indices();
                let mut remaining = folded.as_ref().unwrap().as_slice();
                let mut end = start;
                let mut buffer = [0; 4];
                while !remaining.is_empty() {
                    let (offset, actual) = haystack.next()?;
                    let part = crate::case_fold::character(actual, &mut buffer);
                    remaining = remaining.strip_prefix(part)?;
                    end = start + offset + actual.len_utf8();
                }
                end
            };
            if options.whole_word && !crate::word_classes::whole_word(text, start, end) {
                return None;
            }
            Some(start..end)
        })
}

impl Document {
    /// Literal matches use the supplied Scintilla Unicode folding table. Word edges
    /// use Scintilla's default UTF-8 character classes and Unicode 16.0 table.
    /// Configurable word classes and legacy code pages remain separate work.
    pub fn search_matches(
        &self,
        needle: &str,
        options: SearchOptions,
    ) -> Result<Vec<Range<usize>>, EditError> {
        if options.mode == SearchMode::Regex {
            return crate::regex_search::matches(
                self.text(),
                needle,
                options,
                0..self.text().len(),
            );
        }
        let needle = prepare_needle(needle, options)?;
        Ok(match_ranges(self.text(), &needle, options, 0..self.text().len()).collect())
    }

    pub fn search(
        &self,
        needle: &str,
        start: usize,
        options: SearchOptions,
    ) -> Result<Option<Range<usize>>, EditError> {
        if !self.text().is_char_boundary(start) {
            return Err(EditError::InvalidRange);
        }
        if options.mode == SearchMode::Regex {
            let matches = self.search_matches(needle, options)?;
            return Ok(match options.direction {
                SearchDirection::Forward => matches
                    .iter()
                    .find(|m| m.start >= start)
                    .cloned()
                    .or_else(|| options.wrap.then(|| matches.first().cloned()).flatten()),
                SearchDirection::Backward => matches
                    .iter()
                    .rev()
                    .find(|m| m.end <= start)
                    .cloned()
                    .or_else(|| options.wrap.then(|| matches.last().cloned()).flatten()),
            });
        }
        let needle = prepare_needle(needle, options)?;
        let scan = |range| match_ranges(self.text(), &needle, options, range);
        let found = match options.direction {
            SearchDirection::Forward => scan(start..self.text().len()).next().or_else(|| {
                if options.wrap {
                    scan(0..start).next()
                } else {
                    None
                }
            }),
            SearchDirection::Backward => {
                scan(0..start).rev().find(|m| m.end <= start).or_else(|| {
                    if options.wrap {
                        scan(0..self.text().len()).next_back()
                    } else {
                        None
                    }
                })
            }
        };
        Ok(found)
    }

    /// FindReplaceDlg's ordinary and FirstIncremental search origins. Backward
    /// ordinary search starts one byte before selection maximum, rounded out of
    /// a UTF-8 character by Scintilla, rather than at selection minimum.
    pub fn search_start(&self, direction: SearchDirection, include_selection: bool) -> usize {
        let range = self.selection().range();
        match (direction, include_selection) {
            (SearchDirection::Forward, false) | (SearchDirection::Backward, true) => range.end,
            (SearchDirection::Forward, true) => range.start,
            (SearchDirection::Backward, false) => self.text()[..range.end]
                .char_indices()
                .next_back()
                .map_or(0, |(offset, _)| offset),
        }
    }

    /// Bookmark all lines touched by complete, non-overlapping matches. A match
    /// ending at a line start does not mark that following line. Existing marks,
    /// selection and text history are preserved (Mark All without purge).
    pub fn bookmark_matches_in(
        &mut self,
        needle: &str,
        options: SearchOptions,
        scope: Range<usize>,
    ) -> Result<usize, EditError> {
        self.validate(&scope)?;
        if options.mode == SearchMode::Regex {
            let matches = crate::regex_search::matches(self.text(), needle, options, scope)?;
            let mut lines = std::collections::BTreeSet::new();
            for range in &matches {
                let starts = self.line_starts();
                let first = starts
                    .partition_point(|&p| p <= range.start)
                    .saturating_sub(1);
                let last = starts
                    .partition_point(|&p| p < range.end)
                    .saturating_sub(1)
                    .max(first);
                lines.extend(first..=last);
            }
            self.bookmarks.0.extend(lines);
            return Ok(matches.len());
        }
        let needle = prepare_needle(needle, options)?;
        let limit = scope.end;
        let mut previous_end = 0;
        let mut count = 0;
        let mut lines = std::collections::BTreeSet::new();
        let starts = self.line_starts();
        for found in match_ranges(self.text(), &needle, options, scope) {
            if found.start < previous_end || found.end > limit {
                continue;
            }
            previous_end = found.end;
            let first = starts.partition_point(|&p| p <= found.start) - 1;
            let last = starts.partition_point(|&p| p < found.end) - 1;
            lines.extend(first..=last);
            count += 1;
        }
        self.bookmarks.0.extend(lines);
        Ok(count)
    }

    /// Count non-overlapping occurrences, matching Replace All's traversal.
    /// Direction and wrap do not change a whole-document count.
    pub fn count_matches(&self, needle: &str, options: SearchOptions) -> Result<usize, EditError> {
        self.count_matches_in(needle, options, 0..self.text().len())
    }

    /// Only complete matches inside the supplied UTF-8 range count. Word
    /// boundaries still use the surrounding document, not an isolated slice.
    pub fn count_matches_in(
        &self,
        needle: &str,
        options: SearchOptions,
        scope: Range<usize>,
    ) -> Result<usize, EditError> {
        self.validate(&scope)?;
        if options.mode == SearchMode::Regex {
            return Ok(crate::regex_search::matches(self.text(), needle, options, scope)?.len());
        }
        let needle = prepare_needle(needle, options)?;
        let limit = scope.end;
        let mut end = 0;
        Ok(match_ranges(self.text(), &needle, options, scope)
            .filter(|found| {
                if found.start < end || found.end > limit {
                    return false;
                }
                end = found.end;
                true
            })
            .count())
    }

    /// Replace an already matched search target with decoded replacement text.
    /// Like processReplace, retain identical replacements in history and place
    /// the caret after the inserted text before the caller searches again.
    pub fn replace_search_target(
        &mut self,
        range: Range<usize>,
        replacement: &str,
    ) -> Result<(), EditError> {
        let start = range.start;
        self.apply_search_replacements(&[(range, replacement.to_owned())])?;
        let caret = start + replacement.len();
        self.select(crate::Selection {
            anchor: caret,
            caret,
        })
    }

    /// Replacements never rescan inserted text and are one undo transaction.
    pub fn replace_matches(
        &mut self,
        needle: &str,
        replacement: &str,
        options: SearchOptions,
    ) -> Result<usize, EditError> {
        self.replace_matches_in(needle, replacement, options, 0..self.text().len())
    }

    /// Replace complete matches inside a range and map the current selection
    /// through the edits. Text outside the scope is never changed.
    pub fn replace_matches_in(
        &mut self,
        needle: &str,
        replacement: &str,
        options: SearchOptions,
        scope: Range<usize>,
    ) -> Result<usize, EditError> {
        self.validate(&scope)?;
        if options.mode == SearchMode::Regex {
            let edits = crate::regex_search::replacements(
                self.text(),
                needle,
                replacement,
                options,
                scope,
            )?;
            let count = edits.len();
            self.apply_edits(&edits)?;
            return Ok(count);
        }
        let needle = prepare_needle(needle, options)?;
        let limit = scope.end;
        let matches = match_ranges(self.text(), &needle, options, scope);
        let mut edits = Vec::new();
        let replacement = options.mode.decode(replacement)?;
        let mut end = 0;
        let mut count = 0;
        let mut selection = self.selection();
        let mut original_end = 0;
        let mut replaced_end = 0;
        for found in matches {
            if found.start < end || found.end > limit {
                continue;
            }
            end = found.end;
            // Scintilla ReplaceTarget deletes, then inserts. Deletion clamps
            // endpoints to the start; insertion moves a nonempty selection's
            // start but leaves an empty caret and the selection end in place.
            let start = replaced_end + found.start - original_end;
            selection.replace_target(start..start + found.len(), replacement.len());
            original_end = found.end;
            replaced_end = start + replacement.len();
            edits.push((found, replacement.to_string()));
            count += 1;
        }
        if !edits.is_empty() {
            self.apply_search_replacements(&edits)?;
        }
        self.select(selection)?;
        Ok(count)
    }
}
