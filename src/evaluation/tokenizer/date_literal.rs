use crate::AbacusError;
use crate::units::date::{Date, DayOfWeek, FLAG_EXPLICIT_MERIDIEM, FLAG_TIME_ONLY, Time, TimeZone};

/// If `haystack` starts with the literal word `word` (case-insensitively) and is followed
/// by a non-alphanumeric character (or end-of-string), returns the word length; otherwise
/// returns `None`.
#[inline]
pub(crate) fn strip_word_prefix(haystack: &str, word: &str) -> Option<usize> {
    let len = word.len();
    if haystack.len() >= len
        && haystack.as_bytes()[..len].eq_ignore_ascii_case(word.as_bytes())
        && (haystack.len() == len || !haystack.as_bytes()[len].is_ascii_alphanumeric())
    {
        Some(len)
    } else {
        None
    }
}

/// First words that can begin a textual date phrase (sorted, lowercase) — used as a cheap
/// pre-filter so ordinary identifiers skip the date-phrase parsers entirely.
const DATE_PHRASE_FIRST_WORDS: &[&str] = &[
    "1st",
    "2nd",
    "3rd",
    "4th",
    "5th",
    "apr",
    "april",
    "aug",
    "august",
    "beginning",
    "black",
    "boxing",
    "christmas",
    "cyber",
    "dec",
    "december",
    "easter",
    "end",
    "feb",
    "february",
    "fifth",
    "first",
    "fourth",
    "fri",
    "friday",
    "halloween",
    "independence",
    "jan",
    "january",
    "jul",
    "july",
    "jun",
    "june",
    "juneteenth",
    "labor",
    "labour",
    "last",
    "mar",
    "march",
    "martin",
    "may",
    "memorial",
    "mlk",
    "mon",
    "monday",
    "new",
    "next",
    "nov",
    "november",
    "now",
    "oct",
    "october",
    "past",
    "president",
    "presidents",
    "previous",
    "quarter",
    "remembrance",
    "sat",
    "saturday",
    "second",
    "sep",
    "sept",
    "september",
    "st",
    "start",
    "sun",
    "sunday",
    "tdy",
    "thanksgiving",
    "third",
    "this",
    "thu",
    "thurs",
    "thursday",
    "tmr",
    "today",
    "tomorrow",
    "tue",
    "tues",
    "tuesday",
    "valentine",
    "valentines",
    "veterans",
    "wed",
    "wednesday",
    "xmas",
    "yesterday",
];

/// Returns `true` if the word at the start of `s` can begin a textual date phrase.
pub(crate) fn could_start_date_phrase(s: &str) -> bool {
    let len = s
        .bytes()
        .position(|b| !b.is_ascii_alphanumeric())
        .unwrap_or(s.len());
    if len == 0 || len > 12 {
        return false;
    }
    let mut buf = [0u8; 12];
    for (dst, src) in buf.iter_mut().zip(s.as_bytes()[..len].iter()) {
        *dst = src.to_ascii_lowercase();
    }
    let Ok(word) = std::str::from_utf8(&buf[..len]) else {
        return false;
    };
    DATE_PHRASE_FIRST_WORDS.binary_search(&word).is_ok()
}

pub(crate) fn parse_weekday(s: &str) -> Option<(DayOfWeek, usize)> {
    let matches = [
        ("wednesday", DayOfWeek::Wednesday),
        ("thursday", DayOfWeek::Thursday),
        ("saturday", DayOfWeek::Saturday),
        ("tuesday", DayOfWeek::Tuesday),
        ("monday", DayOfWeek::Monday),
        ("friday", DayOfWeek::Friday),
        ("sunday", DayOfWeek::Sunday),
        ("thurs", DayOfWeek::Thursday),
        ("tues", DayOfWeek::Tuesday),
        ("tue", DayOfWeek::Tuesday),
        ("wed", DayOfWeek::Wednesday),
        ("thu", DayOfWeek::Thursday),
        ("fri", DayOfWeek::Friday),
        ("sat", DayOfWeek::Saturday),
        ("sun", DayOfWeek::Sunday),
        ("mon", DayOfWeek::Monday),
    ];

    for (name, dow) in matches {
        if let Some(len) = strip_word_prefix(s, name) {
            return Some((dow, len));
        }
    }
    None
}

pub(crate) enum RelUnit {
    Week,
    Month,
    Year,
}

pub(crate) fn parse_rel_unit(s: &str) -> Option<(RelUnit, usize)> {
    let matches = [
        ("months", RelUnit::Month),
        ("month", RelUnit::Month),
        ("weeks", RelUnit::Week),
        ("week", RelUnit::Week),
        ("years", RelUnit::Year),
        ("year", RelUnit::Year),
    ];

    for (name, u) in matches {
        if let Some(len) = strip_word_prefix(s, name) {
            return Some((u, len));
        }
    }
    None
}

#[derive(Clone, Copy)]
pub(crate) enum RelModifier {
    Last,
    Next,
    This,
}

pub(crate) fn parse_rel_modifier(s: &str) -> Option<(RelModifier, usize)> {
    let matches = [
        ("previous", RelModifier::Last),
        ("last", RelModifier::Last),
        ("past", RelModifier::Last),
        ("next", RelModifier::Next),
        ("this", RelModifier::This),
    ];

    for (name, m) in matches {
        if let Some(len) = strip_word_prefix(s, name) {
            return Some((m, len));
        }
    }
    None
}

pub(crate) fn parse_ordinal(s: &str) -> Option<(i8, usize)> {
    let matches: &[(&str, i8)] = &[
        ("first", 1),
        ("1st", 1),
        ("second", 2),
        ("2nd", 2),
        ("third", 3),
        ("3rd", 3),
        ("fourth", 4),
        ("4th", 4),
        ("fifth", 5),
        ("5th", 5),
        ("last", -1),
    ];
    for &(word, val) in matches {
        if let Some(len) = strip_word_prefix(s, word) {
            return Some((val, len));
        }
    }
    None
}

pub(crate) fn parse_month(s: &str) -> Option<(u32, usize)> {
    let matches: &[(&str, u32)] = &[
        ("january", 1),
        ("jan", 1),
        ("february", 2),
        ("feb", 2),
        ("march", 3),
        ("mar", 3),
        ("april", 4),
        ("apr", 4),
        ("may", 5),
        ("june", 6),
        ("jun", 6),
        ("july", 7),
        ("jul", 7),
        ("august", 8),
        ("aug", 8),
        ("september", 9),
        ("sept", 9),
        ("sep", 9),
        ("october", 10),
        ("oct", 10),
        ("november", 11),
        ("nov", 11),
        ("december", 12),
        ("dec", 12),
    ];
    for &(word, m) in matches {
        if let Some(len) = strip_word_prefix(s, word) {
            return Some((m, len));
        }
    }
    None
}

/// Parses an optional four-digit year preceded by whitespace (e.g. `" 2026"`). Shorter digit runs
/// and digits that start a time (`10:00`) are not years.
pub(crate) fn parse_optional_year(s: &str) -> Option<(i32, usize)> {
    let s_trim = s.trim_start();
    let ws_len = s.len() - s_trim.len();
    if ws_len == 0 {
        return None;
    }
    let end = s_trim
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(s_trim.len());
    if end == 4 {
        let next_char = s_trim[end..].chars().next();
        if next_char.is_none_or(|nc| !nc.is_alphanumeric() && nc != '_' && nc != ':')
            && let Ok(y) = s_trim[..end].parse::<i32>()
        {
            return Some((y, ws_len + end));
        }
    }
    None
}

/// Parses a day-of-month number with an optional ordinal suffix (`16`, `16th`, `1st`, `22nd`).
fn parse_day_number(s: &str) -> Option<(u32, usize)> {
    let d_end = s
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(s.len());
    if !(1..=2).contains(&d_end) {
        return None;
    }
    let day = s[..d_end].parse::<u32>().ok()?;
    let rest = &s[d_end..];
    for suffix in ["st", "nd", "rd", "th"] {
        if let Some(len) = strip_word_prefix(rest, suffix) {
            return Some((day, d_end + len));
        }
    }
    if rest
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == ':')
    {
        return None;
    }
    Some((day, d_end))
}

/// The year in which an annually recurring event falls next (on or after the reference calendar
/// date) when no year is given.
fn next_occurrence_year(ref_date: &Date, this_year: &Date) -> i32 {
    if this_year.to_epoch_days() < ref_date.to_epoch_days() {
        ref_date.year + 1
    } else {
        ref_date.year
    }
}

pub(crate) fn try_parse_nth_weekday_of_month(
    s: &str,
    ref_date: &Date,
) -> Result<Option<(Date, usize)>, AbacusError> {
    let Some((n, ord_len)) = parse_ordinal(s) else {
        return Ok(None);
    };
    let rest1 = s[ord_len..].trim_start();
    let ws1 = s[ord_len..].len() - rest1.len();
    if ws1 == 0 {
        return Ok(None);
    }
    let Some((dow, dow_len)) = parse_weekday(rest1) else {
        return Ok(None);
    };
    let rest2 = rest1[dow_len..].trim_start();
    let ws2 = rest1[dow_len..].len() - rest2.len();
    if ws2 == 0 {
        return Ok(None);
    }
    let conn = strip_word_prefix(rest2, "of").or_else(|| strip_word_prefix(rest2, "in"));
    let Some(len) = conn else {
        return Ok(None);
    };
    let rest3 = rest2[len..].trim_start();
    let conn_len = len + (rest2[len..].len() - rest3.len());
    let Some((month, month_len)) = parse_month(rest3) else {
        return Ok(None);
    };
    let rest4 = &rest3[month_len..];

    let (year, year_len) = if let Some((y, y_len)) = parse_optional_year(rest4) {
        (y, y_len)
    } else {
        // Pick this year's occurrence unless it has already passed (a nonexistent occurrence this
        // year is checked again for the chosen year below).
        let year = crate::units::events::nth_weekday_of_month(ref_date.year, month, dow, n)
            .map_or(ref_date.year, |d| next_occurrence_year(ref_date, &d));
        (year, 0)
    };

    let date = crate::units::events::nth_weekday_of_month(year, month, dow, n)?;
    let total_len = ord_len + ws1 + dow_len + ws2 + conn_len + month_len + year_len;
    Ok(Some((date, total_len)))
}

pub(crate) fn try_parse_period_boundary(s: &str, ref_date: &Date) -> Option<(Date, usize)> {
    if let Some(len) = strip_word_prefix(s, "quarter end") {
        return Some((crate::units::events::end_of_quarter(ref_date), len));
    }

    let is_end = if let Some(len) = strip_word_prefix(s, "end of") {
        Some((true, len))
    } else if let Some(len) = strip_word_prefix(s, "start of") {
        Some((false, len))
    } else {
        strip_word_prefix(s, "beginning of").map(|len| (false, len))
    };

    if let Some((is_end, prefix_len)) = is_end {
        let mut rest = s[prefix_len..].trim_start();
        let ws_after_prefix = s[prefix_len..].len() - rest.len();
        if ws_after_prefix == 0 {
            return None;
        }

        let mut article_len = 0;
        for art in ["the", "this", "current"] {
            if let Some(len) = strip_word_prefix(rest, art) {
                let after = rest[len..].trim_start();
                article_len = rest.len() - after.len();
                rest = after;
                break;
            }
        }

        if let Some(len) = strip_word_prefix(rest, "next quarter") {
            let total = prefix_len + ws_after_prefix + article_len + len;
            let date = if is_end {
                crate::units::events::end_of_next_quarter(ref_date)
            } else {
                crate::units::events::start_of_next_quarter(ref_date)
            };
            return Some((date, total));
        }

        if let Some(len) = strip_word_prefix(rest, "quarter") {
            let after_q = &rest[len..];
            let (year, year_len) = if let Some((y, y_len)) = parse_optional_year(after_q) {
                (y, y_len)
            } else {
                (ref_date.year, 0)
            };
            let q = crate::units::events::quarter_of(ref_date);
            let date = if is_end {
                crate::units::events::quarter_end_date(q, year)
            } else {
                crate::units::events::quarter_start_date(q, year)
            };
            let total = prefix_len + ws_after_prefix + article_len + len + year_len;
            return Some((date, total));
        }

        for (q_name, q_num) in [("q1", 1), ("q2", 2), ("q3", 3), ("q4", 4)] {
            if let Some(len) = strip_word_prefix(rest, q_name) {
                let after_q = &rest[len..];
                let (year, year_len) = if let Some((y, y_len)) = parse_optional_year(after_q) {
                    (y, y_len)
                } else {
                    (ref_date.year, 0)
                };
                let date = if is_end {
                    crate::units::events::quarter_end_date(q_num, year)
                } else {
                    crate::units::events::quarter_start_date(q_num, year)
                };
                let total = prefix_len + ws_after_prefix + article_len + len + year_len;
                return Some((date, total));
            }
        }

        if let Some(len) = strip_word_prefix(rest, "next month") {
            let total = prefix_len + ws_after_prefix + article_len + len;
            let date = if is_end {
                crate::units::events::end_of_next_month(ref_date)
            } else {
                Date::new(ref_date.year, ref_date.month, 1).add_months(1)
            };
            return Some((date, total));
        }

        if let Some(len) = strip_word_prefix(rest, "month") {
            let total = prefix_len + ws_after_prefix + article_len + len;
            let date = if is_end {
                crate::units::events::end_of_month(ref_date)
            } else {
                crate::units::events::start_of_month(ref_date)
            };
            return Some((date, total));
        }

        if let Some(len) = strip_word_prefix(rest, "next year") {
            let total = prefix_len + ws_after_prefix + article_len + len;
            let date = if is_end {
                Date::new(ref_date.year + 1, 12, 31)
            } else {
                crate::units::events::start_of_next_year(ref_date)
            };
            return Some((date, total));
        }

        if let Some(len) = strip_word_prefix(rest, "year") {
            let total = prefix_len + ws_after_prefix + article_len + len;
            let date = if is_end {
                crate::units::events::end_of_year(ref_date)
            } else {
                crate::units::events::start_of_year(ref_date)
            };
            return Some((date, total));
        }
    }

    None
}

/// Parses a named holiday (`christmas`, `thanksgiving 2027`). Without a year, the next occurrence
/// on or after the reference calendar date is used.
pub(crate) fn try_parse_named_holiday(s: &str, ref_date: &Date) -> Option<(Date, usize)> {
    type HolidayFn = fn(i32) -> Date;
    let holidays: &[(&str, HolidayFn)] = &[
        ("christmas eve", crate::units::events::christmas_eve),
        ("christmas day", crate::units::events::christmas),
        ("christmas", crate::units::events::christmas),
        ("xmas", crate::units::events::christmas),
        ("boxing day", crate::units::events::boxing_day),
        ("new year's eve", crate::units::events::new_years_eve),
        ("new years eve", crate::units::events::new_years_eve),
        ("new year eve", crate::units::events::new_years_eve),
        ("new year's day", crate::units::events::new_year),
        ("new years day", crate::units::events::new_year),
        ("new year's", crate::units::events::new_year),
        ("new years", crate::units::events::new_year),
        ("new year", crate::units::events::new_year),
        ("thanksgiving day", crate::units::events::thanksgiving),
        ("thanksgiving", crate::units::events::thanksgiving),
        ("black friday", crate::units::events::black_friday),
        ("cyber monday", crate::units::events::cyber_monday),
        ("halloween", crate::units::events::halloween),
        ("valentines day", crate::units::events::valentines_day),
        ("valentine's day", crate::units::events::valentines_day),
        ("valentines", crate::units::events::valentines_day),
        ("st patricks day", crate::units::events::st_patricks_day),
        ("st patrick's day", crate::units::events::st_patricks_day),
        ("st paddy's day", crate::units::events::st_patricks_day),
        ("fourth of july", crate::units::events::fourth_of_july),
        ("4th of july", crate::units::events::fourth_of_july),
        ("independence day", crate::units::events::fourth_of_july),
        ("labor day", crate::units::events::labor_day),
        ("labour day", crate::units::events::labor_day),
        ("memorial day", crate::units::events::memorial_day),
        ("martin luther king jr day", crate::units::events::mlk_day),
        ("martin luther king day", crate::units::events::mlk_day),
        ("mlk day", crate::units::events::mlk_day),
        ("presidents day", crate::units::events::presidents_day),
        ("president's day", crate::units::events::presidents_day),
        ("juneteenth", crate::units::events::juneteenth),
        ("veterans day", crate::units::events::veterans_day),
        ("remembrance day", crate::units::events::veterans_day),
        ("easter sunday", crate::units::events::easter),
        ("easter", crate::units::events::easter),
    ];

    for &(name, func) in holidays {
        if let Some(name_len) = strip_word_prefix(s, name) {
            let after_name = &s[name_len..];
            let (year, year_len) = if let Some((y, y_len)) = parse_optional_year(after_name) {
                (y, y_len)
            } else {
                (next_occurrence_year(ref_date, &func(ref_date.year)), 0)
            };
            let date = func(year);
            return Some((date, name_len + year_len));
        }
    }

    None
}

/// Builds a textual date, erroring when the month/day combination doesn't exist (`feb 30`).
fn make_textual_date(year: i32, month: u32, day: u32) -> Result<Date, AbacusError> {
    if crate::units::date::is_valid_date(year, month, day) {
        Ok(Date::new(year, month, day))
    } else {
        Err(AbacusError::InvalidDate(format!(
            "{} {day}, {year:04} is not a valid calendar date",
            crate::units::date::month_name(month)
        )))
    }
}

/// Parses textual dates such as `May 16, 2010`, `May 16th`, `16 May 2010`, `16th of May`.
/// Without a year, the reference date's year is used.
pub(crate) fn try_parse_textual_date(
    s: &str,
    ref_date: &Date,
) -> Result<Option<(Date, usize)>, AbacusError> {
    // 1. Month first: e.g. "May 16, 2010", "May 16th, 2010", "May 16 2010", "May 16"
    if let Some((month, m_len)) = parse_month(s) {
        let rest1 = s[m_len..].trim_start();
        let ws1 = s[m_len..].len() - rest1.len();
        if ws1 > 0 {
            let day = parse_ordinal(rest1)
                .filter(|(ord, _)| *ord > 0)
                .map(|(ord, len)| (ord as u32, len))
                .or_else(|| parse_day_number(rest1));
            if let Some((day, d_len)) = day
                && (1..=31).contains(&day)
            {
                let rest2 = &rest1[d_len..];
                let rest_after_comma = rest2.strip_prefix(',').unwrap_or(rest2);

                let (year, yr_len) = if let Some((y, y_len)) = parse_optional_year(rest_after_comma)
                {
                    (y, (rest2.len() - rest_after_comma.len()) + y_len)
                } else {
                    (ref_date.year, 0)
                };

                let total_len = m_len + ws1 + d_len + yr_len;
                return Ok(Some((make_textual_date(year, month, day)?, total_len)));
            }
        }
    }

    // 2. Day first: e.g. "16 May 2010", "16th May 2010", "16th of May 2010"
    let day = parse_ordinal(s)
        .filter(|(ord, _)| *ord > 0)
        .map(|(ord, len)| (ord as u32, len))
        .or_else(|| parse_day_number(s));
    let Some((day, d_len)) = day else {
        return Ok(None);
    };

    if (1..=31).contains(&day) {
        let rest1 = s[d_len..].trim_start();
        let ws1 = s[d_len..].len() - rest1.len();
        if ws1 > 0 {
            let (conn_len, rest2) = if let Some(len) = strip_word_prefix(rest1, "of") {
                let r = rest1[len..].trim_start();
                (len + (rest1[len..].len() - r.len()), r)
            } else {
                (0, rest1)
            };

            if let Some((month, m_len)) = parse_month(rest2) {
                let rest3 = &rest2[m_len..];
                let rest_after_comma = rest3.strip_prefix(',').unwrap_or(rest3);

                let (year, yr_len) = if let Some((y, y_len)) = parse_optional_year(rest_after_comma)
                {
                    (y, (rest3.len() - rest_after_comma.len()) + y_len)
                } else {
                    (ref_date.year, 0)
                };

                let total_len = d_len + ws1 + conn_len + m_len + yr_len;
                return Ok(Some((make_textual_date(year, month, day)?, total_len)));
            }
        }
    }

    Ok(None)
}

pub(crate) fn try_parse_event_date(
    s: &str,
    ref_date: &Date,
) -> Result<Option<(Date, usize)>, AbacusError> {
    if let Some(res) = try_parse_textual_date(s, ref_date)? {
        return Ok(Some(res));
    }
    if let Some(res) = try_parse_nth_weekday_of_month(s, ref_date)? {
        return Ok(Some(res));
    }
    if let Some(res) = try_parse_period_boundary(s, ref_date) {
        return Ok(Some(res));
    }
    Ok(try_parse_named_holiday(s, ref_date))
}

/// Parses relative date keywords against `ref_date`:
///
/// * `today`/`tdy`, `tomorrow`/`tmr`, `yesterday` — midnight of that calendar day.
/// * `now` — the reference instant itself.
/// * `next <weekday>` / `last <weekday>` — strictly after / before the reference day (on a
///   Wednesday, `next thursday` is tomorrow and `next wednesday` is a week away).
/// * `this <weekday>` — that day within the reference day's Monday-to-Sunday week (may be past).
/// * bare `<weekday>` — the next such day, or today if it already is that day.
/// * `next/last week|month|year` — the reference date shifted by one unit (calendar-aware).
///
/// When `after_quantity` is set (the previous token is a number or quantity) bare weekdays are
/// not recognised so that e.g. `1 sun` can refer to a unit.
pub(crate) fn try_parse_relative_date_keyword(
    s: &str,
    ref_date: &Date,
    after_quantity: bool,
) -> Option<(Date, usize)> {
    let today_dow = ref_date.day_of_week() as i64;
    let midnight = ref_date.start_of_day();

    // 1. Check modifier + space + subject
    if let Some((modifier, mod_len)) = parse_rel_modifier(s) {
        let rest = s[mod_len..].trim_start();
        let ws_len = s[mod_len..].len() - rest.len();
        if ws_len > 0 {
            if let Some((target_dow, sub_len)) = parse_weekday(rest) {
                let target = target_dow as i64;
                let days_offset: i64 = match modifier {
                    RelModifier::Last => -((today_dow - target + 6).rem_euclid(7) + 1),
                    RelModifier::Next => (target - today_dow + 6).rem_euclid(7) + 1,
                    RelModifier::This => target - today_dow,
                };
                return Some((midnight.add_days(days_offset), mod_len + ws_len + sub_len));
            }

            if let Some((unit, sub_len)) = parse_rel_unit(rest) {
                let date = match (modifier, unit) {
                    (RelModifier::Last, RelUnit::Week) => midnight.add_days(-7),
                    (RelModifier::Last, RelUnit::Month) => midnight.add_months(-1),
                    (RelModifier::Last, RelUnit::Year) => midnight.add_years(-1),
                    (RelModifier::Next, RelUnit::Week) => midnight.add_days(7),
                    (RelModifier::Next, RelUnit::Month) => midnight.add_months(1),
                    (RelModifier::Next, RelUnit::Year) => midnight.add_years(1),
                    (RelModifier::This, _) => midnight.clone(),
                };
                return Some((date, mod_len + ws_len + sub_len));
            }
        }
    }

    // 2. Standalone relative keywords
    for (word, days) in [
        ("today", 0),
        ("tdy", 0),
        ("tomorrow", 1),
        ("tmr", 1),
        ("yesterday", -1),
    ] {
        if let Some(len) = strip_word_prefix(s, word) {
            return Some((midnight.add_days(days), len));
        }
    }
    if let Some(len) = strip_word_prefix(s, "now") {
        return Some((ref_date.clone(), len));
    }

    // 3. Bare weekday
    if !after_quantity && let Some((target_dow, sub_len)) = parse_weekday(s) {
        let days_offset = (target_dow as i64 - today_dow).rem_euclid(7);
        return Some((midnight.add_days(days_offset), sub_len));
    }

    None
}

/// Splits an optional leading `at ` from `rest`, returning `(has_at, remainder, skipped_len)`.
fn strip_at(rest: &str) -> (bool, &str, usize) {
    if rest.len() >= 3
        && rest.as_bytes()[..2].eq_ignore_ascii_case(b"at")
        && rest.as_bytes()[2].is_ascii_whitespace()
    {
        let r = rest[2..].trim_start();
        (true, r, rest.len() - r.len())
    } else {
        (false, rest, 0)
    }
}

/// Parses an optional timezone word following a time at `remaining[end_idx..]`. Alphabetic
/// abbreviations (`EST`) and unambiguous numeric offsets (`+05:30`, `-0400`, `UTC+2`) are
/// accepted; a bare `-3` is left alone so `10:00 -3 hours` stays arithmetic.
fn parse_trailing_timezone(remaining: &str, end_idx: usize, date: &mut Date) -> usize {
    let after = remaining[end_idx..].trim_start();
    let skipped = remaining[end_idx..].len() - after.len();
    if skipped == 0 {
        return end_idx;
    }
    let word_len = after
        .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | ',' | '*' | '/'))
        .unwrap_or(after.len());
    let word = &after[..word_len];
    let accepted =
        if word.bytes().all(|b| b.is_ascii_alphabetic()) || TimeZone::is_literal_offset(word) {
            TimeZone::parse(word).ok()
        } else {
            None
        };
    if let Some(tz) = accepted {
        date.timezone = Some(tz);
        end_idx + skipped + word_len
    } else {
        end_idx
    }
}

/// Parses an optional `[at] <time> [tz]` suffix after a date phrase ending at `end_idx`.
fn parse_trailing_time(remaining: &str, end_idx: usize, date: &mut Date) -> usize {
    let rest = remaining[end_idx..].trim_start();
    let skipped_ws = remaining[end_idx..].len() - rest.len();
    if skipped_ws == 0 && !rest.is_empty() {
        return end_idx;
    }
    let (has_at, time_rest, at_len) = strip_at(rest);
    if let Some((time, time_len, meridiem)) = Time::parse_time_spec_ext(time_rest, has_at) {
        date.time = time;
        if meridiem {
            date.flags |= FLAG_EXPLICIT_MERIDIEM;
        }
        parse_trailing_timezone(remaining, end_idx + skipped_ws + at_len + time_len, date)
    } else {
        end_idx
    }
}

/// Lexing context for date literals.
#[derive(Clone, Copy, Default)]
pub(crate) struct DateLexOptions<'d> {
    /// Reference "now" for relative expressions; the system clock is read when absent.
    pub anchor: Option<&'d Date>,
    /// Read ambiguous `NN-NN-YYYY` dates as month-day.
    pub month_first: bool,
    /// The previous token is a number or quantity.
    pub after_quantity: bool,
}

/// Attempts to lex a date/time literal at the start of `remaining`.
///
/// Returns `Ok(None)` if the input isn't a date literal, and `Err` for inputs that are clearly
/// meant as dates but are invalid (`2026-02-30`, `feb 30`, `fifth monday of feb 2026`).
pub(crate) fn try_parse_date_literal_with_anchor(
    remaining: &str,
    opts: DateLexOptions<'_>,
) -> Result<Option<(Date, usize)>, AbacusError> {
    if let Some(stripped) = remaining.strip_prefix('@') {
        // `@...@` is an explicit date literal only when it has content; `@@` and friends stay
        // available as custom operator symbols.
        if let Some(end_idx) = stripped.find('@')
            && stripped[..end_idx].starts_with(|c: char| c.is_ascii_alphanumeric())
        {
            let date = stripped[..end_idx].parse::<Date>()?;
            return Ok(Some((date, end_idx + 2)));
        }
        return Ok(None);
    }

    let starts_alpha = remaining.starts_with(|c: char| c.is_ascii_alphabetic());
    let maybe_phrase = could_start_date_phrase(remaining);
    if starts_alpha && !maybe_phrase {
        return Ok(None);
    }

    // Only read the clock once a date phrase is plausible and no anchor was supplied.
    let fallback;
    let ref_date = if let Some(a) = opts.anchor {
        a
    } else {
        fallback = Date::now();
        &fallback
    };

    if maybe_phrase || remaining.starts_with(|c: char| c.is_ascii_digit()) {
        // Event dates (e.g. "third thursday of november 2026", "end of quarter", "christmas")
        if let Some((mut date, event_len)) = try_parse_event_date(remaining, ref_date)? {
            if ref_date.timezone.is_some() && date.timezone.is_none() {
                date.timezone = ref_date.timezone.clone();
            }
            let end_idx = parse_trailing_time(remaining, event_len, &mut date);
            return Ok(Some((date, end_idx)));
        }
    }

    if maybe_phrase
        && let Some((mut date, kw_len)) =
            try_parse_relative_date_keyword(remaining, ref_date, opts.after_quantity)
    {
        let end_idx = parse_trailing_time(remaining, kw_len, &mut date);
        return Ok(Some((date, end_idx)));
    }

    if starts_alpha {
        return Ok(None);
    }

    // Standalone time literal: e.g. "3pm", "3 PM", "15:00" on the reference date
    if let Some((time, time_len, meridiem)) = Time::parse_time_spec_ext(remaining, false) {
        let mut date = ref_date.start_of_day();
        date.time = time;
        date.flags = FLAG_TIME_ONLY | if meridiem { FLAG_EXPLICIT_MERIDIEM } else { 0 };
        let end_idx = parse_trailing_timezone(remaining, time_len, &mut date);
        return Ok(Some((date, end_idx)));
    }

    // Numeric date literals (e.g. YYYY-MM-DD, DD-MM-YYYY)
    if let Some((y, m, d, date_len)) = Date::parse_ymd_components_with(remaining, opts.month_first)?
    {
        let mut date = Date::new(y, m, d);
        let mut end_idx = date_len;
        if remaining[end_idx..].starts_with(['T', 't'])
            && let Some((time, time_len, meridiem)) =
                Time::parse_time_spec_ext(&remaining[end_idx + 1..], false)
        {
            date.time = time;
            if meridiem {
                date.flags |= FLAG_EXPLICIT_MERIDIEM;
            }
            end_idx += 1 + time_len;
            if remaining[end_idx..].starts_with(['Z', 'z'])
                && !remaining[end_idx + 1..].starts_with(|c: char| c.is_alphanumeric())
            {
                date.timezone = Some(TimeZone::utc());
                end_idx += 1;
            } else {
                end_idx = parse_trailing_timezone(remaining, end_idx, &mut date);
            }
        } else {
            let before = end_idx;
            end_idx = parse_trailing_time(remaining, end_idx, &mut date);
            if end_idx == before {
                // A timezone may follow a bare date (e.g. "07-08-2026 EST").
                end_idx = parse_trailing_timezone(remaining, end_idx, &mut date);
            }
        }
        return Ok(Some((date, end_idx)));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_word_table_is_sorted() {
        assert!(DATE_PHRASE_FIRST_WORDS.windows(2).all(|w| w[0] < w[1]));
        assert!(DATE_PHRASE_FIRST_WORDS.iter().all(|w| w.len() <= 12));
    }

    #[test]
    fn prefilter_matches_phrase_starts_only() {
        assert!(could_start_date_phrase("Christmas"));
        assert!(could_start_date_phrase("valentine's day"));
        assert!(could_start_date_phrase("4th of july"));
        assert!(!could_start_date_phrase("meters"));
        assert!(!could_start_date_phrase("sqrt(2)"));
    }
}
