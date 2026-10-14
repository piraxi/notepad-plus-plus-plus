use npp_editor_core::{Document, Selection};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Clone, Serialize, Deserialize)]
pub enum Step {
    Command(u32),
    Edit {
        offset: isize,
        remove: usize,
        insert: String,
    },
}

pub fn difference(before: &str, after: &str, caret: usize) -> Option<Step> {
    if before == after {
        return None;
    }
    let prefix: usize = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let suffix: usize = before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    Some(Step::Edit {
        offset: before[..prefix].chars().count() as isize
            - before[..caret].chars().count() as isize,
        remove: before[prefix..before.len() - suffix].chars().count(),
        insert: after[prefix..after.len() - suffix].into(),
    })
}

pub fn apply(doc: &mut Document, offset: isize, remove: usize, insert: &str) -> io::Result<()> {
    let chars: Vec<_> = doc
        .text()
        .char_indices()
        .map(|(i, _)| i)
        .chain(Some(doc.text().len()))
        .collect();
    let caret = doc.text()[..doc.selection().caret].chars().count() as isize;
    let start = caret
        .checked_add(offset)
        .filter(|&n| n >= 0)
        .map(|n| n as usize)
        .ok_or_else(|| io::Error::other("Macro moved before the start of the document"))?;
    let end = start
        .checked_add(remove)
        .filter(|&n| n < chars.len())
        .ok_or_else(|| io::Error::other("Macro moved beyond the end of the document"))?;
    let start = chars[start];
    let end = chars[end];
    doc.replace(start..end, insert).map_err(io::Error::other)?;
    doc.select(Selection {
        anchor: start + insert.len(),
        caret: start + insert.len(),
    })
    .map_err(io::Error::other)
}
