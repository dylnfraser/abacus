//! Regression tests for the math/statistics audit findings.
//!
//! Reference values come from mpmath (40-digit precision) unless stated otherwise. Values
//! are checked through `canonical` rather than the display string so they are independent
//! of output formatting.
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
use std::time::{Duration, Instant};

fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    Abacus::standard().eval(expr)
}

fn scalar(expr: &str) -> f64 {
    Abacus::standard()
        .eval_scalar(expr)
        .unwrap_or_else(|e| panic!("{expr} failed: {e:?}"))
        .canonical
}

fn hash_value(expr: &str, key: &str) -> f64 {
    Abacus::standard()
        .eval_hash(expr)
        .unwrap_or_else(|e| panic!("{expr} failed: {e:?}"))
        .get(key)
        .unwrap_or_else(|| panic!("{expr} has no key {key}"))
        .canonical
}

fn interval(expr: &str) -> (f64, f64) {
    match eval(expr).unwrap_or_else(|e| panic!("{expr} failed: {e:?}")) {
        EvalResult::Interval(i) => (i.lo.canonical, i.hi.canonical),
        other => panic!("{expr}: expected interval, got {other}"),
    }
}

/// Asserts `|got - want| <= tol * |want|`.
fn assert_rel(expr: &str, want: f64, tol: f64) {
    let got = scalar(expr);
    let rel = ((got - want) / want).abs();
    assert!(
        rel <= tol,
        "{expr}: got {got:e}, want {want:e}, rel err {rel:e}"
    );
}

fn assert_err(expr: &str) {
    assert!(
        eval(expr).is_err(),
        "{expr} should be an error, got {:?}",
        eval(expr)
    );
}

fn display(expr: &str) -> String {
    eval(expr).unwrap().to_display()
}

// ── Finding 1: erfinv/invnorm accuracy ──

#[cfg(feature = "distributions")]
#[test]
fn invnorm_is_accurate_to_full_precision() {
    assert_rel("invnorm(0.975)", 1.959_963_984_540_054, 1e-15);
    assert_rel("invnorm(0.995)", 2.575_829_303_548_901, 1e-15);
    assert_rel("invnorm(0.9)", 1.281_551_565_544_600_6, 1e-15);
    assert_rel("invnorm(0.3)", -0.524_400_512_708_040_8, 1e-15);
    assert_rel("invnorm(1e-10)", -6.361_340_902_404_056, 1e-15);
    assert_rel("invnorm(1e-300)", -37.047_096_299_361_2, 1e-14);
}

#[cfg(feature = "stats")]
#[test]
fn z_based_intervals_use_the_exact_critical_value() {
    let (lo, hi) = interval("ZInterval(0, 1, 100)");
    assert!((hi - 0.195_996_398_454_005_4).abs() < 1e-15);
    assert!((lo + 0.195_996_398_454_005_4).abs() < 1e-15);
    assert_rel("zmoe(1, 1)", 1.959_963_984_540_054, 1e-15);
}

// ── Findings 2 & 3: unambiguous TTest / ZTest / TInterval forms ──

#[cfg(feature = "stats")]
#[test]
fn t_test_data_form_is_never_read_as_summary_stats() {
    // Data {1, 2, 3} tested against μ₀ = 0: t = 2√3, p = 0.0741799 (mpmath).
    assert_rel("TTestData(0, 1, 2, 3).t", 3.464_101_615_137_754_4, 1e-14);
    assert_rel("TTestData(0, 1, 2, 3).p", 0.074_179_900_227_448_6, 1e-12);
    // The summary form has exactly one signature: (μ₀, x̄, s, n).
    assert_err("TTest(0, 1, 2, 3, 4)");
    assert_rel("TTest(0, 1, 2, 3).t", 3f64.sqrt() / 2.0, 1e-14);
}

#[cfg(feature = "stats")]
#[test]
fn z_test_has_separate_stats_and_data_forms() {
    assert_rel("ZTestStats(0, 6, 1, 1).z", 6.0, 1e-15);
    assert_rel("ZTest(0, 6, 1, 1).z", 6.0, 1e-15);
    assert_rel("ZTestData(0, 1, 6, 6).z", 6.0 * 2f64.sqrt(), 1e-14);
    assert_rel("ZTest(0, 6, 1, 1).p", 1.973_175_290_075_074_4e-9, 1e-12);
    // A 2-point data list cannot be passed to the 4-argument summary form by accident.
    assert_err("ZTest(0, 1, 6, 6, 7)");
}

#[cfg(feature = "stats")]
#[test]
fn t_interval_data_form_keeps_all_points() {
    // TInterval(1, 2, 3): mean 2, s = 1, t*(0.975, 2) = 4.302652729911275
    let (lo, hi) = interval("TInterval(1, 2, 3)");
    assert!((lo - (-0.484_137_711_750_329_4)).abs() < 1e-12, "lo = {lo}");
    assert!((hi - 4.484_137_711_750_329).abs() < 1e-12, "hi = {hi}");
    let (lo2, hi2) = interval("TIntervalData(1, 2, 3)");
    assert_eq!((lo, hi), (lo2, hi2));
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn trailing_fraction_is_data_not_confidence_level() {
    // All three points are data; 95% confidence.
    let (lo, hi) = interval("TInterval(0.2, 0.5, 0.7)");
    assert!((lo - (-0.158_494_281_270_910_7)).abs() < 1e-12, "lo = {lo}");
    assert!((hi - 1.091_827_614_604_244).abs() < 1e-12, "hi = {hi}");
    assert_rel("moe(0.2, 0.5, 0.7)", 0.625_160_947_937_577_4, 1e-12);
    // A confidence level for data forms must be given as a percentage.
    let (lo90, hi90) = interval("TInterval(1, 2, 3, 90%)");
    assert!((lo90 - 0.314_145_539_152_951_5).abs() < 1e-12);
    assert!((hi90 - 3.685_854_460_847_048_5).abs() < 1e-12);
    // Summary forms accept either a fraction or a percentage.
    assert_eq!(
        interval("ZInterval(100 m, 15 m, 100, 0.9)"),
        interval("ZInterval(100 m, 15 m, 100, 90%)")
    );
    assert_eq!(
        interval("TIntervalStats(2, 1, 3)"),
        interval("TInterval(1, 2, 3)")
    );
    assert_eq!(
        interval("ZIntervalData(1, 1, 2, 3)"),
        interval("ZIntervalStats(2, 1, 3)")
    );
}

// ── Findings 4, 12: incomplete gamma/beta convergence and robust inverses ──

#[cfg(feature = "distributions")]
#[test]
fn chi_square_cdf_is_accurate_for_huge_df() {
    assert_rel("chisqcdf(1e6, 1e6)", 0.500_188_063_196_605_5, 1e-13);
    assert_rel("chisqcdf(1e5, 1e5)", 0.500_594_708_104_780_8, 1e-12);
    assert_rel("chisqcdf(10000, 10000)", 0.501_880_634_033_817_6, 1e-12);
    assert_rel("chisqcdf(3, 2)", 0.427_593_295_529_120_2, 1e-14);
    // Shapes beyond the series range use Temme's expansion instead of failing.
    let p = scalar("chisqcdf(2e15, 2e15)");
    assert!((p - 0.5).abs() < 1e-7 && p > 0.5);
}

#[cfg(feature = "distributions")]
#[test]
fn inverse_distributions_converge_in_the_tails() {
    assert_rel("invchisq(0.5, 1e6)", 999_999.333_333_412_4, 1e-14);
    assert_rel("invchisq(0.95, 3)", 7.814_727_903_251_178, 1e-14);
    assert_rel("invchisq(1e-8, 4)", 0.000_282_856_046_672_257_7, 1e-12);
    assert_rel("invt(1e-10, 3)", -2_225.769_284_683_093, 1e-13);
    assert_rel("invt(0.975, 10)", 2.228_138_851_986_274, 1e-14);
    assert_rel("invt(0.999, 2.5)", 13.822_193_110_865_96, 1e-13);
    // Results that overflow f64 are errors rather than a wrong finite number.
    assert_err("invt(1e-300, 0.1)");
}

#[cfg(feature = "distributions")]
#[test]
fn student_t_cdf_is_accurate_for_large_df() {
    assert_rel("tcdf(1e5, 1.5)", 0.933_191_220_238_584_7, 1e-14);
    assert_rel("tcdf(2.5, -40)", 7.097_817_145_246_691e-5, 1e-13);
}

// ── Finding 5: erf / normcdf tails ──

#[cfg(feature = "distributions")]
#[test]
fn normal_cdf_keeps_relative_precision_in_the_tails() {
    assert_eq!(scalar("normcdf(0)"), 0.5);
    assert_rel("normcdf(1.96)", 0.975_002_104_851_779_6, 1e-15);
    assert_rel("normcdf(-5)", 2.866_515_718_791_939e-7, 1e-13);
    assert_rel("normcdf(-8)", 6.220_960_574_271_784e-16, 1e-13);
    assert_rel("normcdf(-20)", 2.753_624_118_606_233_7e-89, 1e-12);
}

#[cfg(feature = "stats")]
#[test]
fn two_sided_p_values_do_not_cancel_to_zero() {
    // z = 9 → p = 2·Φ(−9) = 2.257e-19, which `2·(1 − Φ(9))` would round to 0.
    let p = hash_value("ZTest(0, 9, 1, 1)", "p");
    assert!(
        (p / 2.257_176_811_907_681e-19 - 1.0).abs() < 1e-12,
        "p = {p:e}"
    );
}

// ── Finding 6: affine temperatures in statistics ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn spreads_of_temperatures_are_kelvin_differences() {
    assert_eq!(display("std(10 degC, 20 degC)"), "7.0710678118654755 K");
    assert_eq!(display("range(10 degC, 20 degC)"), "10 K");
    assert_eq!(display("mad(10 degC, 20 degC)"), "5 K");
    assert_eq!(display("iqr(10 degC, 20 degC)"), "5 K");
    assert_rel("var(10 degC, 20 degC)", 50.0, 1e-15);
    assert_err("sum(10 degC, 20 degC)");
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn locations_of_temperatures_stay_temperatures() {
    assert_eq!(display("mean(10 degC, 20 degC)"), "15 °C");
    assert_eq!(display("median(10 degC, 20 degC, 30 degC)"), "20 °C");
    assert_eq!(display("max(10 degC, 20 degC)"), "20 °C");
}

#[cfg(all(feature = "units", feature = "stats", feature = "distributions"))]
#[test]
fn distribution_parameters_treat_sigma_as_a_difference() {
    // σ = 2 °C means 2 K, not 275.15 K.
    assert_rel(
        "normcdf(12 degC, 10 degC, 2 degC)",
        0.841_344_746_068_542_9,
        1e-15,
    );
    let x = Abacus::standard()
        .eval_scalar("invnorm(0.975, 10 degC, 2 degC)")
        .unwrap();
    assert!((x.amount() - 13.919_927_969_080_108).abs() < 1e-12);
    let (lo, hi) = match eval("ZInterval(20 degC, 2 degC, 4)").unwrap() {
        EvalResult::Interval(i) => (i.lo.amount(), i.hi.amount()),
        _ => panic!(),
    };
    assert!((lo - 18.040_036_015_459_947).abs() < 1e-12);
    assert!((hi - 21.959_963_984_540_053).abs() < 1e-12);
    assert_rel("zscore(12 degC, 10 degC, 2 degC)", 1.0, 1e-15);
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn sample_sizes_must_be_dimensionless_integers() {
    assert_err("TTest(15 degC, 10 degC, 20 degC, 30 degC)");
    assert_err("TTest(100 m, 105 m, 15 m, 25 m)");
    assert_err("ZTest(100 m, 105 m, 15 m, 2.5)");
    assert_eq!(
        hash_value("TTestData(15 degC, 10 degC, 20 degC, 30 degC)", "n"),
        3.0
    );
}

// ── Finding 7: unit-aware floored modulo ──

#[cfg(feature = "units")]
#[test]
fn modulo_uses_canonical_units_and_floored_sign() {
    assert_rel("mod(1 m, 30 cm)", 0.1, 1e-12);
    assert_eq!(display("mod(7 m, 3)"), "1 m");
    assert_eq!(scalar("mod(-7, 3)"), 2.0);
    assert_eq!(scalar("mod(7, -3)"), -2.0);
    assert_eq!(scalar("-7 % 3"), 2.0);
    assert_err("mod(5 m, 0)");
}

// ── Finding 8: predict argument order ──

#[cfg(feature = "stats")]
#[test]
fn prediction_argument_order_is_fixed_per_name() {
    assert_eq!(scalar("predict(4, 1, 2, 3, 2, 4, 6)"), 8.0);
    assert_eq!(scalar("linreg_predict(1, 2, 3, 2, 4, 6, 4)"), 8.0);
    assert_eq!(scalar("LinRegPredict(1, 2, 3, 2, 4, 6, 4)"), 8.0);
}

// ── Findings 9 & 10: discrete distributions in log space, no O(k) loops ──

#[cfg(feature = "distributions")]
#[test]
fn discrete_distributions_do_not_overflow() {
    assert_rel("binompdf(2000, .5, 1000)", 0.017_839_011_145_854_32, 1e-13);
    assert_rel("binomcdf(2000, .5, 1000)", 0.508_919_505_572_927_2, 1e-13);
    assert_rel("binompdf(10, 0.3, 3)", 0.266_827_932, 1e-14);
    assert_rel("binomcdf(10, 0.3, 3)", 0.649_610_718_4, 1e-14);
    assert_rel("binompdf(1e6, 1e-6, 2)", 0.183_939_812_555_635_1, 1e-13);
    assert_rel(
        "hypgeompdf(2000, 1000, 1000, 500)",
        0.035_669_103_903_581_87,
        1e-13,
    );
    assert_rel(
        "hypgeomcdf(2000, 1000, 1000, 500)",
        0.517_834_551_951_790_8,
        1e-13,
    );
    assert_rel("hypgeomcdf(50, 10, 20, 3)", 0.364_968_286_776_836_8, 1e-13);
    assert_rel("hypgeompdf(50, 10, 20, 3)", 0.225_929_629_395_929_8, 1e-13);
    assert_rel("poissonpdf(1000, 1000)", 0.012_614_611_348_721_5, 1e-14);
    assert_rel("poissoncdf(3.5, 2)", 0.320_847_198_862_134_1, 1e-14);
    assert_eq!(scalar("binompdf(3e9, .5, 1)"), 0.0);
}

#[cfg(feature = "distributions")]
#[test]
fn huge_cdf_arguments_return_quickly() {
    let start = Instant::now();
    assert_eq!(scalar("poissoncdf(5, 1e9)"), 1.0);
    assert_rel("poissoncdf(1e9, 1e9)", 0.500_008_410_441_739, 1e-12);
    assert_rel("binomcdf(1e9, .5, 5e8)", 0.500_012_615_662_343_5, 1e-9);
    let h = scalar("hypgeomcdf(1e9, 5e8, 1e6, 5e5)");
    assert!(h > 0.5 && h < 0.501);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "took {:?}",
        start.elapsed()
    );
}

// ── Finding 11: lcm/gcd overflow and integer validation ──

#[test]
fn lcm_overflow_is_an_error_not_a_panic() {
    assert_err("lcm(4294967296, 4294967297)");
    assert_err("lcm(1e19, 3)");
    assert_eq!(scalar("lcm(4, 6)"), 12.0);
    assert_eq!(scalar("gcd(-12, 18)"), 6.0);
}

// ── Finding 13: irr ──

#[cfg(feature = "financial")]
#[test]
fn irr_uses_relative_tolerance_and_reports_failure() {
    assert_rel("irr(-1e-11, 2e-11)", 1.0, 1e-12);
    assert_rel("irr(-100, 110)", 0.1, 1e-12);
    assert_rel("irr(-1000, 300, 400, 500)", 0.088_963_394_693_344_76, 1e-10);
    // Multiple roots (0.1 and 0.2): the one nearest the 10% starting guess is returned.
    assert_rel("irr(-100, 230, -132)", 0.1, 1e-12);
    // No sign change → no IRR.
    assert_err("irr(100, 110)");
    assert_err("irr(-100, -110)");
}

// ── Finding 14: 1-PropZInt is the TI-84 Wald interval ──

#[cfg(feature = "stats")]
#[test]
fn one_prop_z_int_matches_ti84() {
    let (lo, hi) = interval("1-PropZInt(40, 100)");
    assert!((lo - 0.303_981_766_472_893_9).abs() < 1e-14, "lo = {lo}");
    assert!((hi - 0.496_018_233_527_106_2).abs() < 1e-14, "hi = {hi}");
}

// ── Finding 15: degenerate standard errors ──

#[test]
fn degenerate_standard_errors_are_errors() {
    assert_err("2-PropZTest(0, 10, 0, 10)");
    assert_err("2-PropZTest(10, 10, 10, 10)");
    assert_err("2-SampZTest(0 m, 1 m, 1 m, 5, 2 m, 5)");
    assert_err("TTestData(5, 1, 1, 1)");
    assert_err("ZTest(0, 1, 0, 10)");
}

// ── Finding 16: skewness / kurtosis definitions ──

#[cfg(feature = "stats")]
#[test]
fn skew_and_kurt_match_excel() {
    assert_rel("skew(1, 2, 3, 10)", 1.763_632_614_803_888, 1e-13);
    assert_rel("kurt(1, 2, 3, 10)", 3.228, 1e-13);
    assert_err("skew(2, 2, 2)");
    assert_err("kurt(2, 2, 2, 2)");
}

// ── Finding 17: F and χ² densities at zero ──

#[cfg(feature = "distributions")]
#[test]
fn densities_at_zero() {
    assert_eq!(scalar("fpdf(2, 5, 0)"), 1.0);
    assert_eq!(scalar("fpdf(3, 5, 0)"), 0.0);
    assert_eq!(scalar("fpdf(1, 5, 0)"), f64::INFINITY);
    assert_rel("fpdf(3, 5, 2)", 0.142_896_390_907_531_3, 1e-13);
    assert_eq!(scalar("chisqpdf(2, 0)"), 0.5);
    assert_eq!(scalar("chisqpdf(3, 0)"), 0.0);
}

// ── Finding 19: non-integer inputs and consistent limits ──

#[cfg(feature = "distributions")]
#[test]
fn integer_arguments_are_validated_not_rounded() {
    for expr in [
        "factorial(2.5)",
        "nCr(5, 2.5)",
        "nPr(5, 2.5)",
        "binompdf(10, .5, 2.5)",
        "binomcdf(10.5, .5, 2)",
        "gcd(2.5, 5)",
        "lcm(2.5, 5)",
        "geomcdf(.5, 2.5)",
        "geompdf(.5, 2.5)",
        "poissonpdf(2, 1.5)",
        "hypgeompdf(10, 5, 3.5, 1)",
        "factorial(-3)",
    ] {
        assert!(
            matches!(eval(expr), Err(AbacusError::IncompatibleFunctionArguments)),
            "{expr} should be rejected, got {:?}",
            eval(expr)
        );
    }
}

#[test]
fn combinatorics_limits_are_consistent() {
    assert_eq!(scalar("nCr(1001, 1)"), 1001.0);
    assert_eq!(scalar("nPr(171, 1)"), 171.0);
    assert_rel("nCr(1000, 500)", 2.702_882_409_454_366e299, 1e-12);
    assert_eq!(scalar("nCr(60, 30)"), 1.182_645_815_648_614_2e17);
    assert_eq!(scalar("factorial(20)"), 2.432_902_008_176_64e18);
    assert!(matches!(
        eval("factorial(171)"),
        Err(AbacusError::EvaluationError(_))
    ));
    assert!(matches!(
        eval("nCr(5000, 2500)"),
        Err(AbacusError::EvaluationError(_))
    ));
}

// ── Finding 20: trig domains and exact values ──

#[cfg(feature = "units")]
#[test]
fn trig_domain_errors_and_exact_quadrants() {
    assert_err("asin(2)");
    assert_err("acos(-1.5)");
    assert_err("acosh(0.5)");
    assert_err("atanh(1)");
    assert_err("tan(90 deg)");
    assert_err("tan(270 deg)");
    assert_eq!(scalar("sin(180 deg)"), 0.0);
    assert_eq!(scalar("cos(90 deg)"), 0.0);
    assert_eq!(scalar("sin(-90 deg)"), -1.0);
    assert_eq!(scalar("sin(pi)"), 0.0);
    assert_eq!(scalar("tan(180 deg)"), 0.0);
    assert_rel("sin(30 deg)", 0.5, 1e-15);
}

#[test]
fn inverse_hyperbolics_are_plain_numbers() {
    let v = Abacus::standard().eval_scalar("atanh(0.5)").unwrap();
    assert!(v.unit.display.render().is_empty());
    assert_rel("atanh(0.5)", 0.549_306_144_334_054_8, 1e-15);
    assert_rel("asinh(1)", 0.881_373_587_019_543, 1e-15);
}

// ── Finding 21: regression consistency ──

#[cfg(feature = "stats")]
#[test]
fn regression_r_is_undefined_for_constant_y() {
    assert_err("corr(1, 2, 3, 5, 5, 5)");
    assert_err("linreg_r(1, 2, 3, 5, 5, 5)");
    assert_err("linreg_r2(1, 2, 3, 5, 5, 5)");
    let h = Abacus::standard()
        .eval_hash("linreg(1, 2, 3, 5, 5, 5)")
        .unwrap();
    assert!(h.get("r").is_none() && h.get("r2").is_none());
    assert_eq!(h.get("slope").unwrap().canonical, 0.0);
}

#[cfg(feature = "stats")]
#[test]
fn regression_reports_slope_standard_error() {
    // y = 2, 4, 5, 9 on x = 1..4: s = 0.9486832980505138, SE(b) = s/√5, SE(a) = s·√(1/4 + 6.25/5)
    let s = 0.948_683_298_050_513_1;
    let expr = "linreg(1, 2, 3, 4, 2, 4, 5, 9)";
    assert!((hash_value(expr, "se") - s).abs() < 1e-14);
    assert!((hash_value(expr, "slope_se") - s / 5f64.sqrt()).abs() < 1e-14);
    assert!((hash_value(expr, "intercept_se") - s * 1.5f64.sqrt()).abs() < 1e-14);
}

// ── Finding 22: Chi2Test argument layout ──

#[cfg(feature = "stats")]
#[test]
fn chi2_test_takes_observed_then_expected() {
    // observed (15, 25), expected (10, 30)
    assert_rel("Chi2Test(15, 25, 10, 30).chi2", 10.0 / 3.0, 1e-15);
    assert_rel(
        "Chi2Test(15, 25, 10, 30).p",
        0.067_889_154_861_829_02,
        1e-13,
    );
    assert_err("Chi2Test(1 m, 2 m, 1 m, 2 m)");
}

// ── Nits ──

#[cfg(feature = "stats")]
#[test]
fn mode_handles_ties_and_no_mode() {
    assert_eq!(scalar("mode(1, 1, 2, 2)"), 1.0);
    assert_eq!(scalar("mode(3, 1, 3, 2)"), 3.0);
    assert_eq!(scalar("mode(5)"), 5.0);
    assert_eq!(scalar("mode(0.1 + 0.2, 0.3, 1)"), 0.3);
    assert!(matches!(
        eval("mode(1, 2, 3)"),
        Err(AbacusError::EvaluationError(_))
    ));
}

#[cfg(feature = "stats")]
#[test]
fn zscore_rejects_non_positive_sigma() {
    assert_err("zscore(5, 3, -2)");
    assert_err("zscore(5, 3, 0)");
    assert_eq!(scalar("zscore(5, 3, 2)"), 1.0);
}

#[cfg(feature = "distributions")]
#[test]
fn exponential_distribution_keeps_precision_for_tiny_arguments() {
    assert_rel("expcdf(1e-20, 1)", 1e-20, 1e-15);
    assert_rel("invexp(1e-20, 1)", 1e-20, 1e-15);
}
