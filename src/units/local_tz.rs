//! Minimal, dependency-free resolution of the system's local UTC offset.
//!
//! On Unix the zone is taken from the `TZ` environment variable (a zone name such as
//! `America/New_York`, an absolute TZif path, or a POSIX rule string such as `EST5EDT,M3.2.0,M11.1.0`),
//! falling back to `/etc/localtime`. The TZif file is parsed once per process (including its POSIX
//! footer rule for instants beyond the last transition), so DST is honoured. When no zone information
//! is available (e.g. on non-Unix targets) the local zone is UTC.

use std::sync::OnceLock;

use crate::units::date::{date_to_epoch_days, days_in_month, is_leap_year};

/// A POSIX TZ transition date rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleDate {
    /// `Jn`: Julian day 1..=365, February 29 is never counted.
    Julian1(u32),
    /// `n`: zero-based day of year 0..=365, February 29 counted in leap years.
    Julian0(u32),
    /// `Mm.w.d`: day `d` (0 = Sunday) of week `w` (1..=5, 5 = last) of month `m`.
    MonthWeekDay(u32, u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rule {
    date: RuleDate,
    /// Local wall-clock seconds after midnight at which the transition occurs.
    time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DstRule {
    abbr: String,
    offset: i32,
    start: Rule,
    end: Rule,
}

/// A parsed POSIX TZ rule string (`STDoffset[DST[offset][,start[/time],end[/time]]]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PosixTz {
    std_abbr: String,
    /// Seconds east of UTC.
    std_offset: i32,
    dst: Option<DstRule>,
}

#[derive(Debug, Clone)]
struct LocalType {
    offset: i32,
    abbr: String,
}

/// A parsed TZif zone.
#[derive(Debug, Clone)]
pub(crate) struct Tzif {
    transitions: Vec<i64>,
    type_indices: Vec<u8>,
    types: Vec<LocalType>,
    footer: Option<PosixTz>,
}

/// A local time zone description.
#[derive(Debug, Clone)]
pub(crate) enum Zone {
    Tzif(Tzif),
    Posix(PosixTz),
}

fn parse_abbr(s: &str) -> Option<(String, &str)> {
    if let Some(rest) = s.strip_prefix('<') {
        let end = rest.find('>')?;
        return Some((rest[..end].to_string(), &rest[end + 1..]));
    }
    let end = s
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(s.len());
    if end < 3 {
        return None;
    }
    Some((s[..end].to_string(), &s[end..]))
}

/// Parses `[+-]hh[:mm[:ss]]` returning seconds and the remaining input.
fn parse_hms(s: &str) -> Option<(i64, &str)> {
    let (sign, body) = match s.as_bytes().first() {
        Some(b'-') => (-1, &s[1..]),
        Some(b'+') => (1, &s[1..]),
        _ => (1, s),
    };
    let mut rest = body;
    let mut total = 0i64;
    for (i, mult) in [3600i64, 60, 1].into_iter().enumerate() {
        if i > 0 {
            match rest.strip_prefix(':') {
                Some(r) => rest = r,
                None => break,
            }
        }
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        total += rest[..end].parse::<i64>().ok()? * mult;
        rest = &rest[end..];
    }
    Some((sign * total, rest))
}

fn parse_rule(s: &str) -> Option<(Rule, &str)> {
    let (date, mut rest) = if let Some(r) = s.strip_prefix('M') {
        let mut parts = [0u32; 3];
        let mut cur = r;
        for (i, part) in parts.iter_mut().enumerate() {
            if i > 0 {
                cur = cur.strip_prefix('.')?;
            }
            let end = cur.find(|c: char| !c.is_ascii_digit()).unwrap_or(cur.len());
            *part = cur[..end].parse().ok()?;
            cur = &cur[end..];
        }
        if !(1..=12).contains(&parts[0]) || !(1..=5).contains(&parts[1]) || parts[2] > 6 {
            return None;
        }
        (RuleDate::MonthWeekDay(parts[0], parts[1], parts[2]), cur)
    } else if let Some(r) = s.strip_prefix('J') {
        let end = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        let n: u32 = r[..end].parse().ok()?;
        if !(1..=365).contains(&n) {
            return None;
        }
        (RuleDate::Julian1(n), &r[end..])
    } else {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let n: u32 = s[..end].parse().ok()?;
        if n > 365 {
            return None;
        }
        (RuleDate::Julian0(n), &s[end..])
    };
    let mut time = 7200;
    if let Some(r) = rest.strip_prefix('/') {
        let (t, r2) = parse_hms(r)?;
        time = t;
        rest = r2;
    }
    Some((Rule { date, time }, rest))
}

impl PosixTz {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let (std_abbr, rest) = parse_abbr(s)?;
        let (std_west, rest) = parse_hms(rest)?;
        let std_offset = i32::try_from(-std_west).ok()?;
        if rest.is_empty() {
            return Some(Self {
                std_abbr,
                std_offset,
                dst: None,
            });
        }
        let (dst_abbr, mut rest) = parse_abbr(rest)?;
        let mut dst_offset = std_offset + 3600;
        if !rest.is_empty() && !rest.starts_with(',') {
            let (west, r) = parse_hms(rest)?;
            dst_offset = i32::try_from(-west).ok()?;
            rest = r;
        }
        let (start, end) = if let Some(r) = rest.strip_prefix(',') {
            let (start, r) = parse_rule(r)?;
            let r = r.strip_prefix(',')?;
            let (end, r) = parse_rule(r)?;
            if !r.is_empty() {
                return None;
            }
            (start, end)
        } else if rest.is_empty() {
            // POSIX default when rules are omitted: US rules.
            (
                Rule {
                    date: RuleDate::MonthWeekDay(3, 2, 0),
                    time: 7200,
                },
                Rule {
                    date: RuleDate::MonthWeekDay(11, 1, 0),
                    time: 7200,
                },
            )
        } else {
            return None;
        };
        Some(Self {
            std_abbr,
            std_offset,
            dst: Some(DstRule {
                abbr: dst_abbr,
                offset: dst_offset,
                start,
                end,
            }),
        })
    }

    /// Epoch day (UTC calendar) on which a rule falls in `year`.
    fn rule_epoch_day(year: i32, date: RuleDate) -> i64 {
        let jan1 = date_to_epoch_days(year, 1, 1);
        match date {
            RuleDate::Julian1(n) => {
                let n = i64::from(n) - 1;
                if is_leap_year(year) && n >= 59 {
                    jan1 + n + 1
                } else {
                    jan1 + n
                }
            }
            RuleDate::Julian0(n) => jan1 + i64::from(n),
            RuleDate::MonthWeekDay(m, w, d) => {
                let first = date_to_epoch_days(year, m, 1);
                // 1970-01-01 was a Thursday (4 when Sunday = 0).
                let first_dow = (first + 4).rem_euclid(7);
                let mut day = 1 + (i64::from(d) - first_dow).rem_euclid(7) + (i64::from(w) - 1) * 7;
                let dim = i64::from(days_in_month(year, m));
                while day > dim {
                    day -= 7;
                }
                first + day - 1
            }
        }
    }

    /// Returns `(offset seconds east of UTC, abbreviation)` in effect at the UTC instant `t` (seconds).
    pub(crate) fn offset_at(&self, t: i64) -> (i32, &str) {
        let Some(dst) = &self.dst else {
            return (self.std_offset, &self.std_abbr);
        };
        let local = t + i64::from(self.std_offset);
        let year = crate::units::date::epoch_days_to_date(local.div_euclid(86_400)).0;
        let start = Self::rule_epoch_day(year, dst.start.date) * 86_400 + dst.start.time
            - i64::from(self.std_offset);
        let end = Self::rule_epoch_day(year, dst.end.date) * 86_400 + dst.end.time
            - i64::from(dst.offset);
        let in_dst = if start < end {
            t >= start && t < end
        } else {
            !(t >= end && t < start)
        };
        if in_dst {
            (dst.offset, &dst.abbr)
        } else {
            (self.std_offset, &self.std_abbr)
        }
    }
}

fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

impl Tzif {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 44 || &bytes[..4] != b"TZif" {
            return None;
        }
        let version = bytes[4];
        let counts = |base: usize| -> Option<[usize; 6]> {
            let mut c = [0usize; 6];
            for (i, v) in c.iter_mut().enumerate() {
                *v = be_u32(bytes, base + 20 + i * 4)? as usize;
            }
            Some(c)
        };
        let [isut, isstd, leap, timecnt, typecnt, charcnt] = counts(0)?;
        let v1_len = timecnt * 5 + typecnt * 6 + charcnt + leap * 8 + isstd + isut;

        let (base, time_size) = if version >= b'2' {
            (44 + v1_len, 8usize)
        } else {
            (0, 4usize)
        };
        let [isut, isstd, leap, timecnt, typecnt, charcnt] = counts(base)?;
        let mut p = base + 44;
        let mut transitions = Vec::with_capacity(timecnt);
        for i in 0..timecnt {
            let at = p + i * time_size;
            let t = if time_size == 8 {
                i64::from_be_bytes(bytes.get(at..at + 8)?.try_into().ok()?)
            } else {
                i64::from(i32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
            };
            transitions.push(t);
        }
        p += timecnt * time_size;
        let type_indices = bytes.get(p..p + timecnt)?.to_vec();
        p += timecnt;
        let ttinfo = bytes.get(p..p + typecnt * 6)?;
        p += typecnt * 6;
        let chars = bytes.get(p..p + charcnt)?;
        p += charcnt + leap * (time_size + 4) + isstd + isut;

        let mut types = Vec::with_capacity(typecnt);
        for i in 0..typecnt {
            let rec = &ttinfo[i * 6..i * 6 + 6];
            let offset = i32::from_be_bytes(rec[..4].try_into().ok()?);
            let idx = rec[5] as usize;
            let abbr_bytes = chars.get(idx..)?;
            let end = abbr_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(abbr_bytes.len());
            let abbr = String::from_utf8_lossy(&abbr_bytes[..end]).into_owned();
            types.push(LocalType { offset, abbr });
        }
        if types.is_empty() || type_indices.iter().any(|&i| i as usize >= types.len()) {
            return None;
        }

        let footer = if version >= b'2' {
            bytes.get(p..).and_then(|rest| {
                let rest = rest.strip_prefix(b"\n")?;
                let end = rest.iter().position(|&b| b == b'\n')?;
                let s = std::str::from_utf8(&rest[..end]).ok()?;
                PosixTz::parse(s)
            })
        } else {
            None
        };

        Some(Self {
            transitions,
            type_indices,
            types,
            footer,
        })
    }

    pub(crate) fn offset_at(&self, t: i64) -> (i32, &str) {
        let idx = self.transitions.partition_point(|&tr| tr <= t);
        if idx == 0 {
            let ty = &self.types[0];
            return (ty.offset, &ty.abbr);
        }
        if idx == self.transitions.len()
            && let Some(footer) = &self.footer
        {
            return footer.offset_at(t);
        }
        let ty = &self.types[self.type_indices[idx - 1] as usize];
        (ty.offset, &ty.abbr)
    }
}

impl Zone {
    pub(crate) fn offset_at(&self, t: i64) -> (i32, &str) {
        match self {
            Self::Tzif(z) => z.offset_at(t),
            Self::Posix(z) => z.offset_at(t),
        }
    }
}

#[cfg(unix)]
fn load_system_zone() -> Option<Zone> {
    let read_tzif = |path: &str| {
        std::fs::read(path)
            .ok()
            .and_then(|b| Tzif::parse(&b))
            .map(Zone::Tzif)
    };
    match std::env::var("TZ") {
        Ok(tz) if !tz.is_empty() => {
            let name = tz.strip_prefix(':').unwrap_or(&tz);
            if name.starts_with('/') {
                return read_tzif(name);
            }
            for dir in [
                "/usr/share/zoneinfo",
                "/var/db/timezone/zoneinfo",
                "/usr/lib/zoneinfo",
            ] {
                if !name.contains("..")
                    && let Some(z) = read_tzif(&format!("{dir}/{name}"))
                {
                    return Some(z);
                }
            }
            PosixTz::parse(name).map(Zone::Posix)
        }
        _ => read_tzif("/etc/localtime"),
    }
}

#[cfg(not(unix))]
fn load_system_zone() -> Option<Zone> {
    None
}

fn system_zone() -> Option<&'static Zone> {
    static ZONE: OnceLock<Option<Zone>> = OnceLock::new();
    ZONE.get_or_init(load_system_zone).as_ref()
}

/// Returns the local UTC offset (minutes east) and zone abbreviation in effect at a UTC instant (ms).
pub(crate) fn local_offset_at_utc_ms(utc_ms: i64) -> (i32, String) {
    match system_zone() {
        Some(zone) => {
            let (secs, abbr) = zone.offset_at(utc_ms.div_euclid(1000));
            (secs / 60, abbr.to_string())
        }
        None => (0, "UTC".to_string()),
    }
}

/// Returns only the local UTC offset in minutes at a UTC instant (ms).
pub(crate) fn local_offset_minutes_at_utc_ms(utc_ms: i64) -> i32 {
    system_zone().map_or(0, |zone| zone.offset_at(utc_ms.div_euclid(1000)).0 / 60)
}

/// Converts a local wall-clock time (expressed as milliseconds since the epoch as if it were UTC)
/// into a UTC instant using the system zone.
pub(crate) fn local_wall_ms_to_utc_ms(wall_ms: i64) -> i64 {
    let first = local_offset_minutes_at_utc_ms(wall_ms);
    let guess = wall_ms.saturating_sub(i64::from(first) * 60_000);
    let second = local_offset_minutes_at_utc_ms(guess);
    if second == first {
        guess
    } else {
        wall_ms.saturating_sub(i64::from(second) * 60_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(y: i32, m: u32, d: u32, h: i64) -> i64 {
        date_to_epoch_days(y, m, d) * 86_400 + h * 3600
    }

    #[test]
    fn posix_us_rules() {
        let z = PosixTz::parse("EST5EDT,M3.2.0,M11.1.0").unwrap();
        assert_eq!(z.offset_at(utc(2026, 1, 15, 12)), (-18_000, "EST"));
        assert_eq!(z.offset_at(utc(2026, 7, 1, 12)), (-14_400, "EDT"));
        // 2026-03-08 02:00 EST == 07:00 UTC
        assert_eq!(z.offset_at(utc(2026, 3, 8, 7) - 1).0, -18_000);
        assert_eq!(z.offset_at(utc(2026, 3, 8, 7)).0, -14_400);
        // 2026-11-01 02:00 EDT == 06:00 UTC
        assert_eq!(z.offset_at(utc(2026, 11, 1, 6) - 1).0, -14_400);
        assert_eq!(z.offset_at(utc(2026, 11, 1, 6)).0, -18_000);
    }

    #[test]
    fn posix_southern_hemisphere_and_fixed() {
        let z = PosixTz::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap();
        assert_eq!(z.offset_at(utc(2026, 1, 15, 0)), (39_600, "AEDT"));
        assert_eq!(z.offset_at(utc(2026, 7, 15, 0)), (36_000, "AEST"));
        let jst = PosixTz::parse("JST-9").unwrap();
        assert_eq!(jst.offset_at(0), (32_400, "JST"));
        let ist = PosixTz::parse("<+0530>-5:30").unwrap();
        assert_eq!(ist.offset_at(0).0, 19_800);
        assert!(PosixTz::parse("").is_none());
        assert!(PosixTz::parse("X1").is_none());
    }

    #[test]
    fn parses_system_tzif_when_present() {
        for path in [
            "/usr/share/zoneinfo/America/New_York",
            "/var/db/timezone/zoneinfo/America/New_York",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                let z = Tzif::parse(&bytes).expect("valid tzif");
                assert_eq!(z.offset_at(utc(2026, 1, 15, 12)).0, -18_000);
                assert_eq!(z.offset_at(utc(2026, 7, 1, 12)).0, -14_400);
                assert_eq!(z.offset_at(utc(2090, 7, 1, 12)).0, -14_400);
                assert_eq!(z.offset_at(utc(1950, 1, 1, 12)).0, -18_000);
                return;
            }
        }
    }

    #[test]
    fn rejects_garbage_tzif() {
        assert!(Tzif::parse(b"not a tzif file at all, definitely not one").is_none());
        assert!(Tzif::parse(b"TZif").is_none());
    }
}
