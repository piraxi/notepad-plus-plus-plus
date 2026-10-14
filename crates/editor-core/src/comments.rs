use crate::{Document, EditError};

impl Document {
    /// Comment complete selected lines, excluding an unselected final line.
    /// `None` toggles, `Some(true)` inserts, `Some(false)` removes.
    pub fn comment_lines(&mut self, marker: &str, mode: Option<bool>) -> Result<(), EditError> {
        let selection = self.selection().range();
        let starts = self.line_starts();
        let first = starts
            .partition_point(|&p| p <= selection.start)
            .saturating_sub(1);
        let last = starts
            .partition_point(|&p| p < selection.end.max(selection.start + 1))
            .saturating_sub(1);
        let positions: Vec<_> = (first..=last)
            .filter_map(|line| {
                let range = self.line_range(line)?;
                let content = &self.text()[range.clone()];
                let indent = content.len() - content.trim_start_matches([' ', '\t']).len();
                let at = range.start + indent;
                (!self.text()[at..range.end]
                    .trim_matches(['\r', '\n'])
                    .is_empty())
                .then_some(at)
            })
            .collect();
        let insert = mode.unwrap_or_else(|| {
            !positions
                .iter()
                .all(|&at| self.text()[at..].starts_with(marker))
        });
        let edits: Vec<_> = positions
            .into_iter()
            .filter_map(|at| {
                if insert {
                    Some((at..at, format!("{marker} ")))
                } else if self.text()[at..].starts_with(marker) {
                    let end = at + marker.len();
                    Some((
                        at..end + usize::from(self.text()[end..].starts_with(' ')),
                        String::new(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        self.apply_edits(&edits)
    }

    pub fn comment_selection(
        &mut self,
        open: &str,
        close: &str,
        remove: bool,
    ) -> Result<(), EditError> {
        let range = self.selection().range();
        let selected = &self.text()[range.clone()];
        let replacement = if remove {
            let trimmed = selected.trim();
            let Some(inner) = trimmed
                .strip_prefix(open)
                .and_then(|s| s.strip_suffix(close))
            else {
                return Ok(());
            };
            inner.to_owned()
        } else {
            format!("{open}{selected}{close}")
        };
        self.replace(range, &replacement)
    }
}
