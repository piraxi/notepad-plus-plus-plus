//! Default Scintilla WordPartLeft/Right: ASCII case groups and non-ASCII runs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Lower,
    Upper,
    Digit,
    Punctuation,
    Space,
    NonAscii,
    Other,
}

fn group(ch: char) -> Group {
    if !ch.is_ascii() {
        Group::NonAscii
    } else if ch.is_ascii_lowercase() {
        Group::Lower
    } else if ch.is_ascii_uppercase() {
        Group::Upper
    } else if ch.is_ascii_digit() {
        Group::Digit
    } else if ch.is_ascii_punctuation() {
        Group::Punctuation
    } else if matches!(ch, ' ' | '\t'..='\r') {
        Group::Space
    } else {
        Group::Other
    }
}

fn at(text: &str, pos: usize) -> char {
    text[pos..].chars().next().unwrap_or('\0')
}
fn before(text: &str, pos: usize) -> usize {
    pos - text[..pos].chars().next_back().unwrap().len_utf8()
}

pub(crate) fn boundary(text: &str, mut pos: usize, right: bool) -> usize {
    if right {
        while pos < text.len() && at(text, pos) == '_' {
            pos += 1;
        }
        if pos == text.len() {
            return pos;
        }
        let initial = group(at(text, pos));
        if initial == Group::Upper {
            if group(at(text, pos + 1)) == Group::Lower {
                pos += 1;
                while pos < text.len() && group(at(text, pos)) == Group::Lower {
                    pos += 1;
                }
            } else {
                while pos < text.len() && group(at(text, pos)) == Group::Upper {
                    pos += 1;
                }
            }
            if group(at(text, pos)) == Group::Lower
                && group(at(text, before(text, pos))) == Group::Upper
            {
                pos -= 1;
            }
        } else if initial == Group::Other {
            pos += at(text, pos).len_utf8();
        } else {
            while pos < text.len() && group(at(text, pos)) == initial {
                pos += at(text, pos).len_utf8();
            }
        }
    } else if pos > 0 {
        pos = before(text, pos);
        while pos > 0 && at(text, pos) == '_' {
            pos = before(text, pos);
        }
        if pos > 0 {
            let initial = group(at(text, pos));
            pos = before(text, pos);
            if initial == Group::Other {
                return pos + at(text, pos).len_utf8();
            }
            while pos > 0 && group(at(text, pos)) == initial {
                pos = before(text, pos);
            }
            let stopped = group(at(text, pos));
            if stopped != initial && !(initial == Group::Lower && stopped == Group::Upper) {
                pos += at(text, pos).len_utf8();
            }
        }
    }
    pos
}
