/// Sorters.h first truncates at the first inadmissible character, then uses
/// decimal-prefix conversion. Thus text-only lines are treated as empty keys.
pub(crate) fn key(line: &str, comma: bool) -> Result<Option<f64>, ()> {
    let separator = if comma { b',' } else { b'.' };
    let end = line
        .bytes()
        .take_while(|b| {
            b.is_ascii_digit()
                || matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'-')
                || *b == separator
        })
        .count();
    let prepared = line[..end].trim_start_matches([' ', '\t', '\r', '\n']);
    if prepared.is_empty() {
        return Ok(None);
    }
    let bytes = prepared.as_bytes();
    let mut pos = usize::from(bytes[0] == b'-');
    let mut digits = 0;
    while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
        pos += 1;
        digits += 1;
    }
    if bytes.get(pos) == Some(&separator) {
        pos += 1;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Err(());
    }
    let token = prepared[..pos].replace(',', ".");
    let number: f64 = token.parse().map_err(|_| ())?;
    let nonzero = token.bytes().any(|b| (b'1'..=b'9').contains(&b));
    // The Windows CRT accepts representable subnormals. ERANGE applies when a
    // nonzero decimal underflows to zero, or when it overflows to infinity.
    if !number.is_finite() || (nonzero && number == 0.0) {
        return Err(());
    }
    Ok(Some(number))
}
