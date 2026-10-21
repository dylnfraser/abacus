//! Regression tests for the post-audit code review findings.

#[cfg(any(feature = "units", feature = "distributions"))]
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

#[test]
#[cfg(all(feature = "units", feature = "stats"))]
fn fahrenheit_spreads_keep_full_precision() {
    // Values match Python's `statistics.stdev`; previously the kelvin offset cost ~3 digits.
    assert_eq!(show("std(20 °F, 30 °F)"), "7.0710678118654755 °R");
    assert_eq!(show("var(20 °F, 30 °F)"), "50 °R^2");
    assert_eq!(
        show("std(98.6 °F, 99.1 °F, 97.9 °F)"),
        "0.6027713773341649 °R"
    );
    assert_eq!(show("std(10 °C, 20 °C, 25 °C)"), "7.637626158259733 K");
}

#[cfg(feature = "distributions")]
fn num(expr: &str) -> f64 {
    eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .into_scalar()
        .unwrap()
        .canonical
}

#[test]
#[cfg(feature = "distributions")]
fn student_t_with_huge_df_matches_the_normal_limit() {
    // Found by cargo-fuzz: invt with df ≈ 5e44 took ~10 s and returned -8388608.
    let start = std::time::Instant::now();
    let z = -6.254_027_707_160_142; // Φ⁻¹(2e-10), mpmath
    for df in ["1e30", "5.288774053104562e44"] {
        let t = num(&format!("invt(2e-10, {df})"));
        assert!((t - z).abs() < 1e-12, "invt(2e-10, {df}) = {t}");
    }
    let normal_tail = 2.823_158_037_043_271e-10; // Φ(-6.2), mpmath
    let tail = num("tcdf(1e20, -6.2)");
    assert!(((tail - normal_tail) / normal_tail).abs() < 1e-13, "{tail}");
    assert!((num("tpdf(1e30, 0)") - 0.398_942_280_401_432_7).abs() < 1e-15);
    // Moderate df is unchanged (mpmath references).
    assert!((num("invt(0.975, 30)") - 2.042_272_456_301_238).abs() < 1e-13);
    assert!((num("tpdf(5, 1)") - 0.219_679_797_350_980_6).abs() < 1e-15);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

#[test]
#[cfg(feature = "distributions")]
fn hopeless_incomplete_beta_inputs_fail_fast() {
    let start = std::time::Instant::now();
    assert!(eval("fcdf(1e30, 1e30, 1)").is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
#[cfg(feature = "distributions")]
fn fuzz_timeout_input_evaluates_quickly() {
    let start = std::time::Instant::now();
    let _ = eval("invt(2m^0%%%%%%975, phi^214)%%%% +myriad-1");
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
