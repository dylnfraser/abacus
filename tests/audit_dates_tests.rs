//! Regression tests for the date/time audit fixes. Every test uses a fixed anchor (or explicit
//! timezones) so results never depend on the wall clock or the machine's local zone.

use abacus::{Abacus, AbacusError, Date, DateFormat, DayOfWeek, TimeZone};
use std::str::FromStr;

/// Wednesday 2026-10-21 09:30 (floating local time).
fn anchor() -> Date {
    Date::new_full(2026, 10, 21, 9, 30, 0, 0)
}

fn calc() -> Abacus {
    Abacus::standard().with_now(anchor())
}

fn date(calc: &Abacus, expr: &str) -> Date {
    calc.eval_date(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
}

fn display(calc: &Abacus, expr: &str) -> String {
    calc.eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .to_display()
}

fn ymd_hm(d: &Date) -> (i32, u32, u32, u32, u32) {
    (d.year, d.month, d.day, d.time.hour, d.time.minute)
}

fn is_invalid_date<T: std::fmt::Debug>(r: Result<T, AbacusError>) -> bool {
    matches!(r, Err(AbacusError::InvalidDate(_)))
}

// Finding 1: month/year durations
#[test]
fn month_and_year_are_calendar_durations() {
    let c = calc();
    assert_eq!(display(&c, "1 month in days"), "30.436875 d");
    assert_eq!(display(&c, "1 year in days"), "365.2425 d");
    assert_eq!(display(&c, "2 years"), "2 yr");
    assert_eq!(
        ymd_hm(&date(&c, "2026-01-31 + 1 month")),
        (2026, 2, 28, 0, 0)
    );
    assert_eq!(
        ymd_hm(&date(&c, "2024-02-29 + 1 year")),
        (2025, 2, 28, 0, 0)
    );
    assert_eq!(
        ymd_hm(&date(&c, "2026-03-31 - 1 month")),
        (2026, 2, 28, 0, 0)
    );
    assert_eq!(
        ymd_hm(&date(&c, "2026-01-15 + 3 months")),
        (2026, 4, 15, 0, 0)
    );
    assert_eq!(ymd_hm(&date(&c, "today + 1 year")), (2027, 10, 21, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "3 months ago")), (2026, 7, 21, 9, 30));
    assert_eq!(ymd_hm(&date(&c, "1 yr from now")), (2027, 10, 21, 9, 30));
    // month()/year() functions still work on dates
    assert_eq!(c.eval_scalar("month(07-08-2026)").unwrap().canonical, 8.0);
    assert_eq!(c.eval_scalar("year(07-08-2026)").unwrap().canonical, 2026.0);
    assert!(is_invalid_date(c.eval("2026-01-15 + 1.5 months")));
}

// Findings 2-4: now/today, default timezone conversion
#[test]
fn now_is_the_instant_and_today_is_midnight() {
    let c = calc();
    assert_eq!(ymd_hm(&date(&c, "now")), (2026, 10, 21, 9, 30));
    assert_eq!(ymd_hm(&date(&c, "now + 2 hours")), (2026, 10, 21, 11, 30));
    assert_eq!(ymd_hm(&date(&c, "in 2 hours")), (2026, 10, 21, 11, 30));
    assert_eq!(ymd_hm(&date(&c, "today")), (2026, 10, 21, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "tomorrow")), (2026, 10, 22, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "yesterday")), (2026, 10, 20, 0, 0));
}

#[test]
fn default_timezone_converts_the_reference_instant() {
    let utc_anchor = Date::new_full(2026, 10, 21, 15, 0, 0, 0).with_timezone(TimeZone::utc());
    let est = TimeZone::parse("EST").unwrap();
    let c = Abacus::standard()
        .with_now(utc_anchor.clone())
        .with_default_timezone(est.clone());
    let now = date(&c, "now");
    assert_eq!(ymd_hm(&now), (2026, 10, 21, 10, 0));
    assert_eq!(now.timezone.as_ref().unwrap().name, "EST");
    assert_eq!(ymd_hm(&date(&c, "in 1 hour")), (2026, 10, 21, 11, 0));
    assert_eq!(ymd_hm(&date(&c, "today")), (2026, 10, 21, 0, 0));

    // Without a default timezone, `now in EST` converts the anchored instant.
    let plain = Abacus::standard().with_now(utc_anchor);
    assert_eq!(ymd_hm(&date(&plain, "now in EST")), (2026, 10, 21, 10, 0));
    assert_eq!(ymd_hm(&date(&plain, "now in JST")), (2026, 10, 22, 0, 0));
}

#[test]
fn system_clock_helpers_are_consistent() {
    let local = TimeZone::local();
    assert!(local.offset_minutes.abs() <= TimeZone::MAX_OFFSET_MINUTES);
    let now = Date::now();
    let today = Date::today();
    assert_eq!(today.time, abacus::Time::default());
    assert!((now.to_epoch_days() - today.to_epoch_days()).abs() <= 1);
    assert_eq!(
        Date::now_utc().timezone.unwrap().offset_minutes,
        0,
        "now_utc is labelled UTC"
    );
}

// Finding 5: invalid numeric literals are errors, not subtraction
#[test]
fn invalid_numeric_dates_are_errors() {
    let c = calc();
    for expr in [
        "2026-02-30",
        "32-01-2026",
        "00-00-0000",
        "1900-02-29",
        "2026-13-01",
    ] {
        assert!(is_invalid_date(c.eval(expr)), "{expr}");
    }
    assert_eq!(c.eval_scalar("12-6-3").unwrap().canonical, 3.0);
    assert_eq!(ymd_hm(&date(&c, "2000-02-29")), (2000, 2, 29, 0, 0));
}

// Findings 6-7: time spans and rollover
#[test]
fn time_spans_roll_over_sensibly() {
    let c = calc();
    assert_eq!(display(&c, "today at 12 to 1"), "1 h");
    assert_eq!(display(&c, "today at 12:00 to 3:00"), "3 h");
    assert_eq!(display(&c, "11pm to 1am"), "2 h");
    assert_eq!(display(&c, "23:00 to 01:00"), "2 h");
    assert_eq!(display(&c, "13:00 to 09:00"), "20 h");
    assert_eq!(display(&c, "today at 11am to 1am"), "14 h");
    assert_eq!(display(&c, "today at 11:30 AM to 2:15 PM"), "2.75 h");
    // Explicit full dates never roll over.
    assert_eq!(display(&c, "2026-08-07 10:00 to 2026-08-07 09:00"), "-1 h");
}

// Finding 8: equality/ordering by instant; holidays by calendar date
#[test]
fn dates_compare_by_instant() {
    let a = Date::from_str("2026-08-07 10:00 EST").unwrap();
    let b = Date::from_str("2026-08-07 15:00 UTC").unwrap();
    assert_eq!(a, b);
    let mut set = std::collections::HashSet::new();
    set.insert(a.clone());
    assert!(set.contains(&b));

    let earlier = Date::from_str("2026-08-07 10:00 UTC").unwrap();
    let later = Date::from_str("2026-08-07 09:00 EST").unwrap(); // 14:00 UTC
    assert!(earlier < later);
}

#[test]
fn holiday_on_the_day_is_this_year() {
    let c = Abacus::standard().with_now(Date::new_full(2026, 12, 25, 9, 30, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "christmas")), (2026, 12, 25, 0, 0));
    assert_eq!(display(&c, "days until christmas"), "0 d");
    let c2 = Abacus::standard().with_now(Date::new_full(2026, 11, 26, 18, 0, 0, 0));
    assert_eq!(ymd_hm(&date(&c2, "thanksgiving")), (2026, 11, 26, 0, 0));
}

// Findings 9-10: anchor date for standalone times; midnight for day keywords
#[test]
fn standalone_times_and_weekdays_use_the_anchor() {
    let c = Abacus::standard().with_now(Date::new_full(2020, 6, 15, 9, 30, 0, 0)); // Monday
    assert_eq!(ymd_hm(&date(&c, "3pm")), (2020, 6, 15, 15, 0));
    assert_eq!(ymd_hm(&date(&c, "15:00")), (2020, 6, 15, 15, 0));
    assert_eq!(ymd_hm(&date(&c, "today")), (2020, 6, 15, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "friday")), (2020, 6, 19, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "monday")), (2020, 6, 15, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "next monday")), (2020, 6, 22, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "last monday")), (2020, 6, 8, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "this sunday")), (2020, 6, 21, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "tomorrow at 3pm")), (2020, 6, 16, 15, 0));
}

// Finding 11: textual dates followed by a time
#[test]
fn textual_dates_followed_by_times() {
    let c = calc();
    assert_eq!(ymd_hm(&date(&c, "May 16 10:00")), (2026, 5, 16, 10, 0));
    assert_eq!(ymd_hm(&date(&c, "may 16, 2027 3pm")), (2027, 5, 16, 15, 0));
    assert_eq!(ymd_hm(&date(&c, "christmas 10:00")), (2026, 12, 25, 10, 0));
    assert_eq!(ymd_hm(&date(&c, "16th May 2010")), (2010, 5, 16, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "16 May 2010")), (2010, 5, 16, 0, 0));
    // Two-digit numbers are not years.
    assert!(!matches!(c.eval_date("christmas 26"), Ok(d) if d.year == 26));
    let parsed = Date::from_str("May 16 10:00").unwrap();
    assert_eq!((parsed.month, parsed.day, parsed.time.hour), (5, 16, 10));
    assert_ne!(parsed.year, 10);
}

// Finding 12: DateFormat controls ambiguous NN-NN-YYYY input
#[test]
fn ambiguous_dates_follow_the_configured_format() {
    let us = Abacus::standard().set_date_format(DateFormat::MMDDYYYY);
    let d = us.eval_date("08-07-2026").unwrap();
    assert_eq!((d.month, d.day), (8, 7));
    assert_eq!(us.eval("08-07-2026").unwrap().to_display(), "08-07-2026");

    let default = Abacus::standard();
    let d = default.eval_date("08-07-2026").unwrap();
    assert_eq!((d.month, d.day), (7, 8));

    // Unambiguous inputs are read the only valid way in either mode.
    assert_eq!(us.eval_date("13-01-2026").unwrap().month, 1);
    assert_eq!(default.eval_date("01-13-2026").unwrap().day, 13);
}

// Finding 13: AM/PM after a space
#[test]
fn spaced_meridiem_times() {
    let c = calc();
    assert_eq!(ymd_hm(&date(&c, "3 PM")), (2026, 10, 21, 15, 0));
    assert_eq!(ymd_hm(&date(&c, "3 pm")), (2026, 10, 21, 15, 0));
    assert_eq!(ymd_hm(&date(&c, "12 AM")), (2026, 10, 21, 0, 0));
    assert_eq!(ymd_hm(&date(&c, "12 PM")), (2026, 10, 21, 12, 0));
    assert_eq!(ymd_hm(&date(&c, "9:15 am")), (2026, 10, 21, 9, 15));
}

// Finding 14: offsets
#[test]
fn timezone_offsets_are_validated() {
    assert_eq!(TimeZone::parse("+0530").unwrap().offset_minutes, 330);
    assert_eq!(TimeZone::parse("-04:00").unwrap().offset_minutes, -240);
    assert_eq!(TimeZone::parse("UTC+2").unwrap().offset_minutes, 120);
    for bad in ["+99", "+05:-30", "+15:00", "+05:60", "+5:3", "+123"] {
        assert!(TimeZone::parse(bad).is_err(), "{bad}");
    }
    let c = calc();
    assert_eq!(
        ymd_hm(&date(&c, "2026-08-07 10:00 -3 hours")),
        (2026, 8, 7, 7, 0)
    );
    let d = date(&c, "2026-08-07 10:00 +0530 in UTC");
    assert_eq!(ymd_hm(&d), (2026, 8, 7, 4, 30));
}

// Finding 15: additional abbreviations
#[test]
fn additional_timezone_abbreviations() {
    for (name, off) in [
        ("AEDT", 660),
        ("NZDT", 780),
        ("SGT", 480),
        ("WET", 0),
        ("IST", 330),
        ("CST", -360),
        ("BST", 60),
    ] {
        assert_eq!(TimeZone::parse(name).unwrap().offset_minutes, off, "{name}");
    }
}

// Finding 16: strict parsing
#[test]
fn from_str_rejects_trailing_garbage_and_bad_times() {
    assert!(Date::from_str("2026-01-01 10:00 garbage").is_err());
    assert!(Date::from_str("2026-01-01 25:00").is_err());
    assert!(Date::from_str("2026-01-01 10:61").is_err());
    let tz_only = Date::from_str("2026-01-01 EST").unwrap();
    assert_eq!(tz_only.timezone.unwrap().name, "EST");
    let c = calc();
    assert!(c.eval("@2026-08-07 10:00 garbage@").is_err());
    let d = date(&c, "07-08-2026 EST");
    assert_eq!(d.timezone.unwrap().name, "EST");
    let iso = date(&c, "2026-08-07T10:00:00Z");
    assert_eq!(ymd_hm(&iso), (2026, 8, 7, 10, 0));
    assert_eq!(iso.timezone.unwrap().offset_minutes, 0);
}

// Finding 17: no silent truncation/saturation
#[test]
fn date_arguments_and_shifts_are_validated() {
    let c = calc();
    assert!(is_invalid_date(c.eval("date(2026, 1.9, 5)")));
    assert!(is_invalid_date(c.eval("date(1e12, 1, 1)")));
    assert!(is_invalid_date(c.eval("date(2026, 2, 30)")));
    assert!(is_invalid_date(c.eval("07-08-2026 + 1e300 days")));
    assert!(is_invalid_date(c.eval("07-08-2026 + 1e12 bdays")));
    assert!(is_invalid_date(c.eval("1e300 s ago")));
    assert!(is_invalid_date(c.eval("07-08-2026 + 0.5 bdays")));
    assert!(is_invalid_date(c.eval("07-08-2026 + 0.4 bdays")));
    assert_eq!(
        ymd_hm(&date(&c, "date(2026, 8, 7, 10, 54, 49)")),
        (2026, 8, 7, 10, 54)
    );
}

// Finding 18: durations in sensible units
#[test]
fn date_differences_use_sensible_units() {
    let c = calc();
    assert_eq!(display(&c, "2026-03-01 - 2026-02-01"), "28 d");
    assert_eq!(display(&c, "07-08-2026 to 17-08-2026"), "10 d");
    assert_eq!(display(&c, "2026-08-07 12:00 - 2026-08-07 09:30"), "2.5 h");
    assert_eq!(display(&c, "2026-08-07 09:45 - 2026-08-07 09:30"), "15 min");
    assert_eq!(
        display(&c, "07-08-2026 10:00 AM EST - 07-08-2026 07:00 AM PST"),
        "0 s"
    );
    let err = c.eval("(07-08-2026 + 1 bday) in days").unwrap_err();
    assert!(
        !format!("{err:?}").contains("unknown timezone"),
        "unhelpful error: {err:?}"
    );
}

// Finding 19: workdays() counts (start, end]
#[test]
fn workdays_counts_end_but_not_start() {
    let c = calc();
    // Mon 19 Oct .. Fri 23 Oct 2026
    assert_eq!(
        c.eval_scalar("workdays(19-10-2026, 23-10-2026)")
            .unwrap()
            .canonical
            / 86_400.0,
        4.0
    );
    assert_eq!(
        ymd_hm(&date(&c, "19-10-2026 + 4 business days")),
        (2026, 10, 23, 0, 0)
    );
    assert_eq!(
        ymd_hm(&date(&c, "3 business days ago")),
        (2026, 10, 16, 9, 30)
    );
}

// Finding 20: weekday abbreviations don't hijack units after a number
#[test]
fn weekday_abbreviations_after_numbers_are_not_dates() {
    let c = calc();
    assert!(!matches!(c.eval("1 sun"), Ok(abacus::EvalResult::Date(_))));
    assert!(!matches!(
        c.eval("1 sun"),
        Err(AbacusError::UnexpectedToken(_))
    ));
    assert_eq!(date(&c, "sun").day_of_week(), DayOfWeek::Sunday);
}

// Finding 22: misc
#[test]
fn nonexistent_textual_dates_are_errors() {
    let c = calc();
    assert!(is_invalid_date(c.eval("fifth monday of feb 2026")));
    assert!(is_invalid_date(c.eval("feb 29 2027")));
    assert!(is_invalid_date(c.eval("31 april 2026")));
    assert_eq!(ymd_hm(&date(&c, "feb 29 2028")), (2028, 2, 29, 0, 0));
    assert_eq!(
        ymd_hm(&date(&c, "fifth thursday of oct 2026")),
        (2026, 10, 29, 0, 0)
    );
}

#[test]
fn default_and_negative_year_display() {
    let d = Date::default();
    assert!(d.is_valid());
    assert_eq!(d.format_iso(), "1970-01-01");
    assert_eq!(Date::new(-44, 3, 15).format_iso(), "-0044-03-15");
    assert_eq!(Date::new(-44, 3, 15).to_string(), "March 15, -0044");
}

#[test]
fn format_functions_are_not_overridden() {
    let c = calc();
    assert_eq!(display(&c, "format_iso(07-08-2026)"), "2026-08-07");
    assert_eq!(display(&c, "format_dmy(07-08-2026)"), "07-08-2026");
    assert_eq!(display(&c, "format_mdy(07-08-2026)"), "08-07-2026");
    assert_eq!(display(&c, "format_date(07-08-2026)"), "August 7, 2026");
    let iso = Abacus::standard().set_date_format(DateFormat::YYYYMMDD);
    assert_eq!(
        iso.eval("format_long(07-08-2026)").unwrap().to_display(),
        "August 7, 2026"
    );
}
