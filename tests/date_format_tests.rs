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
use abacus::{Abacus, DateFormat};

#[test]
fn test_default_date_format_is_month_day_year() {
    let abacus = Abacus::standard();
    assert_eq!(abacus.date_format, DateFormat::MonthDayYear);

    let d_res = abacus.eval("07-08-2026").unwrap();
    assert_eq!(d_res.to_display(), "August 7, 2026");

    let may_res = abacus.eval("05-16-2010").unwrap();
    assert_eq!(may_res.to_display(), "May 16, 2010");
}

#[test]
fn test_configurable_date_format() {
    let abacus_iso = Abacus::standard().set_date_format(DateFormat::YYYYMMDD);
    let d = abacus_iso.eval_date("07-08-2026").unwrap();
    assert_eq!(d.format_with_style(abacus_iso.date_format), "2026-08-07");

    // With a month-first format, ambiguous input is also read month-first so output round-trips.
    let abacus_us = Abacus::standard().set_date_format(DateFormat::MMDDYYYY);
    let d2 = abacus_us.eval_date("08-07-2026").unwrap();
    assert_eq!((d2.month, d2.day), (8, 7));
    assert_eq!(d2.format_with_style(abacus_us.date_format), "08-07-2026");

    let abacus_eu = Abacus::standard().set_date_format(DateFormat::DDMMYYYY);
    let d3 = abacus_eu.eval_date("07-08-2026").unwrap();
    assert_eq!(d3.format_with_style(abacus_eu.date_format), "07-08-2026");
}

#[cfg(feature = "units")]
#[test]
fn test_date_to_date_time_between_conversion() {
    let abacus = Abacus::standard();

    // 07-08-2026 to 17-08-2026 is a whole number of days, so it is shown in days by default
    let res_days_default = abacus.eval("07-08-2026 to 17-08-2026").unwrap();
    assert_eq!(res_days_default.to_display(), "10 d");

    // Chained conversion in days
    let res_days = abacus.eval("07-08-2026 to 17-08-2026 in days").unwrap();
    assert_eq!(res_days.to_display(), "10 d");

    // Chained conversion in hours
    let res_hours = abacus.eval("07-08-2026 to 08-08-2026 in hours").unwrap();
    assert_eq!(res_hours.to_display(), "24 h");
}
