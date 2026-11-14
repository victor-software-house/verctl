//! The HTTP date a registry gives for a retry.
//!
//! crates.io names the time of its next new-crate slot as an IMF-fixdate.
//! It appears in cargo's "try again after" message and in a `Retry-After`
//! header, which may also hold a count of seconds ([RFC 9110 section 10.2.3]).
//! [`wait`] reads both. The parser handles only the fixed form [RFC 9110
//! section 5.6.7] defines, such as `Fri, 13 Nov 2026 15:10:00 GMT`, so no date
//! crate is needed.
//!
//! [RFC 9110 section 10.2.3]: https://www.rfc-editor.org/rfc/rfc9110#section-10.2.3
//! [RFC 9110 section 5.6.7]: https://www.rfc-editor.org/rfc/rfc9110#section-5.6.7

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The day names, Monday first, as the IMF-fixdate spells them.
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// The month names, January first, as the IMF-fixdate spells them.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// 1970-01-01, the epoch, fell on a Thursday: index 3 of [`WEEKDAYS`].
const EPOCH_WEEKDAY: i64 = 3;

/// How long to wait for `value`: a count of seconds, or an IMF-fixdate. A
/// date already past is no wait. `None` when `value` is neither.
pub(crate) fn wait(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let when = parse(value)?;
    Some(when.duration_since(SystemTime::now()).unwrap_or_default())
}

/// The instant an IMF-fixdate names, or `None` when `text` is not one: the
/// weekday must name the date, the month must be a month, and the zone must
/// be `GMT`.
pub(crate) fn parse(text: &str) -> Option<SystemTime> {
    let mut parts = text.split(' ');
    let weekday = parts.next()?.strip_suffix(',')?;
    let day = digits(parts.next()?, 2)?;
    let month = parts.next()?;
    let year = digits(parts.next()?, 4)?;
    let clock = parts.next()?;
    let zone = parts.next()?;
    if parts.next().is_some() || zone != "GMT" {
        return None;
    }
    let weekday = WEEKDAYS
        .iter()
        .zip(0_i64..)
        .find_map(|(name, index)| (*name == weekday).then_some(index))?;
    let month = MONTHS
        .iter()
        .zip(1_i64..)
        .find_map(|(name, index)| (*name == month).then_some(index))?;
    if !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    let mut clock = clock.split(':');
    let hour = digits(clock.next()?, 2).filter(|hour| *hour < 24)?;
    let minute = digits(clock.next()?, 2).filter(|minute| *minute < 60)?;
    let second = digits(clock.next()?, 2).filter(|second| *second < 60)?;
    if clock.next().is_some() {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if (days + EPOCH_WEEKDAY).rem_euclid(7) != weekday {
        return None;
    }
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    UNIX_EPOCH.checked_add(Duration::from_secs(u64::try_from(seconds).ok()?))
}

/// `text` as a number of exactly `width` ASCII digits.
fn digits(text: &str, width: usize) -> Option<i64> {
    if text.len() != width || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// How many days `month` (1 to 12) of `year` has.
fn days_in_month(year: i64, month: i64) -> i64 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The days from 1970-01-01 to the given date of the proleptic Gregorian
/// calendar, negative before it. Howard Hinnant's `days_from_civil`
/// ([chrono-Compatible Low-Level Date Algorithms]), with March as the first
/// month of the shifted year.
///
/// [chrono-Compatible Low-Level Date Algorithms]: https://howardhinnant.github.io/date_algorithms.html#days_from_civil
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{parse, wait};

    fn seconds(text: &str) -> Option<u64> {
        parse(text).map(|when| {
            when.duration_since(UNIX_EPOCH)
                .expect("after the epoch")
                .as_secs()
        })
    }

    /// RFC 9110's own example, and a date crates.io gave in 2026.
    #[test]
    fn a_known_date_is_its_unix_seconds() {
        assert_eq!(seconds("Sun, 06 Nov 1994 08:49:37 GMT"), Some(784_111_777));
        assert_eq!(
            seconds("Fri, 13 Nov 2026 15:10:00 GMT"),
            Some(1_794_582_600)
        );
    }

    /// A leap day counts, and the epoch is day zero.
    #[test]
    fn leap_days_and_the_epoch() {
        assert_eq!(seconds("Thu, 01 Jan 1970 00:00:00 GMT"), Some(0));
        assert_eq!(
            seconds("Thu, 29 Feb 2024 12:00:00 GMT"),
            Some(1_709_208_000)
        );
        assert_eq!(seconds("Thu, 29 Feb 2023 12:00:00 GMT"), None);
    }

    #[test]
    fn a_bad_weekday_or_month_is_refused() {
        assert_eq!(seconds("Fry, 13 Nov 2026 15:10:00 GMT"), None);
        assert_eq!(seconds("Sat, 13 Nov 2026 15:10:00 GMT"), None);
        assert_eq!(seconds("Fri, 13 Nob 2026 15:10:00 GMT"), None);
        assert_eq!(seconds("Fri, 13 November 2026 15:10:00 GMT"), None);
    }

    #[test]
    fn a_zone_other_than_gmt_is_refused() {
        assert_eq!(seconds("Fri, 13 Nov 2026 15:10:00 UTC"), None);
        assert_eq!(seconds("Fri, 13 Nov 2026 15:10:00 +0000"), None);
        assert_eq!(seconds("Fri, 13 Nov 2026 15:10:00"), None);
    }

    #[test]
    fn another_shape_is_refused() {
        for text in [
            "",
            "Friday, 13-Nov-26 15:10:00 GMT",
            "Fri Nov 13 15:10:00 2026",
            "Fri, 13 Nov 2026 25:10:00 GMT",
            "Fri, 13 Nov 2026 15:10 GMT",
            "Fri, 13 Nov 2026 15:10:00 GMT extra",
        ] {
            assert_eq!(seconds(text), None, "{text:?}");
        }
    }

    /// Seconds wait as given, and a date in the past is no wait.
    #[test]
    fn a_retry_after_is_seconds_or_a_date() {
        assert_eq!(wait("0"), Some(Duration::ZERO));
        assert_eq!(wait(" 30 "), Some(Duration::from_secs(30)));
        assert!(wait("Fri, 13 Nov 2026 15:10:00 GMT").is_some());
        assert_eq!(wait("Sun, 06 Nov 1994 08:49:37 GMT"), Some(Duration::ZERO));
        assert_eq!(wait("soon"), None);
        assert_eq!(wait("-5"), None);
    }
}
