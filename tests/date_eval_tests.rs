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
use abacus::{Abacus, Date, eval};

#[cfg(all(feature = "units", feature = "date"))]
#[test]
fn test_section6_date_functionality_expanded() {
    let calc = Abacus::standard();

    // 07-08-2026 + 5 days = 12-08-2026
    assert_eq!(
        calc.eval("07-08-2026 + 5 days").unwrap().to_display(),
        "August 12, 2026"
    );

    // 2026/08/07 - 2 weeks = 24-07-2026
    assert_eq!(
        calc.eval("2026/08/07 - 2 weeks").unwrap().to_display(),
        "July 24, 2026"
    );

    // 2026-08-07 10:30:00 + 3 hours = 07-08-2026 13:30:00
    assert_eq!(
        calc.eval("2026-08-07 10:30:00 + 3 hours")
            .unwrap()
            .to_display(),
        "August 7, 2026 13:30:00"
    );

    // 07-08-2026 02:30 PM = 07-08-2026 14:30:00
    assert_eq!(
        calc.eval("07-08-2026 02:30 PM").unwrap().to_display(),
        "August 7, 2026 14:30:00"
    );

    // Timezone subtraction: 07-08-2026 10:00 AM EST - 07-08-2026 07:00 AM PST -> 0 s
    let diff = calc
        .eval("07-08-2026 10:00 AM EST - 07-08-2026 07:00 AM PST")
        .unwrap();
    assert_eq!(diff.to_display(), "0 s");

    // date(2026, 8, 7, 10, 54, 49) = 07-08-2026 10:54:49
    assert_eq!(
        calc.eval("date(2026, 8, 7, 10, 54, 49)")
            .unwrap()
            .to_display(),
        "August 7, 2026 10:54:49"
    );
}

#[test]
fn test_unquoted_dd_mm_yyyy_date_literals() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("07-08-2026").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 7));

    let d2 = abacus.eval_date("07/08/2026").unwrap();
    assert_eq!(d2, Date::new(2026, 8, 7));

    let d3 = abacus.eval_date("07-08-2026 10:54:49").unwrap();
    assert_eq!(d3, Date::new_with_hms(2026, 8, 7, 10, 54, 49));
}

#[test]
fn test_unquoted_yyyy_mm_dd_date_literals() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("2026-08-07").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 7));

    let d2 = abacus.eval_date("2026/08/07").unwrap();
    assert_eq!(d2, Date::new(2026, 8, 7));
}

#[test]
fn test_delimited_at_date_literals() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("@2026-08-07@").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 7));

    let d2 = abacus.eval_date("@07-08-2026 10:54:49@").unwrap();
    assert_eq!(d2, Date::new_with_hms(2026, 8, 7, 10, 54, 49));
}

#[cfg(feature = "units")]
#[test]
fn test_date_addition_arithmetic() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("07-08-2026 + 5 days").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 12));

    let d2 = abacus.eval_date("2026-08-07 + 3 h").unwrap();
    assert_eq!(d2, Date::new_with_hms(2026, 8, 7, 3, 0, 0));

    let d3 = abacus.eval_date("07-08-2026 23:00:00 + 2 hours").unwrap();
    assert_eq!(d3, Date::new_with_hms(2026, 8, 8, 1, 0, 0));
}

#[cfg(feature = "units")]
#[test]
fn test_date_subtraction_arithmetic() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("07-08-2026 - 2 days").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 5));

    // A whole number of days is reported in days (864,000 s).
    let diff_sec = abacus.eval_scalar("17-08-2026 - 07-08-2026").unwrap();
    assert_eq!(diff_sec.canonical, 864_000.0);
    assert_eq!(diff_sec.to_display(), "10 d");

    let diff_days = eval("(17-08-2026 - 07-08-2026) in days").unwrap();
    assert_eq!(diff_days.to_display(), "10 d");
}

#[test]
fn test_date_property_access() {
    let abacus = Abacus::standard();

    assert_eq!(
        abacus.eval_scalar("07-08-2026.year").unwrap().canonical,
        2026.0
    );
    assert_eq!(
        abacus.eval_scalar("07-08-2026.month").unwrap().canonical,
        8.0
    );
    assert_eq!(abacus.eval_scalar("07-08-2026.day").unwrap().canonical, 7.0);
    assert_eq!(
        abacus
            .eval_scalar("07-08-2026.day_of_week")
            .unwrap()
            .canonical,
        5.0 // Friday
    );
}

#[cfg(feature = "date")]
#[test]
fn test_date_function_operator() {
    let abacus = Abacus::standard();

    let d1 = abacus.eval_date("date(2026, 8, 7)").unwrap();
    assert_eq!(d1, Date::new(2026, 8, 7));

    let d2 = abacus.eval_date("date(2026, 8, 7, 10, 54, 49)").unwrap();
    assert_eq!(d2, Date::new_with_hms(2026, 8, 7, 10, 54, 49));
}

#[cfg(feature = "units")]
#[test]
fn test_mm_dd_yyyy_and_slash_eval_expressions() {
    let calc = Abacus::standard();

    let d1 = calc.eval_date("05-16-2010").unwrap();
    assert_eq!(d1, Date::new(2010, 5, 16));

    let d2 = calc.eval_date("05/16/2010").unwrap();
    assert_eq!(d2, Date::new(2010, 5, 16));

    let d3 = calc.eval_date("5/16/2010 + 10 days").unwrap();
    assert_eq!(d3, Date::new(2010, 5, 26));

    let d4 = calc.eval_date("05-16-2010 - 2 weeks").unwrap();
    assert_eq!(d4, Date::new(2010, 5, 2));

    let diff = calc.eval("(05/26/2010 - 05/16/2010) in days").unwrap();
    assert_eq!(diff.to_display(), "10 d");

    let d5 = calc.eval_date("05-16-2010 14:30:00").unwrap();
    assert_eq!(d5, Date::new_with_hms(2010, 5, 16, 14, 30, 0));

    let d6 = calc.eval_date("05/16/2010 02:30 PM").unwrap();
    assert_eq!(d6, Date::new_with_hms(2010, 5, 16, 14, 30, 0));
}
