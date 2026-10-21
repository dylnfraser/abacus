//! Regression tests for the core units / values / intervals audit findings.

use abacus::{Abacus, AbacusError, Dimensions, EvalResult, Notation, eval};

fn show(expr: &str) -> String {
    eval(expr)
        .unwrap_or_else(|e| panic!("{expr} failed: {e:?}"))
        .to_display()
}

// --- Dimensions: no overflow, exact rational exponents -------------------------------------

#[test]
fn large_exponents_do_not_overflow_or_wrap() {
    assert_eq!(show("1 m^250 * 1 m^250"), "1 m^500");
    assert_eq!(show("(1 m^2)^150"), "1 m^300");
    // L^27 + L^1 must not be accepted as compatible.
    assert_eq!(
        eval("1 m^300 / 1 m^273 + 1 m^274 / 1 m^273"),
        Err(AbacusError::IncompatibleDimensions)
    );
}

#[test]
fn dimension_overflow_is_an_error() {
    let big = Dimensions::LENGTH * 70_000.0;
    assert!(big.checked_add(big).is_none());
    assert!(Dimensions::LENGTH.checked_mul_f64(f64::NAN).is_none());
}

#[test]
fn fractional_exponents_round_trip() {
    assert_eq!(show("((2 m)^(1/7))^7 to m"), "2 m");
    assert_eq!(show("(8 m^3)^(1/3)"), "2 m");
}

// --- Display -----------------------------------------------------------------------------------

#[test]
fn tiny_magnitudes_are_not_displayed_as_zero() {
    assert_eq!(show("1.6e-19 C"), "1.6e-19 C");
    assert_eq!(show("1e-13 m"), "1e-13 m");
    assert_eq!(show("1 fm to m"), "1e-15 m");
}

#[test]
fn huge_magnitudes_use_scientific_notation() {
    assert_eq!(show("1e300 m"), "1e300 m");
    assert_eq!(show("1e307 USD"), "$1e307");
}

#[test]
fn cancellation_noise_is_snapped_to_zero() {
    assert_eq!(show("0.1 + 0.2 - 0.3"), "0");
}

#[test]
fn denominator_only_units_are_displayed() {
    assert_eq!(show("1/(2 m)"), "0.5 1/m");
    assert_eq!(show("3 / (1 m^2)"), "3 1/m^2");
    assert_eq!(show("5 m^-1"), "5 1/m");
    assert_eq!(show("1/(1 kg)"), "1 1/kg");
}

#[test]
fn powers_produce_simplified_units() {
    assert_eq!(show("(4 m^2)^0.5"), "2 m");
    assert_eq!(show("sqrt(4 m^2)"), "2 m");
    assert_eq!(show("1 m^9 * 1 m"), "1 m^10");
    assert_eq!(show("1 m^9 / 1 m"), "1 m^8");
    assert_eq!(show("1 m^0.5 * 1 m^0.5"), "1 m");
    assert_eq!(show("1 m^09"), "1 m^9");
    assert_eq!(show("1 m^9.0"), "1 m^9");
    assert!(show("sqrt(2 m)").ends_with(" m^0.5"));
}

#[test]
fn consolidation_has_no_float_noise() {
    assert_eq!(show("1 m * 1 ft"), "0.3048 m^2");
}

// --- Derived-unit reduction --------------------------------------------------------------------

#[test]
fn ambiguous_dimensions_are_not_relabelled() {
    assert_eq!(show("(10 m/s)^2"), "100 m^2/s^2");
    assert_eq!(show("10 rad/s"), "10 rad/s");
    assert_eq!(show("5 cd"), "5 cd");
}

#[test]
fn explicit_single_units_are_kept() {
    assert_eq!(show("1 atm"), "1 atm");
    assert_eq!(show("3 GHz"), "3 GHz");
    assert_eq!(show("1 mA"), "1 mA");
}

#[test]
fn composite_units_still_reduce() {
    assert_eq!(show("10 N * 5 m"), "50 J");
    assert_eq!(show("12 V * 2 A"), "24 W");
    assert_eq!(show("1 N/m^2"), "1 Pa");
}

// --- Affine temperatures -----------------------------------------------------------------------

#[test]
fn negative_temperatures_can_be_entered() {
    assert_eq!(show("-10 °C"), "-10 °C");
    assert_eq!(show("-40 °F in °C"), "-40 °C");
    assert_eq!(show("(-40) °C"), "-40 °C");
}

#[test]
fn temperature_differences_and_shifts() {
    assert_eq!(show("10 °C - 5 °C"), "5 K");
    assert_eq!(show("0 °C + 1 K"), "1 °C");
    assert_eq!(show("20 °C * 2"), "40 °C");
    assert_eq!(
        eval("10 °C + 5 °C"),
        Err(AbacusError::AffineUnitOperation("add"))
    );
}

#[test]
fn affine_exponent_units() {
    assert_eq!(show("10 °C^1 in K"), "283.15 K");
    assert!(matches!(
        eval("1 °C^2"),
        Err(AbacusError::AffineUnitOperation(_))
    ));
}

// --- Exponent unit parsing & cache -------------------------------------------------------------

#[test]
fn nan_exponents_are_rejected() {
    assert!(eval("1 m^NaN").is_err());
    assert!(eval("1 m^inf").is_err());
}

#[test]
fn exponent_cache_is_bounded_and_normalized() {
    let calc = Abacus::standard();
    for e in ["1", "1.0", "01"] {
        calc.units.unit(&format!("m^{e}")).unwrap();
    }
    assert_eq!(calc.units.cached_exponent_units(), 1);
    for k in 0..10_000 {
        let _ = calc.units.unit(&format!("m^1.{k:07}"));
    }
    assert!(calc.units.cached_exponent_units() <= 4_096);
    assert!(calc.units.unit(&format!("m^{}", "1".repeat(1_000))).is_err());
}

// --- Intervals ---------------------------------------------------------------------------------

#[test]
fn per_division_checks_the_singularity() {
    assert_eq!(show("[1,2] per [-1,1]"), "[-inf, inf]");
    assert_eq!(show("1 per [-1,1]"), "[-inf, inf]");
}

#[test]
fn powers_of_intervals_are_sound() {
    assert_eq!(show("[-2,3]^2"), "[0, 9]");
    assert_eq!(show("[-1,1]^-2"), "[1, inf]");
    assert_eq!(show("[-1,1]^-1"), "[-inf, inf]");
}

#[test]
fn zero_dividend_over_zero_spanning_interval() {
    assert_eq!(show("0/[0,1]"), "[0, 0]");
}

#[test]
fn inexact_bounds_are_rounded_outward() {
    let r = eval("[0.1,0.1]+[0.2,0.2]").unwrap();
    let EvalResult::Interval(i) = r else {
        panic!("expected interval")
    };
    assert!(i.lo.canonical < i.hi.canonical);
    assert!(i.lo.canonical <= 0.3 && i.hi.canonical >= 0.30000000000000004);
    // Exact operations are not widened.
    assert_eq!(show("1..10 + 5"), "6..15");
    assert_eq!(show("[10 N, 20 N] * [2 m, 5 m]"), "[20 J, 100 J]");
}

#[test]
fn percent_intervals_with_negative_endpoints() {
    let EvalResult::Interval(i) = eval("[-1,2] + [5%,10%]").unwrap() else {
        panic!("expected interval")
    };
    assert!(i.lo.canonical <= -1.1);
    assert!(i.hi.canonical >= 2.2);
}

// --- Percent arithmetic ------------------------------------------------------------------------

#[test]
fn percent_arithmetic_is_consistent() {
    assert_eq!(show("50% / 2 s"), "0.25 Hz");
    assert!(show("(5%)^2").starts_with("0.25"));
    assert!(show("5% * 5%").starts_with("0.25"));
    assert!(show("5% * 5%").ends_with('%'));
    assert_eq!(show("10% - 3"), "-2.9");
    assert_eq!(show("20 less than 10%"), "18");
}

// --- Equality ----------------------------------------------------------------------------------

#[test]
fn value_equality_is_reflexive_and_relative() {
    let inf = eval("1/0").unwrap();
    assert_eq!(inf, inf.clone());
    assert_ne!(eval("1.6e-19 C").unwrap(), eval("3.2e-19 C").unwrap());
    assert_eq!(eval("1 km").unwrap(), eval("1000 m").unwrap());
}

// --- Output settings apply to display only -----------------------------------------------------

#[test]
fn variables_keep_full_precision() {
    let mut calc = Abacus::standard();
    calc.eval_mut("x = 1 USD / 3").unwrap();
    assert_eq!(calc.eval("x*3").unwrap().to_display(), "$1");

    let mut calc = Abacus::standard().with_decimal_places(2);
    let shown = calc.set_variable_expr("y", "2 / 3").unwrap();
    assert_eq!(shown.to_display(), "0.67");
    assert_eq!(calc.eval("y * 3").unwrap().to_display(), "2");
}

#[test]
fn decimal_rounding_does_not_overflow() {
    let calc = Abacus::standard().with_decimal_places(10);
    assert_eq!(calc.eval("1e300 m").unwrap().to_display(), "1e300 m");
}

// --- API consistency ---------------------------------------------------------------------------

#[test]
fn scalar_extraction_errors_match() {
    let interval = eval("[1,2]").unwrap();
    assert_eq!(
        interval.as_scalar().err(),
        interval.clone().into_scalar().err()
    );
}

#[test]
fn notation_applies_to_intervals_and_hashes() {
    let calc = Abacus::standard().with_notation(Notation::Scientific);
    let r = calc.eval("[1000, 2000]").unwrap();
    assert_eq!(calc.format_result(&r), "[1e3, 2e3]");
    let h = calc.eval("linreg(1,2,3,2,4,6)").unwrap();
    assert!(calc.format_result(&h).contains("slope: 2e0"));
}

#[test]
fn human_duration_handles_non_finite() {
    assert_eq!(
        abacus::units::value::format_human_duration(f64::NAN),
        "NaN seconds"
    );
}
