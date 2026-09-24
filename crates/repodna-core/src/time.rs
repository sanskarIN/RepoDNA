//! Calendar-aware timestamps used throughout the RepositoryDNA model.
//!
//! RepoDNA needs only a small amount of date arithmetic (UTC dates, months, weekdays,
//! and RFC 3339 formatting), so it implements the proleptic Gregorian calendar directly
//! instead of depending on a full date-time library. The civil-date conversions follow
//! Howard Hinnant's well-known `days_from_civil` / `civil_from_days` algorithms, which
//! are exact for every representable year.

use std::borrow::Cow;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Number of seconds in one day.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// A point in time with one-second precision, stored as seconds since the Unix epoch (UTC).
///
/// Serialized as an RFC 3339 UTC string such as `2026-09-24T10:30:00Z`, so artifacts stay
/// readable by people and by any JSON consumer. Deserialization also accepts RFC 3339
/// strings with numeric offsets and plain integers (Unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(i64);

/// A calendar date and time of day in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CivilDateTime {
    /// Year, e.g. `2026`.
    pub year: i64,
    /// Month of the year, `1..=12`.
    pub month: u32,
    /// Day of the month, `1..=31`.
    pub day: u32,
    /// Hour of the day, `0..=23`.
    pub hour: u32,
    /// Minute of the hour, `0..=59`.
    pub minute: u32,
    /// Second of the minute, `0..=59`.
    pub second: u32,
}

/// Error returned when a timestamp string cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid timestamp {input:?}: expected an RFC 3339 date-time such as 2026-09-24T10:30:00Z")]
pub struct TimestampParseError {
    input: String,
}

impl Timestamp {
    /// The Unix epoch, `1970-01-01T00:00:00Z`.
    pub const UNIX_EPOCH: Timestamp = Timestamp(0);

    /// Creates a timestamp from seconds since the Unix epoch.
    pub const fn from_unix(seconds: i64) -> Self {
        Self(seconds)
    }

    /// Returns the number of seconds since the Unix epoch.
    pub const fn unix(self) -> i64 {
        self.0
    }

    /// Returns the current system time.
    pub fn now() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(elapsed) => Self(i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)),
            Err(before) => Self(-i64::try_from(before.duration().as_secs()).unwrap_or(i64::MAX)),
        }
    }

    /// Returns `SOURCE_DATE_EPOCH` when it is set to a valid integer, otherwise the current time.
    ///
    /// Honoring `SOURCE_DATE_EPOCH` (the reproducible-builds convention) lets users produce
    /// byte-identical artifacts for the same repository revision.
    pub fn now_or_source_date_epoch() -> Self {
        std::env::var("SOURCE_DATE_EPOCH")
            .ok()
            .and_then(|value| value.trim().parse::<i64>().ok())
            .map(Self)
            .unwrap_or_else(Self::now)
    }

    /// Creates a timestamp from a UTC calendar date and time.
    ///
    /// Returns `None` when a component is out of range (for example February 30).
    pub fn from_civil(civil: CivilDateTime) -> Option<Self> {
        if !(1..=12).contains(&civil.month)
            || civil.day == 0
            || civil.day > days_in_month(civil.year, civil.month)
            || civil.hour > 23
            || civil.minute > 59
            || civil.second > 59
        {
            return None;
        }
        let days = days_from_civil(civil.year, civil.month, civil.day);
        let seconds =
            i64::from(civil.hour) * 3600 + i64::from(civil.minute) * 60 + i64::from(civil.second);
        days.checked_mul(SECONDS_PER_DAY)?
            .checked_add(seconds)
            .map(Self)
    }

    /// Creates a timestamp for midnight UTC at the start of the given date.
    pub fn from_ymd(year: i64, month: u32, day: u32) -> Option<Self> {
        Self::from_civil(CivilDateTime {
            year,
            month,
            day,
            hour: 0,
            minute: 0,
            second: 0,
        })
    }

    /// Converts the timestamp to a UTC calendar date and time.
    pub fn civil(self) -> CivilDateTime {
        let days = self.0.div_euclid(SECONDS_PER_DAY);
        let seconds_of_day = self.0.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        // `seconds_of_day` is in 0..86_400, so the narrowing conversions cannot fail.
        let seconds_of_day = u32::try_from(seconds_of_day).unwrap_or(0);
        CivilDateTime {
            year,
            month,
            day,
            hour: seconds_of_day / 3600,
            minute: (seconds_of_day % 3600) / 60,
            second: seconds_of_day % 60,
        }
    }

    /// Formats the timestamp as an RFC 3339 UTC string, e.g. `2026-09-24T10:30:00Z`.
    pub fn to_rfc3339(self) -> String {
        let c = self.civil();
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            c.year, c.month, c.day, c.hour, c.minute, c.second
        )
    }

    /// Formats the UTC date as `YYYY-MM-DD`.
    pub fn date_string(self) -> String {
        let c = self.civil();
        format!("{:04}-{:02}-{:02}", c.year, c.month, c.day)
    }

    /// Formats the UTC month as `YYYY-MM`, the key used for monthly timeline buckets.
    pub fn month_key(self) -> String {
        let c = self.civil();
        format!("{:04}-{:02}", c.year, c.month)
    }

    /// Returns the UTC calendar year.
    pub fn year(self) -> i64 {
        self.civil().year
    }

    /// Returns the day of the week in UTC, where `0` is Monday and `6` is Sunday.
    pub fn weekday(self) -> u32 {
        let days = self.0.div_euclid(SECONDS_PER_DAY);
        // 1970-01-01 was a Thursday (index 3 when Monday is 0).
        u32::try_from((days + 3).rem_euclid(7)).unwrap_or(0)
    }

    /// Returns the hour of the day in UTC (`0..=23`).
    pub fn hour(self) -> u32 {
        self.civil().hour
    }

    /// Returns a timestamp shifted by `seconds`, saturating at the representable range.
    pub fn plus_seconds(self, seconds: i64) -> Self {
        Self(self.0.saturating_add(seconds))
    }

    /// Returns a timestamp shifted by a whole number of days.
    pub fn plus_days(self, days: i64) -> Self {
        self.plus_seconds(days.saturating_mul(SECONDS_PER_DAY))
    }

    /// Returns the number of whole days from `self` until `later` (negative if `later` is earlier).
    pub fn days_until(self, later: Timestamp) -> i64 {
        (later.0 - self.0).div_euclid(SECONDS_PER_DAY)
    }

    /// Returns midnight UTC on the first day of this timestamp's month.
    pub fn start_of_month(self) -> Self {
        let c = self.civil();
        Self(days_from_civil(c.year, c.month, 1) * SECONDS_PER_DAY)
    }

    /// Returns midnight UTC on the first day of the month `months` after this timestamp's month.
    pub fn plus_months(self, months: i64) -> Self {
        let c = self.civil();
        let month_index = c.year * 12 + i64::from(c.month) - 1 + months;
        let year = month_index.div_euclid(12);
        let month = u32::try_from(month_index.rem_euclid(12) + 1).unwrap_or(1);
        Self(days_from_civil(year, month, 1) * SECONDS_PER_DAY)
    }

    /// Parses an RFC 3339 date-time such as `2026-09-24T10:30:00Z` or `2026-09-24T16:00:00+05:30`.
    ///
    /// A space may be used instead of `T`, fractional seconds are ignored, and a bare date
    /// (`2026-09-24`) is interpreted as midnight UTC.
    pub fn parse_rfc3339(input: &str) -> Result<Self, TimestampParseError> {
        parse_rfc3339(input.trim()).ok_or_else(|| TimestampParseError {
            input: input.to_owned(),
        })
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TimestampVisitor;

        impl Visitor<'_> for TimestampVisitor {
            type Value = Timestamp;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an RFC 3339 date-time string or Unix seconds")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Timestamp, E> {
                Timestamp::parse_rfc3339(value).map_err(E::custom)
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Timestamp, E> {
                Ok(Timestamp(value))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Timestamp, E> {
                i64::try_from(value)
                    .map(Timestamp)
                    .map_err(|_| E::custom("timestamp is out of range"))
            }
        }

        deserializer.deserialize_any(TimestampVisitor)
    }
}

impl JsonSchema for Timestamp {
    fn schema_name() -> Cow<'static, str> {
        "Timestamp".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Timestamp").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "date-time",
            "description": "An RFC 3339 date-time in UTC, e.g. 2026-09-24T10:30:00Z."
        })
    }
}

/// Returns `true` for leap years in the proleptic Gregorian calendar.
pub fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Returns the number of days in `month` (1–12) of `year`, or `0` for an invalid month.
pub fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 for a civil date (Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = i64::from((month + 9) % 12);
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Civil date for a number of days since 1970-01-01 (Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * month_from_march + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    })
    .unwrap_or(1);
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn parse_digits(input: &str) -> Option<u32> {
    if input.is_empty() || !input.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    input.parse().ok()
}

fn parse_rfc3339(input: &str) -> Option<Timestamp> {
    let bytes = input.as_bytes();
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = i64::from(parse_digits(input.get(0..4)?)?);
    let month = parse_digits(input.get(5..7)?)?;
    let day = parse_digits(input.get(8..10)?)?;
    if bytes.len() == 10 {
        return Timestamp::from_ymd(year, month, day);
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') || bytes.len() < 19 {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let hour = parse_digits(input.get(11..13)?)?;
    let minute = parse_digits(input.get(14..16)?)?;
    let second = parse_digits(input.get(17..19)?)?;

    let mut rest = input.get(19..)?;
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    let offset_seconds = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let offset = &rest[1..];
            let (hours, minutes) = match offset.len() {
                5 if offset.as_bytes()[2] == b':' => (offset.get(0..2)?, offset.get(3..5)?),
                4 => (offset.get(0..2)?, offset.get(2..4)?),
                _ => return None,
            };
            let hours = i64::from(parse_digits(hours)?);
            let minutes = i64::from(parse_digits(minutes)?);
            if hours > 23 || minutes > 59 {
                return None;
            }
            sign * (hours * 3600 + minutes * 60)
        }
    };
    let local = Timestamp::from_civil(CivilDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })?;
    Some(local.plus_seconds(-offset_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch_and_known_dates() {
        assert_eq!(Timestamp::UNIX_EPOCH.to_rfc3339(), "1970-01-01T00:00:00Z");
        // 2000-02-29 is a leap day in a century leap year.
        let leap = Timestamp::from_ymd(2000, 2, 29).unwrap();
        assert_eq!(leap.unix(), 951_782_400);
        assert_eq!(leap.to_rfc3339(), "2000-02-29T00:00:00Z");
        assert_eq!(
            Timestamp::from_unix(1_790_245_800).to_rfc3339(),
            "2026-09-24T10:30:00Z"
        );
    }

    #[test]
    fn round_trips_every_day_across_four_centuries() {
        let start = Timestamp::from_ymd(1900, 1, 1).unwrap();
        let mut previous = start;
        for offset in 1..(400 * 366) {
            let current = start.plus_days(offset);
            let civil = current.civil();
            assert_eq!(Timestamp::from_civil(civil), Some(current));
            assert_eq!(previous.days_until(current), 1);
            previous = current;
        }
    }

    #[test]
    fn rejects_invalid_calendar_dates() {
        assert_eq!(Timestamp::from_ymd(2023, 2, 29), None);
        assert_eq!(Timestamp::from_ymd(2024, 13, 1), None);
        assert_eq!(Timestamp::from_ymd(2024, 4, 31), None);
        assert!(Timestamp::from_ymd(2024, 2, 29).is_some());
    }

    #[test]
    fn computes_weekdays() {
        // 1970-01-01 was a Thursday, 2026-09-24 is a Thursday, 2024-03-04 a Monday.
        assert_eq!(Timestamp::UNIX_EPOCH.weekday(), 3);
        assert_eq!(Timestamp::from_ymd(2026, 9, 24).unwrap().weekday(), 3);
        assert_eq!(Timestamp::from_ymd(2024, 3, 4).unwrap().weekday(), 0);
        // Dates before the epoch use floor division.
        assert_eq!(Timestamp::from_ymd(1969, 12, 31).unwrap().weekday(), 2);
    }

    #[test]
    fn parses_rfc3339_variants() {
        let expected = Timestamp::from_unix(1_790_245_800);
        for input in [
            "2026-09-24T10:30:00Z",
            "2026-09-24t10:30:00z",
            "2026-09-24 10:30:00Z",
            "2026-09-24T10:30:00.123456Z",
            "2026-09-24T16:00:00+05:30",
            "2026-09-24T16:00:00+0530",
            "2026-09-24T05:30:00-05:00",
        ] {
            assert_eq!(Timestamp::parse_rfc3339(input), Ok(expected), "{input}");
        }
        assert_eq!(
            Timestamp::parse_rfc3339("2026-09-24"),
            Ok(Timestamp::from_ymd(2026, 9, 24).unwrap())
        );
        for invalid in [
            "",
            "2026",
            "2026-9-24",
            "2026-09-24T10:30",
            "2026-09-24T10:30:00+5",
            "yesterday",
        ] {
            assert!(Timestamp::parse_rfc3339(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn month_arithmetic_handles_year_boundaries() {
        let november = Timestamp::parse_rfc3339("2025-11-17T08:00:00Z").unwrap();
        assert_eq!(
            november.start_of_month().to_rfc3339(),
            "2025-11-01T00:00:00Z"
        );
        assert_eq!(november.plus_months(2).to_rfc3339(), "2026-01-01T00:00:00Z");
        assert_eq!(
            november.plus_months(-11).to_rfc3339(),
            "2024-12-01T00:00:00Z"
        );
        assert_eq!(november.month_key(), "2025-11");
        assert_eq!(november.date_string(), "2025-11-17");
    }

    #[test]
    fn serializes_as_rfc3339_and_accepts_integers() {
        let value = Timestamp::from_unix(86_400);
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, "\"1970-01-02T00:00:00Z\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), value);
        assert_eq!(serde_json::from_str::<Timestamp>("86400").unwrap(), value);
        assert!(serde_json::from_str::<Timestamp>("\"not a date\"").is_err());
    }

    #[test]
    fn leap_year_rules() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2000));
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2024, 0), 0);
    }
}
