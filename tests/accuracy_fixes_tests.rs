//! Regression tests for the accuracy sweep fixes (display, financial, `%`, nCr, roots,
//! overflow). Reference values come from mpmath at 40+ digits.

#[cfg(feature = "units")]
use abacus::eval;

#[cfg(feature = "units")]
fn show(expr: &str) -> String {
    eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .to_display()
}

#[cfg(any(feature = "financial", feature = "stats"))]
fn num(expr: &str) -> f64 {
    abacus::eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .into_scalar()
        .unwrap()
        .canonical
}

#[cfg(feature = "financial")]
fn assert_rel(expr: &str, expected: f64, tol: f64) {
    let got = num(expr);
    let rel = ((got - expected) / expected).abs();
    assert!(
        rel <= tol,
        "{expr}: got {got}, expected {expected} (rel {rel:e})"
    );
}

#[test]
#[cfg(feature = "units")]
fn long_numbers_keep_their_fractional_digits() {
    assert_eq!(show("9460730472580.8"), "9460730472580.8");
    assert_eq!(show("1234567890123.4"), "1234567890123.4");
    assert_eq!(show("1 ly in km"), "9460730472580.8 km");
    // Float noise right next to an integer is still hidden.
    assert_eq!(show("2.9999999999999996"), "3");
    assert_eq!(show("0.1 * 3 * 10"), "3");
}

#[test]
#[cfg(feature = "financial")]
fn financial_functions_are_accurate_at_tiny_rates() {
    // Previously fv(1e-12, 360, -100) was 36003.2 and pmt was off in the 5th digit.
    assert_rel("fv(1e-12, 360, -100)", 36_000.000_006_461_996, 1e-14);
    assert_rel("pv(1e-12, 360, -100)", 35_999.999_993_501_995, 1e-14);
    assert_rel("pmt(1e-12, 360, 200000)", -555.555_555_655_833_3, 1e-14);
    assert_rel(
        "compound(1000, 1e-9, 30, 12)",
        1_000.000_030_000_000_4,
        1e-14,
    );
    // Ordinary rates are unchanged (and correctly rounded).
    assert_rel("pmt(0.05/12, 360, 200000)", -1_073.643_246_024_278, 1e-15);
    assert_rel("fv(0.5, 12, -100)", 25_749.267_578_125, 1e-15);
}

#[test]
#[cfg(feature = "units")]
fn modulo_with_a_signed_divisor() {
    assert_eq!(show("5 % -3"), "-1");
    assert_eq!(show("7 % -3"), "-2");
    assert_eq!(show("5 % +3"), "2");
    // Percentages followed by a spaced or unspaced minus stay percentages.
    assert_eq!(show("10% - 3"), "-2.9");
    assert_eq!(show("10%-3"), "-2.9");
}

#[test]
#[cfg(feature = "stats")]
fn n_cr_is_correctly_rounded() {
    // These all match mpmath's exact binomial rounded to f64.
    assert_eq!(num("nCr(1000, 500)"), 2.702_882_409_454_365_5e299);
    assert_eq!(num("nCr(200, 100)"), 9.054_851_465_610_328e58);
    assert_eq!(num("nCr(1029, 514)"), 1.429_820_686_498_904e308);
    assert_eq!(num("nCr(52, 5)"), 2_598_960.0);
    assert!(abacus::eval("nCr(1030, 515)").is_err()); // overflows f64
}

#[test]
#[cfg(feature = "units")]
fn real_roots_of_negative_numbers() {
    assert_eq!(show("(-8)^(1/3)"), "-2");
    assert_eq!(show("(-8)^(2/3)"), "4");
    assert_eq!(show("(-32)^(1/5)"), "-2");
    assert_eq!(show("cbrt(-27)"), "-3");
    assert_eq!(show("cbrt(8 m^3)"), "2 m");
    assert_eq!(show("[-8, 27]^(1/3)"), "[-2, 3]");
    // Even roots of negatives have no real value.
    assert_eq!(show("(-8)^(1/2)"), "NaN");
}

#[test]
#[cfg(feature = "units")]
fn arithmetic_overflow_is_an_error() {
    for expr in [
        "1e308 * 10",
        "1e308 + 1e308",
        "1e308 / 1e-10",
        "10^400",
        "1 km * 1e306",
    ] {
        assert!(eval(expr).is_err(), "{expr} should overflow");
    }
    // Division by zero and infinite operands are not overflow.
    assert_eq!(show("1/0"), "inf");
    assert_eq!(show("0^-1"), "inf");
    assert_eq!(show("(1/0) + 1"), "inf");
}

#[test]
#[cfg(all(feature = "units", feature = "stats"))]
fn same_unit_temperature_differences_are_exact() {
    assert_eq!(show("30 °F - 20 °F"), "10 °R");
    assert_eq!(show("98.6 °F - 32 °F"), "66.6 °R");
    assert_eq!(show("range(20 °F, 30 °F)"), "10 °R");
    assert_eq!(show("iqr(20 °F, 30 °F, 40 °F, 50 °F)"), "15 °R");
}
