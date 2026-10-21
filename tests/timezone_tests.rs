// Helpers and imports are only used by tests gated on default features.
#![cfg_attr(
    not(all(
        feature = "units",
        feature = "stats",
        feature = "distributions",
        feature = "date",
        feature = "financial",
        feature = "number-scales",
        feature = "currencies"
    )),
    allow(unused)
)]
use abacus::{Abacus, Date, Time, TimeZone};
use std::str::FromStr;

#[test]
fn test_am_pm_time_parsing_and_formatting() {
    let t_am = Time::new_12h(10, 30, 0, false).unwrap();
    assert_eq!(t_am.hour, 10);
    assert_eq!(t_am.format_12h(), "10:30:00 AM");

    let t_pm = Time::new_12h(2, 45, 0, true).unwrap();
    assert_eq!(t_pm.hour, 14);
    assert_eq!(t_pm.format_12h(), "02:45:00 PM");

    let t_midnight = Time::new_12h(12, 0, 0, false).unwrap();
    assert_eq!(t_midnight.hour, 0);

    let t_noon = Time::new_12h(12, 0, 0, true).unwrap();
    assert_eq!(t_noon.hour, 12);
}

#[test]
fn test_date_string_parsing_with_am_pm() {
    let d1 = Date::from_str("07-08-2026 10:00 AM").unwrap();
    assert_eq!(d1, Date::new_with_hms(2026, 8, 7, 10, 0, 0));

    let d2 = Date::from_str("07-08-2026 2:30:00 PM").unwrap();
    assert_eq!(d2, Date::new_with_hms(2026, 8, 7, 14, 30, 0));

    let d3 = Date::from_str("07-08-2026 12:15 AM").unwrap();
    assert_eq!(d3, Date::new_with_hms(2026, 8, 7, 0, 15, 0));

    let d4 = Date::from_str("07-08-2026 12:15 PM").unwrap();
    assert_eq!(d4, Date::new_with_hms(2026, 8, 7, 12, 15, 0));
}

#[test]
fn test_timezone_parsing() {
    assert_eq!(TimeZone::parse("EST").unwrap(), TimeZone::new("EST", -300));
    assert_eq!(TimeZone::parse("PST").unwrap(), TimeZone::new("PST", -480));
    assert_eq!(TimeZone::parse("UTC").unwrap(), TimeZone::utc());
    assert_eq!(TimeZone::parse("JST").unwrap(), TimeZone::new("JST", 540));
    assert_eq!(
        TimeZone::parse("+05:30").unwrap(),
        TimeZone::new("+05:30", 330)
    );
    assert_eq!(
        TimeZone::parse("-04:00").unwrap(),
        TimeZone::new("-04:00", -240)
    );
}

#[cfg(feature = "units")]
#[test]
fn test_timezone_conversions_expression() {
    let abacus = Abacus::standard();

    let d_pst = abacus.eval_date("07-08-2026 10:00 AM EST to PST").unwrap();
    assert_eq!(d_pst.time.hour, 7);
    assert_eq!(d_pst.timezone.unwrap().name, "PST");

    let d_jst = abacus.eval_date("07-08-2026 14:00 UTC to JST").unwrap();
    assert_eq!(d_jst.time.hour, 23);
    assert_eq!(d_jst.timezone.unwrap().name, "JST");

    let d_offset = abacus
        .eval_date("07-08-2026 10:00:00 -04:00 to +02:00")
        .unwrap();
    assert_eq!(d_offset.time.hour, 16);
    assert_eq!(d_offset.timezone.unwrap().name, "+02:00");
}

#[test]
fn test_date_difference_across_timezones() {
    let abacus = Abacus::standard();

    // 10:00 AM EST is identical absolute time as 07:00 AM PST
    let diff = abacus
        .eval_scalar("07-08-2026 10:00 AM EST - 07-08-2026 07:00 AM PST")
        .unwrap();
    assert_eq!(diff.canonical, 0.0);
}
