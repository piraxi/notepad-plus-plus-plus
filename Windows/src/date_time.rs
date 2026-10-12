use std::{io, ptr::null};
use windows_sys::Win32::{
    Foundation::SYSTEMTIME, Globalization::*, System::SystemInformation::GetLocalTime,
};

pub fn current(long: bool, date_first: bool) -> io::Result<String> {
    let mut now = SYSTEMTIME::default();
    // SAFETY: GetLocalTime initializes this valid SYSTEMTIME on the stack.
    unsafe {
        GetLocalTime(&mut now);
    }
    format_at(&now, long, date_first)
}

pub const DEFAULT_CUSTOM_FORMAT: &str = "yyyy-MM-dd HH:mm:ss";

pub fn current_custom(format: &str) -> String {
    let mut now = SYSTEMTIME::default();
    // SAFETY: Valid output time storage.
    unsafe {
        GetLocalTime(&mut now);
    }
    custom_at(&now, format)
}

/// Common.cpp formats time first, then feeds that result to date formatting.
/// Escape every 't' first so AM/PM's M cannot be interpreted as a month token.
pub fn custom_at(time: &SYSTEMTIME, format: &str) -> String {
    let has_midday = format.contains('t');
    let escaped: Vec<u16> = format
        .encode_utf16()
        .map(|u| if u == u16::from(b't') { 1 } else { u })
        .chain(Some(0))
        .collect();
    let mut clock = [0u16; 260];
    let mut date = [0u16; 260];
    // SAFETY: Separate initialized buffers avoid aliasing input and output;
    // formats are terminated and all storage outlives the synchronous calls.
    let length = unsafe {
        if GetTimeFormatEx(
            null(),
            0,
            time,
            escaped.as_ptr(),
            clock.as_mut_ptr(),
            clock.len() as i32,
        ) == 0
        {
            return String::new();
        }
        GetDateFormatEx(
            null(),
            0,
            time,
            clock.as_ptr(),
            date.as_mut_ptr(),
            date.len() as i32,
            null(),
        )
    };
    if length == 0 {
        return String::new();
    }
    let formatted = &date[..length as usize - 1];
    if !has_midday {
        return String::from_utf16_lossy(formatted);
    }
    let mut midday = [0u16; 260];
    // SAFETY: A literal terminated tt format and a valid output buffer.
    let length = unsafe {
        GetTimeFormatEx(
            null(),
            0,
            time,
            windows_sys::core::w!("tt"),
            midday.as_mut_ptr(),
            midday.len() as i32,
        )
    };
    let midday = &midday[..length.saturating_sub(1) as usize];
    String::from_utf16_lossy(&restore_midday(formatted, midday))
}

fn restore_midday(formatted: &[u16], midday: &[u16]) -> Vec<u16> {
    let mut result = Vec::with_capacity(formatted.len());
    let mut index = 0;
    while index < formatted.len() {
        if formatted[index] != 1 {
            result.push(formatted[index]);
        } else if !midday.is_empty() {
            if formatted.get(index + 1) == Some(&1) {
                result.extend_from_slice(midday);
                index += 1;
            } else {
                result.push(midday[0]);
            }
        }
        index += 1;
    }
    result
}

pub fn format_at(time: &SYSTEMTIME, long: bool, date_first: bool) -> io::Result<String> {
    let mut date = [0u16; 128];
    let mut clock = [0u16; 128];
    // SAFETY: Fixed output buffers and input time remain live during both calls.
    // Null locale selects the user's default, matching NppCommands.cpp.
    let (date_len, time_len) = unsafe {
        let date_len = GetDateFormatEx(
            null(),
            if long { DATE_LONGDATE } else { DATE_SHORTDATE },
            time,
            null(),
            date.as_mut_ptr(),
            date.len() as i32,
            null(),
        );
        if date_len == 0 {
            return Err(io::Error::last_os_error());
        }
        let time_len = GetTimeFormatEx(
            null(),
            TIME_NOSECONDS,
            time,
            null(),
            clock.as_mut_ptr(),
            clock.len() as i32,
        );
        if time_len == 0 {
            return Err(io::Error::last_os_error());
        }
        (date_len, time_len)
    };
    let date = String::from_utf16_lossy(&date[..date_len as usize - 1]);
    let clock = String::from_utf16_lossy(&clock[..time_len as usize - 1]);
    Ok(if date_first {
        format!("{date} {clock}")
    } else {
        format!("{clock} {date}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_default_formats_date_and_seconds_and_keeps_failure_semantics() {
        let time = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDay: 12,
            wDayOfWeek: 1,
            wHour: 13,
            wMinute: 4,
            wSecond: 5,
            wMilliseconds: 0,
        };
        assert_eq!(
            custom_at(&time, DEFAULT_CUSTOM_FORMAT),
            "2026-10-12 13:04:05"
        );
        assert_eq!(
            custom_at(&time, "HH:mm:ss yyyy/MM/dd"),
            "13:04:05 2026/10/12"
        );
        assert_eq!(custom_at(&time, &"x".repeat(300)), "");
        assert_eq!(custom_at(&time, ""), "");
        let mut midday = [0u16; 260];
        // SAFETY: Fixed valid time, literal format and initialized output buffer.
        let length = unsafe {
            GetTimeFormatEx(
                null(),
                0,
                &time,
                windows_sys::core::w!("tt"),
                midday.as_mut_ptr(),
                midday.len() as i32,
            )
        };
        assert!(length > 0);
        let midday = &midday[..length as usize - 1];
        let short = midday
            .first()
            .map_or(String::new(), |u| String::from_utf16_lossy(&[*u]));
        assert_eq!(
            custom_at(&time, "hh:mm tt t"),
            format!("01:04 {} {short}", String::from_utf16_lossy(midday))
        );
    }

    #[test]
    fn midday_restoration_handles_pairs_singles_and_empty_locale_designators() {
        let input = [1, 1, 32, 1, 32, 1, 1, 1];
        assert_eq!(
            String::from_utf16_lossy(&restore_midday(&input, &[80, 77])),
            "PM P PMP"
        );
        assert_eq!(String::from_utf16_lossy(&restore_midday(&input, &[])), "  ");
    }

    #[test]
    fn date_formats_ignore_seconds_and_reject_invalid_dates() {
        let mut time = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDay: 12,
            wDayOfWeek: 1,
            wHour: 13,
            wMinute: 4,
            wSecond: 5,
            wMilliseconds: 0,
        };
        for long in [false, true] {
            let before = format_at(&time, long, false).unwrap();
            time.wSecond = 59;
            assert_eq!(format_at(&time, long, false).unwrap(), before);
            let reversed = format_at(&time, long, true).unwrap();
            assert_eq!(before.len(), reversed.len());
            assert!(!before.contains('\0'));
        }
        time.wMonth = 13;
        assert!(format_at(&time, false, false).is_err());
    }
}
