use crate::{EditError, SearchOptions};
use std::ops::Range;

fn expression(needle: &str, options: SearchOptions) -> Result<fancy_regex::Regex, EditError> {
    if needle.is_empty() {
        return Err(EditError::EmptySearch);
    }
    fancy_regex::RegexBuilder::new(needle)
        .multi_line(true)
        .case_insensitive(!options.match_case)
        .build()
        .map_err(|e| EditError::InvalidRegex(e.to_string()))
}
pub fn matches(
    text: &str,
    needle: &str,
    options: SearchOptions,
    scope: Range<usize>,
) -> Result<Vec<Range<usize>>, EditError> {
    let regex = expression(needle, options)?;
    let mut result = Vec::new();
    for found in regex.find_iter(text) {
        let found = found.map_err(|e| EditError::InvalidRegex(e.to_string()))?;
        if found.start() >= scope.start && found.end() <= scope.end {
            result.push(found.start()..found.end());
        }
    }
    Ok(result)
}
pub fn replacements(
    text: &str,
    needle: &str,
    replacement: &str,
    options: SearchOptions,
    scope: Range<usize>,
) -> Result<Vec<(Range<usize>, String)>, EditError> {
    let regex = expression(needle, options)?;
    let mut format = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            format.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => format.push('\n'),
            Some('r') => format.push('\r'),
            Some('t') => format.push('\t'),
            Some(c) if c.is_ascii_digit() => {
                format.push_str("${");
                format.push(c);
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    format.push(chars.next().unwrap());
                }
                format.push('}');
            }
            Some('\\') => format.push('\\'),
            Some(c) => {
                format.push('\\');
                format.push(c);
            }
            None => format.push('\\'),
        }
    }
    let mut edits = Vec::new();
    for captures in regex.captures_iter(text) {
        let captures = captures.map_err(|e| EditError::InvalidRegex(e.to_string()))?;
        let found = captures.get(0).unwrap();
        if found.start() < scope.start || found.end() > scope.end {
            continue;
        }
        let mut expanded = String::new();
        captures.expand(&format, &mut expanded);
        edits.push((found.start()..found.end(), expanded));
    }
    Ok(edits)
}
