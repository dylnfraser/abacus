use abacus::units::date::{
    Date, DayOfWeek, Time, date_to_epoch_days, days_in_month, epoch_days_to_date, is_leap_year,
    is_valid_date,
};
use abacus::{AbacusError, UnitRegistry};
use std::str::FromStr;

#[test]
fn test_time_creation_validation_and_formatting() {
    let t1 = Time::new(14, 30, 0, 0);
    assert!(t1.is_valid());
    assert_eq!(t1.format(), "14:30:00");
    assert_eq!(t1.to_string(), "14:30:00");

    let t2 = Time::new(9, 5, 7, 123);
    assert!(t2.is_valid());
    assert_eq!(t2.format(), "09:05:07.123");

    let invalid_time = Time::new(25, 60, 60, 1000);
    assert!(!invalid_time.is_valid());
}

#[test]
fn test_leap_years_and_days_in_month() {
    assert!(is_leap_year(2000));
    assert!(!is_leap_year(1900));
    assert!(is_leap_year(2024));
    assert!(!is_leap_year(2025));

    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2025, 2), 28);
    assert_eq!(days_in_month(2026, 1), 31);
    assert_eq!(days_in_month(2026, 4), 30);
}

#[test]
fn test_date_creation_and_validation() {
    let valid_d = Date::new(2026, 8, 7);
    assert!(valid_d.is_valid());

    let leap_d = Date::new(2024, 2, 29);
    assert!(leap_d.is_valid());

    let invalid_leap = Date::new(2025, 2, 29);
    assert!(!invalid_leap.is_valid());

    assert!(!is_valid_date(2026, 13, 1));
    assert!(!is_valid_date(2026, 0, 10));
    assert!(!is_valid_date(2026, 5, 32));
}

#[test]
fn test_day_of_week_and_year() {
    // 2026-08-07 is Friday
    let d = Date::new(2026, 8, 7);
    assert_eq!(d.day_of_week(), DayOfWeek::Friday);
    assert_eq!(d.day_of_week().name(), "Friday");

    // 2026-01-01 is Day 1 of year
    let d_start = Date::new(2026, 1, 1);
    assert_eq!(d_start.day_of_year(), 1);

    // 2026-12-31 is Day 365 of non-leap year
    let d_end = Date::new(2026, 12, 31);
    assert_eq!(d_end.day_of_year(), 365);

    // 2024-12-31 is Day 366 of leap year
    let leap_end = Date::new(2024, 12, 31);
    assert_eq!(leap_end.day_of_year(), 366);
}

#[test]
fn test_epoch_conversions_roundtrip() {
    let epoch = Date::new(1970, 1, 1);
    assert_eq!(epoch.to_epoch_days(), 0);
    assert_eq!(epoch.to_epoch_milliseconds(), 0);
    assert_eq!(Date::from_epoch_days(0), epoch);

    let test_dates = vec![
        (1600, 1, 1),
        (1900, 2, 28),
        (1970, 1, 1),
        (2000, 2, 29),
        (2026, 8, 7),
        (2099, 12, 31),
    ];

    for (y, m, d) in test_dates {
        let epoch_days = date_to_epoch_days(y, m, d);
        let (ry, rm, rd) = epoch_days_to_date(epoch_days);
        assert_eq!((y, m, d), (ry, rm, rd), "Roundtrip failed for {y}-{m}-{d}");
    }
}

#[test]
fn test_date_arithmetic_days_months_years() {
    let d = Date::new(2026, 8, 7);

    // Add/sub days
    assert_eq!(d.add_days(10), Date::new(2026, 8, 17));
    assert_eq!(d.sub_days(7), Date::new(2026, 7, 31));

    // Add months with clamping
    let d_jan = Date::new(2024, 1, 31);
    assert_eq!(d_jan.add_months(1), Date::new(2024, 2, 29)); // Leap year Feb

    let d_jan_non_leap = Date::new(2025, 1, 31);
    assert_eq!(d_jan_non_leap.add_months(1), Date::new(2025, 2, 28)); // Non-leap year Feb

    // Add years
    let leap_bday = Date::new(2024, 2, 29);
    assert_eq!(leap_bday.add_years(1), Date::new(2025, 2, 28));
    assert_eq!(leap_bday.add_years(4), Date::new(2028, 2, 29));
}

#[test]
fn test_date_time_overflow_across_midnight() {
    let d = Date::new_with_hms(2026, 8, 7, 23, 30, 0);

    // Adding 1 hour crosses midnight to next day 00:30:00
    let d_next = d.add_hours(1);
    assert_eq!(d_next, Date::new_with_hms(2026, 8, 8, 0, 30, 0));

    // Adding 48 hours moves 2 days forward
    let d_2days = d.add_hours(48);
    assert_eq!(d_2days, Date::new_with_hms(2026, 8, 9, 23, 30, 0));
}

#[test]
fn test_date_difference() {
    let d1 = Date::new(2026, 8, 7);
    let d2 = Date::new(2026, 8, 17);

    assert_eq!(d1.days_between(&d2), 10);
    assert_eq!(d2.days_between(&d1), -10);
    assert_eq!(d1.seconds_between(&d2), 10 * 86_400);

    // Operator subtraction: d2 - d1 -> 10 days (864,000 s), shown in days
    let diff_val = &d2 - &d1;
    assert_eq!(diff_val.canonical, 864_000.0);
    assert_eq!(diff_val.to_display(), "10 d");
}

#[test]
fn test_date_and_value_arithmetic() {
    let reg = UnitRegistry::standard();
    let d = Date::new(2026, 8, 7);

    let five_days = reg.value(5.0, "day").unwrap();
    let d_plus_5 = (&d + &five_days).unwrap();
    assert_eq!(d_plus_5, Date::new(2026, 8, 12));

    let three_hours = reg.value(3.0, "h").unwrap();
    let d_plus_3h = (&d + &three_hours).unwrap();
    assert_eq!(d_plus_3h, Date::new_with_hms(2026, 8, 7, 3, 0, 0));

    let two_days = reg.value(2.0, "day").unwrap();
    let d_minus_2 = (&d - &two_days).unwrap();
    assert_eq!(d_minus_2, Date::new(2026, 8, 5));

    // Adding non-time value should fail
    let five_meters = reg.value(5.0, "m").unwrap();
    let err = (&d + &five_meters).unwrap_err();
    assert_eq!(err, AbacusError::IncompatibleDimensions);
}

#[test]
fn test_date_parsing_iso() {
    let d1 = Date::from_str("2026-08-07").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 7));
    assert_eq!(d1.to_string(), "August 7, 2026");
    assert_eq!(
        d1.format_with_style(abacus::DateFormat::YYYYMMDD),
        "2026-08-07"
    );

    let d2 = Date::from_str("2026-08-07 10:54:49").unwrap();
    assert_eq!(d2, Date::new_with_hms(2026, 8, 7, 10, 54, 49));
    assert_eq!(d2.to_string(), "August 7, 2026 10:54:49");

    let d3 = Date::from_str("2026-08-07T10:54:49.123").unwrap();
    assert_eq!(d3, Date::new_full(2026, 8, 7, 10, 54, 49, 123));

    // Invalid dates return error
    assert!(Date::from_str("invalid-date").is_err());
    assert!(Date::from_str("2025-02-29").is_err());
}

#[test]
fn test_mm_dd_yyyy_and_slash_date_parsing() {
    let d1 = Date::from_str("05-16-2010").unwrap();
    assert_eq!(d1, Date::new(2010, 5, 16));

    let d2 = Date::from_str("05/16/2010").unwrap();
    assert_eq!(d2, Date::new(2010, 5, 16));

    let d3 = Date::from_str("16-05-2010").unwrap();
    assert_eq!(d3, Date::new(2010, 5, 16));

    let d4 = Date::from_str("16/05/2010").unwrap();
    assert_eq!(d4, Date::new(2010, 5, 16));

    let d5 = Date::from_str("5-16-2010").unwrap();
    assert_eq!(d5, Date::new(2010, 5, 16));

    let d6 = Date::from_str("5/16/2010").unwrap();
    assert_eq!(d6, Date::new(2010, 5, 16));
}
