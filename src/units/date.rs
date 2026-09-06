use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::sync::Arc;

use crate::error::AbacusError;
use crate::units::dimensions::Dimensions;
use crate::units::unit::Unit;
use crate::units::value::Value;

/// Enum representing days of the week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DayOfWeek {
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
    Sunday = 7,
}

impl DayOfWeek {
    #[must_use]
    pub fn from_iso_number(n: u32) -> Option<Self> {
        match n {
            1 => Some(Self::Monday),
            2 => Some(Self::Tuesday),
            3 => Some(Self::Wednesday),
            4 => Some(Self::Thursday),
            5 => Some(Self::Friday),
            6 => Some(Self::Saturday),
            7 => Some(Self::Sunday),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Monday => "Monday",
            Self::Tuesday => "Tuesday",
            Self::Wednesday => "Wednesday",
            Self::Thursday => "Thursday",
            Self::Friday => "Friday",
            Self::Saturday => "Saturday",
            Self::Sunday => "Sunday",
        }
    }
}

impl fmt::Display for DayOfWeek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Definition of weekend days for business day calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WeekendDays {
    #[default]
    SaturdaySunday,
    FridaySaturday,
    ThursdayFriday,
    SundayOnly,
}

impl WeekendDays {
    #[must_use]
    pub fn is_weekend(&self, dow: DayOfWeek) -> bool {
        match self {
            Self::SaturdaySunday => matches!(dow, DayOfWeek::Saturday | DayOfWeek::Sunday),
            Self::FridaySaturday => matches!(dow, DayOfWeek::Friday | DayOfWeek::Saturday),
            Self::ThursdayFriday => matches!(dow, DayOfWeek::Thursday | DayOfWeek::Friday),
            Self::SundayOnly => matches!(dow, DayOfWeek::Sunday),
        }
    }

    #[must_use]
    pub fn is_business_day(&self, dow: DayOfWeek) -> bool {
        !self.is_weekend(dow)
    }

    #[must_use]
    pub fn business_days_per_week(&self) -> i64 {
        match self {
            Self::SaturdaySunday | Self::FridaySaturday | Self::ThursdayFriday => 5,
            Self::SundayOnly => 6,
        }
    }
}

/// Structure representing a `TimeZone` with offset in minutes relative to UTC.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeZone {
    pub name: String,
    pub offset_minutes: i32,
}

impl TimeZone {
    pub fn new(name: impl Into<String>, offset_minutes: i32) -> Self {
        Self {
            name: name.into(),
            offset_minutes,
        }
    }

    #[must_use]
    pub fn utc() -> Self {
        Self::new("UTC", 0)
    }

    pub const SUPPORTED_TIMEZONES: &'static [(&'static str, i32)] = &[
        ("UTC", 0),
        ("GMT", 0),
        ("Z", 0),
        ("EST", -300),
        ("EDT", -240),
        ("CST", -360),
        ("CDT", -300),
        ("MST", -420),
        ("MDT", -360),
        ("PST", -480),
        ("PDT", -420),
        ("AKST", -540),
        ("AKDT", -480),
        ("HST", -600),
        ("CET", 60),
        ("BST", 60),
        ("CEST", 120),
        ("EET", 120),
        ("EEST", 180),
        ("MSK", 180),
        ("IST", 330), // +05:30
        ("JST", 540), // +09:00
        ("KST", 540),
        ("AEST", 600), // +10:00
        ("NZST", 720), // +12:00
    ];

    pub fn parse(s: &str) -> Result<Self, AbacusError> {
        let s = s.trim();
        let upper = s.to_ascii_uppercase();

        let offset = Self::SUPPORTED_TIMEZONES
            .iter()
            .find(|(name, _)| *name == upper.as_str())
            .map(|(_, off)| *off);

        if let Some(off) = offset {
            return Ok(Self::new(upper, off));
        }

        let clean = upper.strip_prefix("UTC").unwrap_or(&upper);
        if clean.starts_with('+') || clean.starts_with('-') {
            let sign = if clean.starts_with('-') { -1 } else { 1 };
            let body = &clean[1..];
            let parts: Vec<&str> = body.split(':').collect();
            let (hours, mins) = if parts.len() == 1 {
                let h = parts[0].parse::<i32>().map_err(|_| {
                    AbacusError::InvalidDate(format!("invalid timezone offset: '{s}'"))
                })?;
                (h, 0)
            } else if parts.len() == 2 {
                let h = parts[0].parse::<i32>().map_err(|_| {
                    AbacusError::InvalidDate(format!("invalid timezone offset: '{s}'"))
                })?;
                let m = parts[1].parse::<i32>().map_err(|_| {
                    AbacusError::InvalidDate(format!("invalid timezone offset: '{s}'"))
                })?;
                (h, m)
            } else {
                return Err(AbacusError::InvalidDate(format!(
                    "invalid timezone offset format: '{s}'"
                )));
            };

            let total_mins = sign * (hours * 60 + mins);
            return Ok(Self::new(s, total_mins));
        }

        Err(AbacusError::InvalidDate(format!("unknown timezone: '{s}'")))
    }

    #[must_use]
    pub fn format_offset(&self) -> String {
        let sign = if self.offset_minutes >= 0 { '+' } else { '-' };
        let abs_mins = self.offset_minutes.abs();
        let hours = abs_mins / 60;
        let mins = abs_mins % 60;
        if mins == 0 {
            format!("{sign}{hours:02}:00")
        } else {
            format!("{sign}{hours:02}:{mins:02}")
        }
    }
}

impl fmt::Display for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Structure representing a time of day with millisecond resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Time {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}

impl Time {
    #[must_use]
    pub fn new(hour: u32, minute: u32, second: u32, millisecond: u32) -> Self {
        Self {
            hour,
            minute,
            second,
            millisecond,
        }
    }

    pub fn new_12h(
        hour_12: u32,
        minute: u32,
        second: u32,
        is_pm: bool,
    ) -> Result<Self, AbacusError> {
        if !(1..=12).contains(&hour_12) {
            return Err(AbacusError::InvalidDate(format!(
                "invalid 12-hour value: {hour_12}"
            )));
        }
        let hour_24 = match (hour_12, is_pm) {
            (12, false) => 0,
            (12, true) => 12,
            (h, false) => h,
            (h, true) => h + 12,
        };
        Ok(Self::new(hour_24, minute, second, 0))
    }

    #[must_use]
    pub fn from_hms(hour: u32, minute: u32, second: u32) -> Self {
        Self::new(hour, minute, second, 0)
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.hour < 24 && self.minute < 60 && self.second < 60 && self.millisecond < 1000
    }

    #[must_use]
    pub fn to_total_milliseconds(&self) -> u64 {
        ((u64::from(self.hour) * 3600 + u64::from(self.minute) * 60 + u64::from(self.second))
            * 1000)
            + u64::from(self.millisecond)
    }

    #[must_use]
    pub fn from_total_milliseconds(ms: u64) -> (Self, u64) {
        let ms_per_day = 86_400_000u64;
        let days_overflow = ms / ms_per_day;
        let rem_ms = ms % ms_per_day;

        let hour = (rem_ms / 3_600_000) as u32;
        let rem_ms = rem_ms % 3_600_000;
        let minute = (rem_ms / 60_000) as u32;
        let rem_ms = rem_ms % 60_000;
        let second = (rem_ms / 1000) as u32;
        let millisecond = (rem_ms % 1000) as u32;

        (
            Self {
                hour,
                minute,
                second,
                millisecond,
            },
            days_overflow,
        )
    }

    #[must_use]
    pub fn parse_time_spec(s: &str, has_at: bool) -> Option<(Time, usize)> {
        if s.is_empty() {
            return None;
        }

        let mut char_indices = s.char_indices().peekable();
        let &(start_idx, first_c) = char_indices.peek()?;
        if !first_c.is_ascii_digit() {
            return None;
        }

        let mut end_num1 = start_idx;
        while let Some(&(idx, c)) = char_indices.peek() {
            if c.is_ascii_digit() {
                end_num1 = idx + 1;
                char_indices.next();
            } else {
                break;
            }
        }

        let hour_str = &s[start_idx..end_num1];
        let mut hour = hour_str.parse::<u32>().ok()?;

        let mut minute = 0u32;
        let mut second = 0u32;
        let mut millisecond = 0u32;
        let mut current_end = end_num1;

        let has_colon = char_indices.peek().is_some_and(|&(_, c)| c == ':');

        if has_colon {
            char_indices.next(); // consume ':'
            let start_num2 = current_end + 1;
            let mut end_num2 = start_num2;

            while let Some(&(idx, c)) = char_indices.peek() {
                if c.is_ascii_digit() {
                    end_num2 = idx + 1;
                    char_indices.next();
                } else {
                    break;
                }
            }

            if end_num2 == start_num2 {
                return None;
            }

            minute = s[start_num2..end_num2].parse::<u32>().ok()?;
            current_end = end_num2;

            if char_indices.peek().is_some_and(|&(_, c)| c == ':') {
                char_indices.next(); // consume ':'
                let start_num3 = current_end + 1;
                let mut end_num3 = start_num3;

                while let Some(&(idx, c)) = char_indices.peek() {
                    if c.is_ascii_digit() {
                        end_num3 = idx + 1;
                        char_indices.next();
                    } else {
                        break;
                    }
                }

                if end_num3 > start_num3 {
                    second = s[start_num3..end_num3].parse::<u32>().ok()?;
                    current_end = end_num3;

                    if char_indices.peek().is_some_and(|&(_, c)| c == '.') {
                        let mut dot_lookahead = char_indices.clone();
                        dot_lookahead.next();
                        if dot_lookahead
                            .peek()
                            .is_some_and(|&(_, c)| c.is_ascii_digit())
                        {
                            char_indices.next(); // consume '.'
                            let start_ms = current_end + 1;
                            let mut end_ms = start_ms;
                            while let Some(&(idx, c)) = char_indices.peek() {
                                if c.is_ascii_digit() {
                                    end_ms = idx + 1;
                                    char_indices.next();
                                } else {
                                    break;
                                }
                            }
                            let ms_str = &s[start_ms..end_ms];
                            millisecond = match ms_str.len() {
                                1 => ms_str.parse::<u32>().unwrap_or(0) * 100,
                                2 => ms_str.parse::<u32>().unwrap_or(0) * 10,
                                3 => ms_str.parse::<u32>().unwrap_or(0),
                                _ => ms_str[..3].parse::<u32>().unwrap_or(0),
                            };
                            current_end = end_ms;
                        }
                    }
                }
            }
        }

        let after_time = &s[current_end..];
        let trimmed = after_time.trim_start();
        let ws_len = after_time.len() - trimmed.len();
        let upper_trimmed = trimmed.to_ascii_uppercase();

        let mut is_am_pm = false;
        let mut is_pm = false;

        if upper_trimmed.starts_with("AM") {
            if trimmed.len() == 2 || !trimmed.as_bytes()[2].is_ascii_alphanumeric() {
                is_am_pm = true;
                is_pm = false;
                current_end += ws_len + 2;
            }
        } else if upper_trimmed.starts_with("PM")
            && (trimmed.len() == 2 || !trimmed.as_bytes()[2].is_ascii_alphanumeric())
        {
            is_am_pm = true;
            is_pm = true;
            current_end += ws_len + 2;
        }

        if is_am_pm {
            if hour == 0 || hour > 12 {
                return None;
            }
            if is_pm && hour < 12 {
                hour += 12;
            } else if !is_pm && hour == 12 {
                hour = 0;
            }
        } else if !has_colon && !has_at {
            return None;
        }

        let time = Self::new(hour, minute, second, millisecond);
        if time.is_valid() {
            Some((time, current_end))
        } else {
            None
        }
    }

    #[must_use]
    pub fn format(&self) -> String {
        if self.millisecond == 0 {
            format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
        } else {
            format!(
                "{:02}:{:02}:{:02}.{:03}",
                self.hour, self.minute, self.second, self.millisecond
            )
        }
    }

    #[must_use]
    pub fn format_12h(&self) -> String {
        let (h12, is_pm) = match self.hour {
            0 => (12, false),
            1..=11 => (self.hour, false),
            12 => (12, true),
            _ => (self.hour - 12, true),
        };
        let am_pm = if is_pm { "PM" } else { "AM" };
        if self.millisecond == 0 {
            format!("{:02}:{:02}:{:02} {}", h12, self.minute, self.second, am_pm)
        } else {
            format!(
                "{:02}:{:02}:{:02}.{:03} {}",
                h12, self.minute, self.second, self.millisecond, am_pm
            )
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl FromStr for Time {
    type Err = AbacusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Time::parse_time_spec(s.trim(), false)
            .map(|(t, _)| t)
            .ok_or_else(|| AbacusError::InvalidDate(format!("invalid time format: '{s}'")))
    }
}

/// Helper function to check if a year is a leap year in the Gregorian calendar.
#[must_use]
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Helper function to return the number of days in a given month of a year.
#[must_use]
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Check if year, month, day form a valid calendar date.
#[must_use]
pub fn is_valid_date(year: i32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    day <= days_in_month(year, month)
}

/// Helper function to return the English full name of a month.
#[must_use]
pub fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

/// Convert (year, month, day) to days since Unix epoch 1970-01-01 (Proleptic Gregorian algorithm).
#[must_use]
pub fn date_to_epoch_days(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 {
        i64::from(year) - 1
    } else {
        i64::from(year)
    };
    let m = if month <= 2 {
        i64::from(month) + 12
    } else {
        i64::from(month)
    };
    let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Convert days since Unix epoch 1970-01-01 to (year, month, day).
#[must_use]
pub fn epoch_days_to_date(epoch_days: i64) -> (i32, u32, u32) {
    let z = epoch_days.saturating_add(719468);
    let era = if z >= 0 {
        z / 146097
    } else {
        (z.saturating_sub(146096)) / 146097
    };
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Structure representing a calendar Date with Time and optional `TimeZone`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub time: Time,
    pub timezone: Option<TimeZone>,
    pub format: DateFormat,
}

impl PartialEq for Date {
    fn eq(&self, other: &Self) -> bool {
        self.year == other.year
            && self.month == other.month
            && self.day == other.day
            && self.time == other.time
            && self.timezone == other.timezone
    }
}

impl Eq for Date {}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.year, self.month, self.day, &self.time, &self.timezone).cmp(&(
            other.year,
            other.month,
            other.day,
            &other.time,
            &other.timezone,
        ))
    }
}

impl std::hash::Hash for Date {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.year.hash(state);
        self.month.hash(state);
        self.day.hash(state);
        self.time.hash(state);
        self.timezone.hash(state);
    }
}

use std::time::{SystemTime, UNIX_EPOCH};

impl Date {
    #[must_use]
    pub fn now() -> Self {
        let start = SystemTime::now();
        let since_epoch = start.duration_since(UNIX_EPOCH).unwrap_or_default();
        let total_ms = since_epoch.as_millis() as i64;
        Self::from_epoch_milliseconds(total_ms)
    }

    #[must_use]
    pub fn today() -> Self {
        let now = Self::now();
        Self::new(now.year, now.month, now.day)
    }

    #[must_use]
    pub fn tomorrow() -> Self {
        Self::today().add_days(1)
    }

    #[must_use]
    pub fn yesterday() -> Self {
        Self::today().add_days(-1)
    }

    #[must_use]
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self {
            year,
            month,
            day,
            time: Time::default(),
            timezone: None,
            format: DateFormat::default(),
        }
    }

    #[must_use]
    pub fn with_time(year: i32, month: u32, day: u32, time: Time) -> Self {
        Self {
            year,
            month,
            day,
            time,
            timezone: None,
            format: DateFormat::default(),
        }
    }

    #[must_use]
    pub fn with_timezone(mut self, tz: TimeZone) -> Self {
        self.timezone = Some(tz);
        self
    }

    #[must_use]
    pub fn with_format(mut self, format: DateFormat) -> Self {
        self.format = format;
        self
    }

    #[must_use]
    pub fn parse_ymd_components(s: &str) -> Option<(i32, u32, u32, usize)> {
        let bytes = s.as_bytes();
        let mut start = 0;
        while start < bytes.len() && (bytes[start] == b' ' || bytes[start] == b'\t') {
            start += 1;
        }
        if start >= bytes.len() || !bytes[start].is_ascii_digit() {
            return None;
        }

        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_digit() || bytes[end] == b'-' || bytes[end] == b'/')
        {
            end += 1;
        }
        let date_bytes = &bytes[start..end];
        let sep = if date_bytes.contains(&b'-') {
            b'-'
        } else if date_bytes.contains(&b'/') {
            b'/'
        } else {
            return None;
        };

        let mut split = date_bytes.split(|&b| b == sep);
        let p1 = split.next()?;
        let p2 = split.next()?;
        let p3 = split.next()?;
        if split.next().is_some() {
            return None;
        }

        if !(p1.len() == 4 || p3.len() == 4) || p1.is_empty() || p2.is_empty() || p3.is_empty() {
            return None;
        }
        if !p1.iter().all(u8::is_ascii_digit)
            || !p2.iter().all(u8::is_ascii_digit)
            || !p3.iter().all(u8::is_ascii_digit)
        {
            return None;
        }

        let p1_str = std::str::from_utf8(p1).ok()?;
        let p2_str = std::str::from_utf8(p2).ok()?;
        let p3_str = std::str::from_utf8(p3).ok()?;

        let (year, month, day) = if p1.len() == 4 {
            let yr = p1_str.parse::<i32>().ok()?;
            let n2 = p2_str.parse::<u32>().ok()?;
            let n3 = p3_str.parse::<u32>().ok()?;
            if n2 > 12 && n3 <= 12 {
                (yr, n3, n2)
            } else {
                (yr, n2, n3)
            }
        } else {
            let n1 = p1_str.parse::<u32>().ok()?;
            let n2 = p2_str.parse::<u32>().ok()?;
            let yr = p3_str.parse::<i32>().ok()?;
            if n1 <= 12 && n2 > 12 {
                // e.g. 05-16-2010 or 05/16/2010 -> month = 5, day = 16
                (yr, n1, n2)
            } else if n1 > 12 && n2 <= 12 {
                // e.g. 16-05-2010 or 16/05/2010 -> month = 5, day = 16
                (yr, n2, n1)
            } else {
                // Ambiguous e.g. 07-08-2026 -> default DD-MM-YYYY
                (yr, n2, n1)
            }
        };

        if !is_valid_date(year, month, day) {
            return None;
        }

        Some((year, month, day, end))
    }

    pub fn apply_time_value(&self, rhs: &Value, sign: i64) -> Result<Date, AbacusError> {
        self.apply_time_value_with(rhs, sign, WeekendDays::SaturdaySunday)
    }

    pub fn apply_time_value_with(
        &self,
        rhs: &Value,
        sign: i64,
        weekend: WeekendDays,
    ) -> Result<Date, AbacusError> {
        if rhs.unit.dimensions != Dimensions::TIME {
            return Err(AbacusError::IncompatibleDimensions);
        }
        if rhs.unit.is_business_day_unit() {
            let count = (rhs.canonical / 86400.0).round() as i64;
            Ok(self.add_business_days_with(sign * count, weekend))
        } else {
            let ms = (rhs.canonical * 1000.0).round() as i64;
            Ok(self.add_milliseconds(sign * ms))
        }
    }

    /// Retrieves a numerical property value from this Date by property name using custom weekend rules.
    #[must_use]
    pub fn get_property_with(&self, prop: &str, weekend: WeekendDays) -> Option<f64> {
        match prop {
            "year" => Some(f64::from(self.year)),
            "month" => Some(f64::from(self.month)),
            "day" => Some(f64::from(self.day)),
            "hour" => Some(f64::from(self.time.hour)),
            "minute" => Some(f64::from(self.time.minute)),
            "second" => Some(f64::from(self.time.second)),
            "millisecond" | "ms" => Some(f64::from(self.time.millisecond)),
            "day_of_week" | "weekday" => Some(f64::from(self.day_of_week() as u32)),
            "day_of_year" => Some(f64::from(self.day_of_year())),
            "is_weekend" => Some(if self.is_weekend_with(weekend) {
                1.0
            } else {
                0.0
            }),
            "is_workday" | "is_business_day" => Some(if self.is_business_day_with(weekend) {
                1.0
            } else {
                0.0
            }),
            "offset" | "offset_minutes" => Some(
                self.timezone
                    .as_ref()
                    .map_or(0.0, |tz| f64::from(tz.offset_minutes)),
            ),
            _ => None,
        }
    }

    /// Retrieves a numerical property value from this Date by property name.
    #[must_use]
    pub fn get_property(&self, prop: &str) -> Option<f64> {
        self.get_property_with(prop, WeekendDays::SaturdaySunday)
    }

    #[must_use]
    pub fn new_with_hms(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Self {
        Self {
            year,
            month,
            day,
            time: Time::from_hms(hour, minute, second),
            timezone: None,
            format: DateFormat::default(),
        }
    }

    #[must_use]
    pub fn new_full(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        millisecond: u32,
    ) -> Self {
        Self {
            year,
            month,
            day,
            time: Time::new(hour, minute, second, millisecond),
            timezone: None,
            format: DateFormat::default(),
        }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        is_valid_date(self.year, self.month, self.day) && self.time.is_valid()
    }

    #[must_use]
    pub fn day_of_week(&self) -> DayOfWeek {
        let epoch_days = self.to_epoch_days();
        let dow = (epoch_days + 3).rem_euclid(7) + 1;
        DayOfWeek::from_iso_number(dow as u32).unwrap_or(DayOfWeek::Thursday)
    }

    #[must_use]
    pub fn day_of_year(&self) -> u32 {
        let start_of_year = date_to_epoch_days(self.year, 1, 1);
        (self.to_epoch_days() - start_of_year + 1) as u32
    }

    #[must_use]
    pub fn to_epoch_days(&self) -> i64 {
        date_to_epoch_days(self.year, self.month, self.day)
    }

    #[must_use]
    pub fn from_epoch_days(days: i64) -> Self {
        let (year, month, day) = epoch_days_to_date(days);
        Self::new(year, month, day)
    }

    #[must_use]
    pub fn to_epoch_milliseconds(&self) -> i64 {
        let local_days_ms = self.to_epoch_days().saturating_mul(86_400_000);
        let local_time_ms = self.time.to_total_milliseconds() as i64;
        let local_ms = local_days_ms.saturating_add(local_time_ms);

        if let Some(ref tz) = self.timezone {
            local_ms.saturating_sub(i64::from(tz.offset_minutes).saturating_mul(60_000))
        } else {
            local_ms
        }
    }

    #[must_use]
    pub fn from_epoch_milliseconds(total_ms: i64) -> Self {
        let ms_per_day = 86_400_000i64;
        let days = total_ms.div_euclid(ms_per_day);
        let rem_ms = total_ms.rem_euclid(ms_per_day) as u64;

        let (time, _) = Time::from_total_milliseconds(rem_ms);
        let (year, month, day) = epoch_days_to_date(days);

        Self {
            year,
            month,
            day,
            time,
            timezone: None,
            format: DateFormat::default(),
        }
    }

    // TimeZone conversions
    #[must_use]
    pub fn to_utc(&self) -> Date {
        if let Some(ref tz) = self.timezone {
            if tz.offset_minutes == 0 {
                return self.clone();
            }
            let utc_ms = self.to_epoch_milliseconds();
            let mut d = Self::from_epoch_milliseconds(utc_ms);
            d.timezone = Some(TimeZone::utc());
            d.format = self.format;
            d
        } else {
            self.clone()
        }
    }

    #[must_use]
    pub fn to_timezone(&self, target_tz: &TimeZone) -> Date {
        let utc_ms = self.to_epoch_milliseconds();
        let target_ms = utc_ms + (i64::from(target_tz.offset_minutes) * 60_000);
        let mut d = Self::from_epoch_milliseconds(target_ms);
        d.timezone = Some(target_tz.clone());
        d.format = self.format;
        d
    }

    // Arithmetic methods
    #[must_use]
    pub fn add_milliseconds(&self, ms: i64) -> Self {
        if let Some(ref tz) = self.timezone {
            let utc_ms = self.to_epoch_milliseconds().saturating_add(ms);
            let target_ms =
                utc_ms.saturating_add(i64::from(tz.offset_minutes).saturating_mul(60_000));
            let mut d = Self::from_epoch_milliseconds(target_ms);
            d.timezone = Some(tz.clone());
            d.format = self.format;
            d
        } else {
            let mut d =
                Self::from_epoch_milliseconds(self.to_epoch_milliseconds().saturating_add(ms));
            d.format = self.format;
            d
        }
    }

    #[must_use]
    pub fn add_seconds(&self, seconds: i64) -> Self {
        self.add_milliseconds(seconds.saturating_mul(1000))
    }

    #[must_use]
    pub fn add_minutes(&self, minutes: i64) -> Self {
        self.add_milliseconds(minutes.saturating_mul(60_000))
    }

    #[must_use]
    pub fn add_hours(&self, hours: i64) -> Self {
        self.add_milliseconds(hours.saturating_mul(3_600_000))
    }

    #[must_use]
    pub fn add_days(&self, days: i64) -> Self {
        self.add_milliseconds(days.saturating_mul(86_400_000))
    }

    #[must_use]
    pub fn sub_days(&self, days: i64) -> Self {
        self.add_days(days.saturating_neg())
    }

    #[must_use]
    pub fn add_months(&self, months: i32) -> Self {
        let total_months = i64::from(self.year)
            .saturating_mul(12)
            .saturating_add(i64::from(self.month) - 1)
            .saturating_add(i64::from(months));
        let new_year = total_months.div_euclid(12) as i32;
        let new_month = (total_months.rem_euclid(12) + 1) as u32;

        let max_days = days_in_month(new_year, new_month);
        let new_day = self.day.min(max_days);

        Self {
            year: new_year,
            month: new_month,
            day: new_day,
            time: self.time,
            timezone: self.timezone.clone(),
            format: self.format,
        }
    }

    #[must_use]
    pub fn add_years(&self, years: i32) -> Self {
        self.add_months(years.saturating_mul(12))
    }

    #[must_use]
    pub fn days_between(&self, other: &Self) -> i64 {
        self.seconds_between(other) / 86_400
    }

    #[must_use]
    pub fn seconds_between(&self, other: &Self) -> i64 {
        (other
            .to_epoch_milliseconds()
            .saturating_sub(self.to_epoch_milliseconds()))
            / 1000
    }

    #[must_use]
    pub fn milliseconds_between(&self, other: &Self) -> i64 {
        other.to_epoch_milliseconds() - self.to_epoch_milliseconds()
    }

    // Formatting methods
    #[must_use]
    pub fn format_with_style(&self, style: DateFormat) -> String {
        let tz_suffix = if let Some(ref tz) = self.timezone {
            format!(" {}", tz.name)
        } else {
            String::new()
        };
        let date_part = match style {
            DateFormat::MonthDayYear => {
                format!("{} {}, {:04}", month_name(self.month), self.day, self.year)
            }
            DateFormat::DDMMYYYY => format!("{:02}-{:02}-{:04}", self.day, self.month, self.year),
            DateFormat::YYYYMMDD => format!("{:04}-{:02}-{:02}", self.year, self.month, self.day),
            DateFormat::MMDDYYYY => format!("{:02}-{:02}-{:04}", self.month, self.day, self.year),
        };

        if self.time.hour == 0
            && self.time.minute == 0
            && self.time.second == 0
            && self.time.millisecond == 0
        {
            format!("{date_part}{tz_suffix}")
        } else {
            format!("{date_part} {}{tz_suffix}", self.time.format())
        }
    }

    #[must_use]
    pub fn format(&self) -> String {
        self.format_with_style(self.format)
    }

    #[must_use]
    pub fn format_iso(&self) -> String {
        self.format_with_style(DateFormat::YYYYMMDD)
    }
    #[must_use]
    pub fn is_weekend_with(&self, weekend: WeekendDays) -> bool {
        weekend.is_weekend(self.day_of_week())
    }

    #[must_use]
    pub fn is_weekend(&self) -> bool {
        self.is_weekend_with(WeekendDays::SaturdaySunday)
    }

    #[must_use]
    pub fn is_business_day_with(&self, weekend: WeekendDays) -> bool {
        weekend.is_business_day(self.day_of_week())
    }

    #[must_use]
    pub fn is_business_day(&self) -> bool {
        self.is_business_day_with(WeekendDays::SaturdaySunday)
    }

    #[must_use]
    pub fn add_business_days_with(&self, n: i64, weekend: WeekendDays) -> Self {
        if n == 0 {
            return self.clone();
        }
        let step = if n > 0 { 1 } else { -1 };
        let mut cur = self.clone();
        let mut remaining = n.abs();

        // If starting on weekend, advance to the first business day
        while !cur.is_business_day_with(weekend) {
            cur = cur.add_days(step);
            if cur.is_business_day_with(weekend) {
                remaining -= 1;
                break;
            }
        }

        let bdays_per_week = weekend.business_days_per_week();
        if bdays_per_week > 0 {
            let weeks = remaining / bdays_per_week;
            remaining %= bdays_per_week;

            if weeks > 0 {
                cur = cur.add_days(weeks * 7 * step);
            }
        }

        for _ in 0..remaining {
            cur = cur.add_days(step);
            while !cur.is_business_day_with(weekend) {
                cur = cur.add_days(step);
            }
        }
        cur
    }

    #[must_use]
    pub fn add_business_days(&self, n: i64) -> Self {
        self.add_business_days_with(n, WeekendDays::SaturdaySunday)
    }

    #[must_use]
    pub fn business_days_between_with(&self, other: &Self, weekend: WeekendDays) -> i64 {
        let self_days = self.to_epoch_days();
        let other_days = other.to_epoch_days();

        if self_days == other_days {
            return 0;
        }

        let (start, end, sign) = if self_days < other_days {
            (self_days, other_days, 1i64)
        } else {
            (other_days, self_days, -1i64)
        };

        let diff = end - start;
        let weeks = diff / 7;
        let rem = diff % 7;

        let bdays_per_week = weekend.business_days_per_week();
        let mut count = weeks * bdays_per_week;
        for i in 1..=rem {
            let day = start + weeks * 7 + i;
            let dow_num = (day + 3).rem_euclid(7) + 1;
            if let Some(dow) = DayOfWeek::from_iso_number(dow_num as u32)
                && weekend.is_business_day(dow)
            {
                count += 1;
            }
        }

        count * sign
    }

    #[must_use]
    pub fn business_days_between(&self, other: &Self) -> i64 {
        self.business_days_between_with(other, WeekendDays::SaturdaySunday)
    }
}

/// Date format style enum for displaying dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DateFormat {
    /// Month Day, Year format (e.g. "May 16, 2010" / "August 7, 2026") - Default!
    #[default]
    MonthDayYear,
    /// DD-MM-YYYY format (e.g. 07-08-2026)
    DDMMYYYY,
    /// YYYY-MM-DD format (e.g. 2026-08-07)
    YYYYMMDD,
    /// MM-DD-YYYY format (e.g. 08-07-2026)
    MMDDYYYY,
}

impl Default for Date {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

// String Parsing
impl FromStr for Date {
    type Err = AbacusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(AbacusError::InvalidDate("empty string".to_string()));
        }

        // Handle ISO T separator: e.g. "2026-08-07T10:00:00Z"
        let normalized = if !s.contains(' ') && s.contains('T') {
            s.replace('T', " ")
        } else {
            s.to_string()
        };

        let (year, month, day, consumed) =
            if let Some(res) = Date::parse_ymd_components(&normalized) {
                res
            } else if let Some((date, consumed)) =
                crate::evaluation::tokenizer::date_literal::try_parse_textual_date(
                    &normalized,
                    &Date::today(),
                )
            {
                (date.year, date.month, date.day, consumed)
            } else {
                return Err(AbacusError::InvalidDate(format!(
                    "invalid date format: '{s}'"
                )));
            };

        let mut time = Time::new(0, 0, 0, 0);
        let mut timezone = None;

        let rem = normalized[consumed..].trim_start();
        if !rem.is_empty() {
            if let Some((parsed_time, time_len)) = Time::parse_time_spec(rem, false) {
                time = parsed_time;
                let tz_part = rem[time_len..].trim();
                if !tz_part.is_empty()
                    && let Ok(tz) = TimeZone::parse(tz_part)
                {
                    timezone = Some(tz);
                }
            } else {
                let tz_part = rem.trim();
                if let Ok(tz) = TimeZone::parse(tz_part) {
                    timezone = Some(tz);
                }
            }
        }

        let mut date = Date::with_time(year, month, day, time);
        date.timezone = timezone;

        if !date.is_valid() {
            return Err(AbacusError::InvalidDate(format!(
                "date out of bounds: '{s}'"
            )));
        }

        Ok(date)
    }
}

// Operators for Date arithmetic
impl Sub<&Date> for &Date {
    type Output = Value;

    fn sub(self, rhs: &Date) -> Self::Output {
        let diff_ms = self.to_epoch_milliseconds() - rhs.to_epoch_milliseconds();
        let seconds = diff_ms as f64 / 1000.0;

        let unit = Unit {
            scalar: 1.0,
            offset: 0.0,
            dimensions: Dimensions::TIME,
            display: crate::units::unit::UnitExpr::single("s"),
        };
        Value::new(seconds, Arc::new(unit))
    }
}

impl Sub<Date> for Date {
    type Output = Value;
    fn sub(self, rhs: Date) -> Self::Output {
        &self - &rhs
    }
}

impl Add<&Value> for &Date {
    type Output = Result<Date, AbacusError>;

    fn add(self, rhs: &Value) -> Self::Output {
        self.apply_time_value(rhs, 1)
    }
}

impl Add<Value> for Date {
    type Output = Result<Date, AbacusError>;
    fn add(self, rhs: Value) -> Self::Output {
        &self + &rhs
    }
}

impl Sub<&Value> for &Date {
    type Output = Result<Date, AbacusError>;

    fn sub(self, rhs: &Value) -> Self::Output {
        self.apply_time_value(rhs, -1)
    }
}

impl Sub<Value> for Date {
    type Output = Result<Date, AbacusError>;
    fn sub(self, rhs: Value) -> Self::Output {
        &self - &rhs
    }
}
