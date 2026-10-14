//! Conservative formatting: never rewrite literal/comment tokens or reorder JSON keys.
use crate::scintilla::{self, ids::*};
use npp_editor_core::IndentationStyle;
use std::{io, ops::Range};
use windows_sys::Win32::Foundation::HWND;

pub fn indent(columns: usize, width: usize, style: IndentationStyle) -> String {
    if style == IndentationStyle::Spaces {
        " ".repeat(columns)
    } else {
        format!(
            "{}{}",
            "\t".repeat(columns / width),
            " ".repeat(columns % width)
        )
    }
}
fn columns(text: &str, width: usize) -> usize {
    text.bytes()
        .take_while(|c| matches!(c, b' ' | b'\t'))
        .fold(0, |n, c| n + if c == b'\t' { width - n % width } else { 1 })
}
fn lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let (mut start, mut index) = (0, 0);
    let mut lines = Vec::new();
    while index < bytes.len() {
        if matches!(bytes[index], b'\r' | b'\n') {
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            lines.push(&text[start..index + 1]);
            start = index + 1;
        }
        index += 1;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}
pub fn protected(hwnd: HWND, source: &str, command: u32) -> Vec<bool> {
    scintilla::send(hwnd, SCI_COLOURISE, 0, -1);
    let projection = crate::projection::Projection::new(source);
    #[repr(C)]
    struct RangeFull {
        start: isize,
        end: isize,
    }
    #[repr(C)]
    struct StyledText {
        range: RangeFull,
        text: *mut u8,
    }
    let mut bytes = vec![0; projection.text.len() * 2 + 2];
    let mut request = StyledText {
        range: RangeFull {
            start: 0,
            end: projection.text.len() as isize,
        },
        text: bytes.as_mut_ptr(),
    };
    scintilla::send(
        hwnd,
        SCI_GETSTYLEDTEXTFULL,
        0,
        &mut request as *mut _ as isize,
    );
    let mut protected_styles = [false; 256];
    if let Some(data) = crate::syntax_data::lookup(command) {
        for style in data.styles {
            protected_styles[style.id] = style.protected;
        }
    }
    let mut result = vec![false; source.len()];
    let mut units = 0;
    for (index, ch) in projection.text.char_indices() {
        let start = projection.byte_offset(units);
        units += ch.len_utf16();
        let end = projection.byte_offset(units);
        result[start..end].fill(protected_styles[bytes[index * 2 + 1] as usize]);
    }
    result
}

pub fn json(
    source: &str,
    width: usize,
    style: IndentationStyle,
    ending: &str,
) -> io::Result<String> {
    // Validate before any edit. Formatting works on original tokens rather
    // than a map, retaining key order, duplicate keys and number spelling.
    let _: serde_json::Value = serde_json::from_str(source).map_err(io::Error::other)?;
    let mut result = String::new();
    let mut depth = 0usize;
    let mut string = false;
    let mut escaped = false;
    let chars: Vec<_> = source.chars().collect();
    for (index, &ch) in chars.iter().enumerate() {
        if string {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                result.push(ch);
                string = true;
            }
            '{' | '[' => {
                result.push(ch);
                depth += 1;
                let next = chars[index + 1..].iter().find(|c| !c.is_whitespace());
                if next != Some(&if ch == '{' { '}' } else { ']' }) {
                    result.push_str(ending);
                    result.push_str(&indent(depth * width, width, style));
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                let previous = chars[..index].iter().rev().find(|c| !c.is_whitespace());
                if previous != Some(&if ch == '}' { '{' } else { '[' }) {
                    result.push_str(ending);
                    result.push_str(&indent(depth * width, width, style));
                }
                result.push(ch);
            }
            ',' => {
                result.push(',');
                result.push_str(ending);
                result.push_str(&indent(depth * width, width, style));
            }
            ':' => result.push_str(": "),
            c if c.is_whitespace() => (),
            c => result.push(c),
        }
    }
    if source.ends_with(['\r', '\n']) {
        result.push_str(ending);
    }
    Ok(result)
}

pub fn xml(
    source: &str,
    width: usize,
    style: IndentationStyle,
    ending: &str,
) -> io::Result<String> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(source);
    let mut nodes = Vec::<(Range<usize>, u8)>::new();
    let mut stack = Vec::<(usize, bool)>::new();
    let mut preserve = std::collections::HashMap::new();
    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(io::Error::other)?;
        let end = reader.buffer_position() as usize;
        let index = nodes.len();
        let kind = match event {
            Event::Eof => break,
            Event::Start(ref tag) => {
                let mut keep = stack.last().is_some_and(|(_, keep)| *keep);
                for attribute in tag.attributes() {
                    let attribute = attribute.map_err(io::Error::other)?;
                    if attribute.key.as_ref() == b"xml:space"
                        && attribute.value.as_ref() == b"preserve"
                    {
                        keep = true;
                    }
                }
                stack.push((index, keep));
                1
            }
            Event::End(_) => {
                let Some((first, keep)) = stack.pop() else {
                    return Err(io::Error::other("Unmatched XML closing tag"));
                };
                if keep {
                    preserve.insert(first, index);
                }
                2
            }
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                if !source[start..end].trim().is_empty() {
                    if let Some((_, keep)) = stack.last_mut() {
                        *keep = true;
                    }
                }
                3
            }
            _ => 0,
        };
        nodes.push((start..end, kind));
    }
    if !stack.is_empty() {
        return Err(io::Error::other("Unclosed XML element"));
    }
    let (mut depth, mut index) = (0usize, 0usize);
    let mut result = String::new();
    while index < nodes.len() {
        let (range, kind) = &nodes[index];
        if *kind == 3 && source[range.clone()].trim().is_empty() {
            index += 1;
            continue;
        }
        if *kind == 2 {
            depth = depth.saturating_sub(1);
        }
        if !result.is_empty() {
            result.push_str(ending);
        }
        result.push_str(&indent(depth * width, width, style));
        if let Some(&last) = preserve.get(&index) {
            result.push_str(&source[range.start..nodes[last].0.end]);
            index = last + 1;
        } else {
            result.push_str(&source[range.clone()]);
            if *kind == 1 {
                depth += 1;
            }
            index += 1;
        }
    }
    if source.ends_with(['\r', '\n']) {
        result.push_str(ending);
    }
    Ok(result)
}

pub fn reindent(
    hwnd: HWND,
    source: &str,
    command: u32,
    scope: Range<usize>,
    width: usize,
    style: IndentationStyle,
) -> String {
    let protected = protected(hwnd, source, command);
    let lexer = crate::syntax_data::lookup(command).map_or("null", |data| data.lexer);
    let braces = matches!(
        lexer,
        "cpp" | "rust" | "css" | "objc" | "json" | "phpscript"
    );
    let semantic = matches!(lexer, "python" | "nim" | "gdscript");
    let blocks = matches!(
        lexer,
        "lua" | "bash" | "sql" | "vb" | "pascal" | "ruby" | "fortran" | "f77" | "hypertext" | "xml"
    );
    let mut depth = 0usize;
    let mut stack = vec![0usize];
    let mut result = String::new();
    let mut offset = 0;
    for (line_number, raw) in lines(source).into_iter().enumerate() {
        let content = raw.trim_end_matches(['\r', '\n']);
        let leading = content.len() - content.trim_start_matches([' ', '\t']).len();
        let body = &content[leading..];
        let code: String = content
            .char_indices()
            .filter(|(i, _)| !protected[offset + i])
            .map(|(_, c)| c)
            .collect();
        let starts_inside_literal = offset > 0
            && protected.get(offset).copied().unwrap_or(false)
            && protected.get(offset - 1).copied().unwrap_or(false);
        let closing = code
            .trim_start()
            .chars()
            .take_while(|c| matches!(c, '}' | ']' | ')'))
            .count();
        let target_depth = if braces {
            depth.saturating_sub(closing)
        } else if semantic && !body.is_empty() && !starts_inside_literal {
            let existing = columns(content, width);
            while stack.len() > 1 && existing < *stack.last().unwrap() {
                stack.pop();
            }
            if existing > *stack.last().unwrap() {
                stack.push(existing);
            }
            stack.len() - 1
        } else if blocks {
            (scintilla::send(hwnd, SCI_GETFOLDLEVEL, line_number, 0) as u32
                & SC_FOLDLEVELNUMBERMASK)
                .saturating_sub(SC_FOLDLEVELBASE) as usize
        } else {
            columns(content, width) / width
        };
        if offset >= scope.start && offset < scope.end {
            if starts_inside_literal {
                result.push_str(raw);
            } else {
                if !body.is_empty() {
                    if braces || semantic || blocks {
                        result.push_str(&indent(target_depth * width, width, style));
                    } else {
                        result.push_str(&content[..leading]);
                    }
                    let trailing = content.trim_end_matches([' ', '\t']).len();
                    let preserve_trailing = trailing < content.len()
                        && protected.get(offset + trailing).copied().unwrap_or(false);
                    result.push_str(if preserve_trailing {
                        body
                    } else {
                        body.trim_end_matches([' ', '\t'])
                    });
                }
                result.push_str(&raw[content.len()..]);
            }
        }
        if braces {
            for ch in code.chars() {
                match ch {
                    '{' | '[' | '(' => depth += 1,
                    '}' | ']' | ')' => depth = depth.saturating_sub(1),
                    _ => (),
                }
            }
        }
        offset += raw.len();
    }
    result
}

pub fn newline(
    hwnd: HWND,
    source: &str,
    range: Range<usize>,
    command: u32,
    width: usize,
    style: IndentationStyle,
    ending: &str,
) -> (String, usize) {
    let start = source[..range.start]
        .rfind(['\n', '\r'])
        .map_or(0, |i| i + 1);
    let before = &source[start..range.start];
    let base = columns(before, width);
    let protected = protected(hwnd, source, command);
    let code: String = before
        .char_indices()
        .filter(|(i, _)| !protected[start + i])
        .map(|(_, c)| c)
        .collect();
    let lexer = crate::syntax_data::lookup(command).map_or("null", |d| d.lexer);
    let trimmed = code.trim_end();
    let extra = trimmed.ends_with(['{', '[', '('])
        || (matches!(lexer, "python" | "gdscript" | "nim") && trimmed.ends_with(':'))
        || (lexer == "lua"
            && [" then", " do", "function", "repeat"]
                .iter()
                .any(|s| trimmed.ends_with(s)))
        || (lexer == "bash" && ["then", "do"].iter().any(|s| trimmed.ends_with(s)));
    let mut result = format!(
        "{ending}{}",
        indent(base + usize::from(extra) * width, width, style)
    );
    let caret = range.start + result.len();
    let next = source[range.end..].chars().next();
    let paired = matches!(
        (trimmed.chars().last(), next),
        (Some('{'), Some('}')) | (Some('['), Some(']')) | (Some('('), Some(')'))
    );
    if extra && paired {
        result.push_str(ending);
        result.push_str(&indent(base, width, style));
    }
    (result, caret)
}
