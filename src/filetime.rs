//! Converts a Windows `FILETIME` (100ns ticks since 1601-01-01 UTC — the
//! on-disk representation of `Image Info`'s capture-date property) into an
//! EXIF-formatted `"YYYY:MM:DD HH:MM:SS"` string.
//!
//! Deliberately hand-rolled instead of pulling in a date/time crate: this
//! is the one place fpx-convert needs calendar math, and the algorithm
//! (Howard Hinnant's `civil_from_days`) is a well-known, compact,
//! dependency-free way to turn a day count into a proleptic-Gregorian
//! year/month/day.

const TICKS_PER_SECOND: u64 = 10_000_000;
const SECONDS_1601_TO_1970: i64 = 11_644_473_600;

/// Converts days since the Unix epoch (1970-01-01) into a
/// (year, month, day) civil date. <http://howardhinnant.github.io/date_algorithms.html>
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Formats a FILETIME value as `"YYYY:MM:DD HH:MM:SS"`. Returns `None` if
/// the value doesn't correspond to a representable calendar date (e.g. 0,
/// which some tools use as an "unset" sentinel).
pub fn format_exif_datetime(filetime: u64) -> Option<String> {
    if filetime == 0 {
        return None;
    }
    let total_seconds = (filetime / TICKS_PER_SECOND) as i64 - SECONDS_1601_TO_1970;
    if total_seconds < 0 {
        return None;
    }
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    if year < 1 {
        return None;
    }
    let hour = seconds_of_day / 3600;
    let minute = (seconds_of_day % 3600) / 60;
    let second = seconds_of_day % 60;
    Some(format!(
        "{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sample_file_timestamp() {
        // Confirmed against a real sample file: 1997-12-25 15:29:39 UTC.
        let filetime: u64 = 0x01BD_1149_E9DB_7380;
        assert_eq!(
            format_exif_datetime(filetime).as_deref(),
            Some("1997:12:25 15:29:39")
        );
    }

    #[test]
    fn unset_filetime_is_none() {
        assert_eq!(format_exif_datetime(0), None);
    }
}
