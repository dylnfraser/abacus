//! Regression tests for the post-audit code review findings.

#[cfg(feature = "units")]
use abacus::eval;

#[cfg(feature = "units")]
fn show(expr: &str) -> String {
    eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .to_display()
}

#[test]
#[cfg(feature = "units")]
fn exact_integer_differences_are_not_snapped_to_zero() {
    assert_eq!(show("9007199254740992 - 9007199254740991"), "1");
    assert_eq!(show("10000000000000000 - 9999999999999998"), "2");
    assert_eq!(show("1e16 + 2 - 1e16"), "2");
    // Genuine cancellation noise on non-integers is still cleaned up.
    assert_eq!(show("0.1 + 0.2 - 0.3"), "0");
    assert_eq!(show("1.1 - 1.0 - 0.1"), "0");
}

#[test]
#[cfg(feature = "units")]
fn tiny_angles_are_not_treated_as_zero() {
    assert_eq!(show("sin(1e-13)"), "1e-13");
    assert_eq!(show("tan(1e-13 rad)"), "1e-13");
    assert_eq!(show("sin(0)"), "0");
    assert_eq!(show("sin(180 deg)"), "0");
    assert_eq!(show("cos(90 deg)"), "0");
    assert!(eval("tan(90 deg)").is_err());
}

#[test]
#[cfg(all(feature = "units", feature = "stats"))]
fn temperature_spreads_use_the_same_unit_as_subtraction() {
    assert_eq!(show("30 °F - 20 °F"), "10 °R");
    assert_eq!(show("range(20 °F, 30 °F)"), "10 °R");
    assert!(show("std(20 °F, 30 °F)").ends_with(" °R"));
    assert_eq!(show("range(20 °C, 30 °C)"), "10 K");
}

#[test]
#[cfg(feature = "units")]
fn distinct_whole_numbers_display_distinctly() {
    assert_ne!(show("1e20 + 16384"), show("1e20"));
    // Float noise on short decimals is still hidden.
    assert_eq!(show("0.1 + 0.2"), "0.3");
}

#[test]
#[cfg(feature = "units")]
fn temperature_divided_by_zero_is_an_error() {
    assert!(eval("20 °C / 0").is_err());
    assert_eq!(show("20 °C / 2"), "10 °C");
}

#[test]
#[cfg(feature = "date")]
fn distinct_floating_dates_never_compare_equal() {
    use abacus::units::date::Date;
    use std::collections::HashSet;
    // 02:30 doesn't exist on 2026-03-08 in US zones (DST gap); it must still differ from 03:30.
    let gap: Date = "2026-03-08 02:30".parse().unwrap();
    let after: Date = "2026-03-08 03:30".parse().unwrap();
    assert_ne!(gap, after);
    assert!(gap < after);
    let set: HashSet<Date> = [gap.clone(), after.clone()].into_iter().collect();
    assert_eq!(set.len(), 2);

    // Zoned dates still compare by instant.
    let est: Date = "2026-08-07 10:00 EST".parse().unwrap();
    let utc: Date = "2026-08-07 15:00 UTC".parse().unwrap();
    assert_eq!(est, utc);
    let nine_est: Date = "2026-08-07 09:00 EST".parse().unwrap();
    let ten_utc: Date = "2026-08-07 10:00 UTC".parse().unwrap();
    assert!(nine_est > ten_utc); // 14:00Z vs 10:00Z
}
