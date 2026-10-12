use windows_sys::Win32::UI::Controls::IsCharLowerW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CharLowerW, CharUpperW, IsCharAlphaNumericW, IsCharAlphaW,
};

#[derive(Clone, Copy)]
pub enum Mode {
    Upper,
    Lower,
    ProperForce,
    ProperBlend,
    SentenceForce,
    SentenceBlend,
    Invert,
    Random,
}

fn alpha(unit: u16) -> bool {
    // SAFETY: The API accepts a single UTF-16 code unit by value.
    unsafe { IsCharAlphaW(unit) != 0 }
}
fn alphanumeric(unit: u16) -> bool {
    // SAFETY: The API accepts a single UTF-16 code unit by value.
    unsafe { IsCharAlphaNumericW(unit) != 0 }
}
fn mapped(unit: u16, upper: bool) -> u16 {
    // SAFETY: A value with a zero high word selects the documented single
    // character form of these APIs, rather than a dereferenced string pointer.
    unsafe {
        (if upper {
            CharUpperW(unit as usize as *mut u16)
        } else {
            CharLowerW(unit as usize as *mut u16)
        }) as usize as u16
    }
}
fn space(unit: u16) -> bool {
    unsafe extern "C" {
        fn iswspace(unit: u16) -> i32;
    }
    // SAFETY: Matches the source's CRT whitespace classification by value.
    unsafe { iswspace(unit) != 0 }
}

pub fn convert(text: &str, mode: Mode) -> String {
    convert_with_random(text, mode, || {
        unsafe extern "C" {
            fn rand() -> i32;
        }
        // SAFETY: This parameterless CRT call matches the source's random-case
        // stream. Only alphabetic UTF-16 units consume a draw.
        unsafe { rand() & 1 != 0 }
    })
}

fn convert_with_random(text: &str, mode: Mode, mut random_upper: impl FnMut() -> bool) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    let force = matches!(mode, Mode::ProperForce | Mode::SentenceForce);
    let proper = matches!(mode, Mode::ProperForce | Mode::ProperBlend);
    let (mut new_sentence, mut was_cr, mut was_lf) = (true, false, false);
    for i in 0..units.len() {
        let unit = units[i];
        if matches!(mode, Mode::Upper | Mode::Lower) {
            units[i] = mapped(unit, matches!(mode, Mode::Upper));
        } else if matches!(mode, Mode::Invert) {
            // SAFETY: Character classification takes its UTF-16 unit by value.
            units[i] = mapped(unit, unsafe { IsCharLowerW(unit) != 0 });
        } else if matches!(mode, Mode::Random) {
            if alpha(unit) {
                units[i] = mapped(unit, random_upper());
            }
        } else if proper {
            if alpha(unit) {
                let apostrophe = i >= 2
                    && matches!(units[i - 1], 0x27 | 0x2018 | 0x2019)
                    && alphanumeric(units[i - 2]);
                if !apostrophe && (i == 0 || !alphanumeric(units[i - 1])) {
                    units[i] = mapped(unit, true);
                } else if force {
                    units[i] = mapped(unit, false);
                }
            }
        } else if alpha(unit) {
            if new_sentence {
                units[i] = mapped(unit, true);
                new_sentence = false;
            } else if force {
                units[i] = mapped(unit, false);
            }
            was_cr = false;
            was_lf = false;
            if units[i] == b'i' as u16
                && i > 0
                && i + 1 < units.len()
                && (space(units[i - 1]) || matches!(units[i - 1], 0x28 | 0x22))
                && (space(units[i + 1]) || units[i + 1] == 0x27)
            {
                units[i] = b'I' as u16;
            }
        } else if matches!(unit, 0x2e | 0x21 | 0x3f) {
            new_sentence = i + 1 < units.len() && !alphanumeric(units[i + 1]);
        } else if unit == 0x0d {
            if was_cr {
                new_sentence = true;
            } else {
                was_cr = true;
            }
        } else if unit == 0x0a {
            if was_lf {
                new_sentence = true;
            } else {
                was_lf = true;
            }
        }
    }
    // The source maps one UTF-16 unit at a time, leaving surrogate units intact.
    String::from_utf16(&units).expect("Windows case mapping preserves valid UTF-16")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_and_lower_preserve_source_utf16_mapping_behavior() {
        assert_eq!(convert("aßﬃé🦀𐐨\0", Mode::Upper), "AßﬃÉ🦀𐐨\0");
        assert_eq!(convert("AİÉ🦀𐐀\0", Mode::Lower), "aİé🦀𐐀\0");
        assert_eq!(convert("ΟΣ", Mode::Lower), "οσ");
    }

    #[test]
    fn invert_uses_single_windows_units_without_unicode_expansion() {
        assert_eq!(convert("aBéÉß🦀𐐀\0", Mode::Invert), "AbÉéß🦀𐐀\0");
        assert_eq!(convert("", Mode::Invert), "");
    }

    #[test]
    fn random_consumes_draws_only_for_native_alphabetic_units() {
        let mut draws = 0;
        let output = convert_with_random("a1-B🦀é\0Z", Mode::Random, || {
            draws += 1;
            draws % 2 == 1
        });
        assert_eq!(output, "A1-b🦀É\0z");
        assert_eq!(draws, 4);
        assert_eq!(
            convert_with_random("123🦀\0", Mode::Random, || panic!("no letters")),
            "123🦀\0"
        );
    }

    #[test]
    fn random_uses_the_crt_stream_across_calls() {
        // CRT random state is thread-local. Use a dedicated thread so resetting
        // it cannot affect other test commands or the host's UI thread.
        std::thread::spawn(|| {
            unsafe extern "C" {
                fn srand(seed: u32);
            }
            // SAFETY: Seeds only this disposable thread's CRT random stream.
            unsafe {
                srand(1);
            }
            assert_eq!(convert("aaa", Mode::Random), "AAa");
            assert_eq!(convert("12🦀", Mode::Random), "12🦀");
            assert_eq!(convert("aaaaa", Mode::Random), "aAaaa");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn proper_case_handles_quotes_digits_and_blended_capitals() {
        let text = "hELLO don't DON’T DON‘T 2FAST foo-BAR 🦀éCOLE";
        assert_eq!(
            convert(text, Mode::ProperForce),
            "Hello Don't Don’t Don‘t 2fast Foo-Bar 🦀École"
        );
        assert_eq!(
            convert(text, Mode::ProperBlend),
            "HELLO Don't DON’T DON‘T 2FAST Foo-BAR 🦀ÉCOLE"
        );
        assert_eq!(
            convert("'hello' ‘WORLD’", Mode::ProperForce),
            "'Hello' ‘World’"
        );
    }

    #[test]
    fn sentences_distinguish_adjacent_punctuation_and_paragraph_breaks() {
        let text = "hELLO.world! nEXT\r\nSTILL\r\n\r\nNEW?yes. END";
        assert_eq!(
            convert(text, Mode::SentenceForce),
            "Hello.world! Next\r\nstill\r\n\r\nNew?yes. End"
        );
        assert_eq!(
            convert(text, Mode::SentenceBlend),
            "HELLO.world! NEXT\r\nSTILL\r\n\r\nNEW?yes. END"
        );
        assert_eq!(
            convert("a\n \nB\rc\r\rD", Mode::SentenceForce),
            "A\n \nB\rc\r\rD"
        );
    }

    #[test]
    fn pronoun_exception_and_utf16_units_follow_source() {
        assert_eq!(
            convert("well i am (i am \"i am i'am i", Mode::SentenceForce),
            "Well I am (I am \"I am I'am i"
        );
        let text = "🦀\0éCOLE 𐐀";
        assert_eq!(convert(text, Mode::ProperForce), "🦀\0École 𐐀");
        assert_eq!(convert("", Mode::SentenceForce), "");
    }
}
