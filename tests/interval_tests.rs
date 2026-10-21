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
use abacus::{Abacus, AbacusError, EvalResult};

fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    Abacus::standard().eval(expr)
}

// ── Basic interval construction ──

#[cfg(feature = "units")]
#[test]
fn test_section2_interval_arithmetic_expanded() {
    let calc = Abacus::standard();

    // Flagship example: [9.8 m, 10.2 m] / [1.9 s, 2.1 s]
    // Both quotients are inexact, so the bounds are rounded outward by one ulp.
    let speed = calc.eval("[9.8 m, 10.2 m] / [1.9 s, 2.1 s]").unwrap();
    assert_eq!(
        speed.to_display(),
        "[4.666666666666666 m/s, 5.36842105263158 m/s]"
    );

    // Scalar + Interval: 5 m + [1 m, 3 m] -> [6 m, 8 m]
    assert_eq!(
        calc.eval("5 m + [1 m, 3 m]").unwrap().to_display(),
        "[6 m, 8 m]"
    );

    // Unit conversion on interval: [1 km, 2 km] to m -> [1000 m, 2000 m]
    assert_eq!(
        calc.eval("[1 km, 2 km] to m").unwrap().to_display(),
        "[1000 m, 2000 m]"
    );

    // Force * Distance interval: [10 N, 20 N] * [2 m, 5 m] -> [20 J, 100 J]
    assert_eq!(
        calc.eval("[10 N, 20 N] * [2 m, 5 m]").unwrap().to_display(),
        "[20 J, 100 J]"
    );
}

#[cfg(feature = "units")]
#[test]
fn test_basic_interval_construction() {
    // Verifies [lo, hi] syntax produces an interval with correct endpoints
    let result = eval("[1 m, 5 m]").unwrap();
    assert_eq!(result.to_display(), "[1 m, 5 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_auto_normalizes_order() {
    // Verifies that [hi, lo] is auto-normalized to [lo, hi]
    let result = eval("[10 m, 2 m]").unwrap();
    assert_eq!(result.to_display(), "[2 m, 10 m]");
}

#[test]
fn test_dimensionless_interval() {
    // Verifies creating intervals with plain numbers (no units)
    let result = eval("[3, 7]").unwrap();
    assert_eq!(result.to_display(), "[3, 7]");
}

// ── Interval + Interval arithmetic ──

#[cfg(feature = "units")]
#[test]
fn test_interval_addition() {
    // [1 m, 3 m] + [2 m, 4 m] = [3 m, 7 m]
    let result = eval("[1 m, 3 m] + [2 m, 4 m]").unwrap();
    assert_eq!(result.to_display(), "[3 m, 7 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_subtraction() {
    // [5 m, 10 m] - [1 m, 3 m] = [2 m, 9 m] (min: 5-3=2, max: 10-1=9)
    let result = eval("[5 m, 10 m] - [1 m, 3 m]").unwrap();
    assert_eq!(result.to_display(), "[2 m, 9 m]");
}

#[test]
fn test_interval_multiplication() {
    // [2, 3] * [4, 5] = [8, 15] (all corners positive)
    let result = eval("[2, 3] * [4, 5]").unwrap();
    assert_eq!(result.to_display(), "[8, 15]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_division_the_flagship_example() {
    // The headline feature: [9.8 m, 10.2 m] / [1.9 s, 2.1 s]
    // Corners: 9.8/1.9≈5.157, 9.8/2.1≈4.666, 10.2/1.9≈5.368, 10.2/2.1≈4.857
    // Result: [4.666... m/s, 5.368... m/s]
    let result = eval("[9.8 m, 10.2 m] / [1.9 s, 2.1 s]").unwrap();
    let display = result.to_display();
    assert!(
        display.starts_with("["),
        "Expected interval display, got: {}",
        display
    );
    assert!(
        display.contains("m/s"),
        "Expected m/s units, got: {}",
        display
    );

    // Extract numerical bounds and verify
    if let EvalResult::Interval(interval) = result {
        let lo_val = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi_val = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        assert!(
            (lo_val - 4.666666).abs() < 0.01,
            "lo should be ~4.666, got {}",
            lo_val
        );
        assert!(
            (hi_val - 5.368421).abs() < 0.01,
            "hi should be ~5.368, got {}",
            hi_val
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── Scalar + Interval mixed arithmetic ──

#[cfg(feature = "units")]
#[test]
fn test_scalar_plus_interval() {
    // 5 m + [1 m, 3 m] = [6 m, 8 m]
    let result = eval("5 m + [1 m, 3 m]").unwrap();
    assert_eq!(result.to_display(), "[6 m, 8 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_plus_scalar() {
    // [1 m, 3 m] + 5 m = [6 m, 8 m]
    let result = eval("[1 m, 3 m] + 5 m").unwrap();
    assert_eq!(result.to_display(), "[6 m, 8 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_times_scalar() {
    // [2 m, 4 m] * 3 = [6 m, 12 m]
    let result = eval("[2 m, 4 m] * 3").unwrap();
    assert_eq!(result.to_display(), "[6 m, 12 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_scalar_times_interval() {
    // 3 * [2 m, 4 m] = [6 m, 12 m]
    let result = eval("3 * [2 m, 4 m]").unwrap();
    assert_eq!(result.to_display(), "[6 m, 12 m]");
}

// ── Interval with unit conversion ──

#[cfg(feature = "units")]
#[test]
fn test_interval_unit_conversion() {
    // [1 km, 2 km] to m = [1000 m, 2000 m]
    let result = eval("[1 km, 2 km] to m").unwrap();
    assert_eq!(result.to_display(), "[1000 m, 2000 m]");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_unit_conversion_mixed_units() {
    // [500 m, 2 km] to m = [500 m, 2000 m]
    let result = eval("[500 m, 2 km] to m").unwrap();
    assert_eq!(result.to_display(), "[500 m, 2000 m]");
}

// ── Interval in grouped expressions ──

#[test]
fn test_interval_in_parentheses() {
    // ([1, 3] + [2, 4]) * 2 = [3, 7] * 2 = [6, 14]
    let result = eval("([1, 3] + [2, 4]) * 2").unwrap();
    assert_eq!(result.to_display(), "[6, 14]");
}

// ── Implicit multiplication with intervals ──

#[test]
fn test_implicit_multiplication_with_interval() {
    // 5 [1, 3] = 5 * [1, 3] = [5, 15]
    let result = eval("5 [1, 3]").unwrap();
    assert_eq!(result.to_display(), "[5, 15]");
}

// ── Negative and zero-crossing intervals ──

#[test]
fn test_interval_crossing_zero_multiplication() {
    // [-2, 3] * [1, 4] = corners: -2*1=-2, -2*4=-8, 3*1=3, 3*4=12 → [-8, 12]
    let result = eval("[-2, 3] * [1, 4]").unwrap();
    assert_eq!(result.to_display(), "[-8, 12]");
}

#[test]
fn test_negative_interval() {
    // [-5, -1] + [2, 3] = [-3, 2]
    let result = eval("[-5, -1] + [2, 3]").unwrap();
    assert_eq!(result.to_display(), "[-3, 2]");
}

// ── Derived unit reduction with intervals ──

#[cfg(feature = "units")]
#[test]
fn test_interval_derived_unit_reduction() {
    // Force × distance → Joules: [10 N, 20 N] * [2 m, 5 m] = [20 J, 100 J]
    let result = eval("[10 N, 20 N] * [2 m, 5 m]").unwrap();
    assert_eq!(result.to_display(), "[20 J, 100 J]");
}

// ── Unary operators on intervals ──

#[cfg(feature = "units")]
#[test]
fn test_unary_negation_on_interval() {
    // -[2 m, 5 m] = [-5 m, -2 m]
    let result = eval("-[2 m, 5 m]").unwrap();
    assert_eq!(result.to_display(), "[-5 m, -2 m]");
}

// ── Error cases ──

#[cfg(feature = "stats")]
#[test]
fn test_interval_in_function_errors() {
    // Intervals cannot be passed as function arguments
    assert!(matches!(
        eval("mean([1, 3])"),
        Err(AbacusError::IntervalInFunction)
    ));
}

#[cfg(feature = "units")]
#[test]
fn test_incompatible_interval_dimensions_error() {
    // [1 m, 5 s] should error — incompatible dimensions
    assert!(matches!(
        eval("[1 m, 5 s]"),
        Err(AbacusError::IncompatibleDimensions)
    ));
}

#[test]
fn test_unclosed_bracket_error() {
    // [1, 3 without closing ] should error
    assert!(eval("[1, 3").is_err());
}

#[test]
fn test_interval_missing_comma_error() {
    // [1 3] without comma should error
    assert!(eval("[1 3]").is_err());
}

#[cfg(feature = "units")]
#[test]
fn test_range_syntax_intervals() {
    // 1..10 + 5 = 6..15
    let res = eval("1..10 + 5").unwrap();
    assert_eq!(res.to_display(), "6..15");

    // 1..10 - 5 = -4..5
    let res = eval("1..10 - 5").unwrap();
    assert_eq!(res.to_display(), "-4..5");

    // 5 + 1..10 = 6..15
    let res = eval("5 + 1..10").unwrap();
    assert_eq!(res.to_display(), "6..15");

    // 5 - 1..10 = -5..4
    let res = eval("5 - 1..10").unwrap();
    assert_eq!(res.to_display(), "-5..4");

    // Adding two intervals: 1..10 + 2..5 = 3..15
    let res = eval("1..10 + 2..5").unwrap();
    assert_eq!(res.to_display(), "3..15");

    // Subtracting two intervals: 1..10 - 2..5 = -4..8
    let res = eval("1..10 - 2..5").unwrap();
    assert_eq!(res.to_display(), "-4..8");

    // Multiplication with scalar: 1..10 * 2 = 2..20
    let res = eval("1..10 * 2").unwrap();
    assert_eq!(res.to_display(), "2..20");

    // Standalone range interval: 1..10 = 1..10
    let res = eval("1..10").unwrap();
    assert_eq!(res.to_display(), "1..10");

    // Range intervals with physical units
    let res = eval("1 m .. 10 m + 5 m").unwrap();
    assert_eq!(res.to_display(), "6 m..15 m");

    let res = eval("1..10 m + 5 m").unwrap();
    assert_eq!(res.to_display(), "6 m..15 m");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_division_zero_crossing_singularity() {
    // [1 m, 2 m] / [-1 s, 1 s] = [-inf m/s, inf m/s]
    let res = eval("[1 m, 2 m] / [-1 s, 1 s]").unwrap();
    assert_eq!(res.to_display(), "[-inf m/s, inf m/s]");

    // [1, 2] / [0, 2] = [0.5, inf]
    let res = eval("[1, 2] / [0, 2]").unwrap();
    assert_eq!(res.to_display(), "[0.5, inf]");

    // [-2, -1] / [0, 2] = [-inf, -0.5]
    let res = eval("[-2, -1] / [0, 2]").unwrap();
    assert_eq!(res.to_display(), "[-inf, -0.5]");

    // Range style 1..2 / -1..1 = -inf..inf
    let res = eval("1..2 / -1..1").unwrap();
    assert_eq!(res.to_display(), "-inf..inf");
}
