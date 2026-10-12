//! Default UTF-8 word boundaries from Scintilla's Document and CharClassify.
#[path = "word_classes_data.rs"]
mod data;

const WORD: u32 = 0;
const PUNCTUATION: u32 = 1;
const SPACE: u32 = 2;
const NEWLINE: u32 = 3;

fn class(ch: char) -> u32 {
    if ch.is_ascii() {
        if matches!(ch, '\r' | '\n') {
            NEWLINE
        } else if ch.is_ascii_control() || ch == ' ' {
            SPACE
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            WORD
        } else {
            PUNCTUATION
        }
    } else {
        let index = data::RANGES.partition_point(|entry| entry >> 2 <= ch as u32);
        data::RANGES[index - 1] & 3
    }
}

fn edge(inside: u32, outside: u32) -> bool {
    inside != outside && matches!(inside, WORD | PUNCTUATION)
}

pub(crate) fn word_range(text: &str, caret: usize) -> std::ops::Range<usize> {
    let mut start = caret;
    let mut end = caret;
    for ch in text[..caret]
        .chars()
        .rev()
        .take_while(|&ch| class(ch) == WORD)
    {
        start -= ch.len_utf8();
    }
    for ch in text[caret..].chars().take_while(|&ch| class(ch) == WORD) {
        end += ch.len_utf8();
    }
    start..end
}

/// Scintilla Document::NextWordStart, for a valid UTF-8 caret position.
pub(crate) fn next_word_start(text: &str, mut position: usize, rightwards: bool) -> usize {
    if rightwards {
        let mut chars = text[position..].chars().peekable();
        if let Some(&first) = chars.peek() {
            let initial = class(first);
            while chars.peek().is_some_and(|&ch| class(ch) == initial) {
                position += chars.next().unwrap().len_utf8();
            }
            while chars.peek().is_some_and(|&ch| class(ch) == SPACE) {
                position += chars.next().unwrap().len_utf8();
            }
        }
    } else {
        let mut chars = text[..position].chars().rev().peekable();
        while chars.peek().is_some_and(|&ch| class(ch) == SPACE) {
            position -= chars.next().unwrap().len_utf8();
        }
        if let Some(&first) = chars.peek() {
            let initial = class(first);
            while chars.peek().is_some_and(|&ch| class(ch) == initial) {
                position -= chars.next().unwrap().len_utf8();
            }
        }
    }
    position
}

pub(crate) fn whole_word(text: &str, start: usize, end: usize) -> bool {
    let matched = &text[start..end];
    let Some(first) = matched.chars().next() else {
        return false;
    };
    let last = matched.chars().next_back().expect("nonempty match");
    edge(
        class(first),
        text[..start].chars().next_back().map_or(SPACE, class),
    ) && edge(class(last), text[end..].chars().next().map_or(SPACE, class))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_classes_match_every_source_unicode_scalar() {
        let source =
            include_str!("../../../notepad_plus_plus/scintilla/src/CharacterCategoryMap.cxx");
        let table = source
            .split_once("const int catRanges[] = {")
            .unwrap()
            .1
            .split_once("};")
            .unwrap()
            .0;
        let ranges: Vec<u32> = table
            .lines()
            .filter_map(|line| line.trim().strip_suffix(',')?.parse().ok())
            .collect();
        let mut index = 0;
        for value in 128..=0x10ffff {
            while index + 1 < ranges.len() && ranges[index + 1] >> 5 <= value {
                index += 1;
            }
            if let Some(ch) = char::from_u32(value) {
                let expected = match ranges[index] & 31 {
                    0..=10 => WORD,
                    11..=21 => PUNCTUATION,
                    23 | 24 => NEWLINE,
                    _ => SPACE,
                };
                assert_eq!(class(ch), expected, "U+{value:04X}");
            }
        }
    }
}
