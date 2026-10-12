#[path = "case_fold_data.rs"]
mod data;

pub(crate) fn character(ch: char, buffer: &mut [u8; 4]) -> &[u8] {
    if let Ok(index) = data::FOLDS.binary_search_by_key(&(ch as u32), |entry| entry.0) {
        data::FOLDS[index].1
    } else {
        ch.encode_utf8(buffer).as_bytes()
    }
}

pub(crate) fn text(text: &str) -> Vec<u8> {
    let mut folded = Vec::with_capacity(text.len());
    let mut buffer = [0; 4];
    for ch in text.chars() {
        folded.extend_from_slice(character(ch, &mut buffer));
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_unicode_scalar_matches_compiled_scintilla_oracle() {
        let mut expected = include_str!("../tests/fixtures/case_fold.tsv")
            .lines()
            .map(|line| {
                let (point, bytes) = line.split_once('\t').unwrap();
                let point = u32::from_str_radix(point, 16).unwrap();
                let bytes: Vec<u8> = (0..bytes.len())
                    .step_by(2)
                    .map(|index| u8::from_str_radix(&bytes[index..index + 2], 16).unwrap())
                    .collect();
                (point, bytes)
            })
            .peekable();
        let mut buffer = [0; 4];
        let mut identity = [0; 4];
        for point in 0..=0x10ffff {
            let Some(ch) = char::from_u32(point) else {
                continue;
            };
            let actual = character(ch, &mut buffer);
            if expected.peek().is_some_and(|entry| entry.0 == point) {
                assert_eq!(actual, expected.next().unwrap().1, "U+{point:04X}");
            } else {
                assert_eq!(
                    actual,
                    ch.encode_utf8(&mut identity).as_bytes(),
                    "U+{point:04X}"
                );
            }
        }
        assert!(expected.next().is_none());
    }
}
