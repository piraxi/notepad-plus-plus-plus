use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Utf16LeNoBom,
    Utf16BeNoBom,
    Windows1252,
    Windows1251,
    ShiftJis,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    #[default]
    CrLf,
    Lf,
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CrLf => "\r\n",
            Self::Lf => "\n",
            Self::Cr => "\r",
        }
    }

    /// Choose the most frequent ending, breaking ties by first occurrence.
    pub fn detect(text: &str) -> Self {
        let mut counts = [0usize; 3];
        let mut first = [usize::MAX; 3];
        let mut iter = text.bytes().enumerate().peekable();
        while let Some((offset, byte)) = iter.next() {
            let kind = match byte {
                b'\r' if iter.peek().is_some_and(|(_, b)| *b == b'\n') => {
                    iter.next();
                    0
                }
                b'\n' => 1,
                b'\r' => 2,
                _ => continue,
            };
            counts[kind] += 1;
            first[kind] = first[kind].min(offset);
        }
        let best = (0..3)
            .max_by_key(|&i| (counts[i], std::cmp::Reverse(first[i]), std::cmp::Reverse(i)))
            .unwrap();
        [Self::CrLf, Self::Lf, Self::Cr][best]
    }

    pub fn convert(self, text: &str) -> String {
        let mut output = String::with_capacity(text.len());
        let mut iter = text.chars().peekable();
        while let Some(ch) = iter.next() {
            match ch {
                '\r' => {
                    if iter.peek() == Some(&'\n') {
                        iter.next();
                    }
                    output.push_str(self.as_str());
                }
                '\n' => output.push_str(self.as_str()),
                _ => output.push(ch),
            }
        }
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for DecodeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeError(pub Encoding);
impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Text contains characters that cannot be represented in {:?}; choose a Unicode encoding",
            self.0
        )
    }
}
impl Error for EncodeError {}

impl Encoding {
    /// Formats whose interpretation must be retained for reload and reopen.
    /// These cannot be inferred reliably from a Unicode byte-order mark.
    pub fn requires_explicit_decode(self) -> bool {
        self.legacy().is_some() || matches!(self, Self::Utf16LeNoBom | Self::Utf16BeNoBom)
    }

    fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, DecodeError> {
        if bytes.len() % 2 != 0 {
            return Err(DecodeError("odd-length UTF-16 file"));
        }
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| {
                if little_endian {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|_| DecodeError("unpaired UTF-16 surrogate"))
    }
    fn legacy(self) -> Option<&'static encoding_rs::Encoding> {
        match self {
            Self::Windows1252 => Some(encoding_rs::WINDOWS_1252),
            Self::Windows1251 => Some(encoding_rs::WINDOWS_1251),
            Self::ShiftJis => Some(encoding_rs::SHIFT_JIS),
            _ => None,
        }
    }

    /// Explicit legacy interpretation does not sniff or strip BOM-like bytes.
    /// BOM-less UTF-16 is explicit: all code units, including U+FEFF, are text.
    /// Other Unicode variants require the same BOM policy as automatic decoding.
    pub fn decode_as(self, bytes: &[u8]) -> Result<String, DecodeError> {
        if matches!(self, Self::Utf16LeNoBom | Self::Utf16BeNoBom) {
            return Self::decode_utf16(bytes, self == Self::Utf16LeNoBom);
        }
        if let Some(encoding) = self.legacy() {
            return encoding
                .decode_without_bom_handling_and_without_replacement(bytes)
                .map(|text| text.into_owned())
                .ok_or(DecodeError("invalid byte sequence for selected encoding"));
        }
        let (detected, text) = Self::decode(bytes)?;
        if detected != self {
            return Err(DecodeError(
                "byte order mark does not match selected Unicode encoding",
            ));
        }
        Ok(text)
    }
    /// Detect BOMs; otherwise require valid UTF-8. Never silently replace bytes.
    pub fn decode(bytes: &[u8]) -> Result<(Self, String), DecodeError> {
        let (encoding, body) = if let Some(body) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
            (Self::Utf8Bom, body)
        } else if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe]) {
            (Self::Utf16Le, body)
        } else if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
            (Self::Utf16Be, body)
        } else {
            (Self::Utf8, bytes)
        };
        let text = match encoding {
            Self::Utf8 | Self::Utf8Bom => String::from_utf8(body.to_vec())
                .map_err(|_| DecodeError("invalid UTF-8; select an explicit legacy encoding"))?,
            Self::Utf16Le | Self::Utf16Be => Self::decode_utf16(body, encoding == Self::Utf16Le)?,
            Self::Windows1252
            | Self::Windows1251
            | Self::ShiftJis
            | Self::Utf16LeNoBom
            | Self::Utf16BeNoBom => {
                unreachable!("automatic decoding only detects Unicode")
            }
        };
        Ok((encoding, text))
    }

    pub fn encode(self, text: &str) -> Result<Vec<u8>, EncodeError> {
        if let Some(encoding) = self.legacy() {
            let (bytes, _, errors) = encoding.encode(text);
            if errors {
                return Err(EncodeError(self));
            }
            // Some legacy mappings are many-to-one. Require a lossless round trip
            // rather than silently changing characters such as yen/backslash.
            if encoding
                .decode_without_bom_handling_and_without_replacement(&bytes)
                .as_deref()
                != Some(text)
            {
                return Err(EncodeError(self));
            }
            return Ok(bytes.into_owned());
        }
        Ok(match self {
            Self::Utf8 => text.as_bytes().to_vec(),
            Self::Utf8Bom => [b"\xef\xbb\xbf".as_slice(), text.as_bytes()].concat(),
            Self::Utf16Le | Self::Utf16Be | Self::Utf16LeNoBom | Self::Utf16BeNoBom => {
                let mut output = match self {
                    Self::Utf16Le => vec![0xff, 0xfe],
                    Self::Utf16Be => vec![0xfe, 0xff],
                    _ => Vec::new(),
                };
                for unit in text.encode_utf16() {
                    let bytes = if matches!(self, Self::Utf16Le | Self::Utf16LeNoBom) {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    };
                    output.extend_from_slice(&bytes);
                }
                output
            }
            Self::Windows1252 | Self::Windows1251 | Self::ShiftJis => {
                unreachable!("handled by legacy encoder")
            }
        })
    }
}
