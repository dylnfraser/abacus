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
use abacus::{Abacus, AbacusError, EvalResult, Hash, Value};

fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    Abacus::standard().eval(expr)
}

fn eval_scalar(expr: &str) -> Result<Value, AbacusError> {
    Abacus::standard().eval_scalar(expr)
}

fn eval_hash(expr: &str) -> Result<Hash, AbacusError> {
    Abacus::standard().eval_hash(expr)
}

// ── Linear Regression Hash Output (linreg, LinReg) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_section5_linear_regression_expanded() {
    let calc = Abacus::standard();

    // linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m) -> slope = 10 m/s
    let slope = calc
        .eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope")
        .unwrap();
    assert_eq!(slope.to_display(), "10 m/s");

    // linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).intercept -> 5 m
    let icpt = calc
        .eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).intercept")
        .unwrap();
    assert_eq!(icpt.to_display(), "5 m");

    // Prediction arithmetic: linreg(...).slope * 5 s -> 50 m
    let pred = calc
        .eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s")
        .unwrap();
    assert_eq!(pred.to_display(), "50 m");
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_hash_output() {
    // linreg returns a Hash containing slope, intercept, r2, r, se, mean_x, mean_y
    let hash = eval_hash("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").unwrap();

    let slope = hash.get("slope").unwrap();
    assert_eq!(slope.to_display(), "10 m/s");

    let intercept = hash.get("intercept").unwrap();
    assert_eq!(intercept.to_display(), "0 m");

    let r2 = hash.get("r2").unwrap();
    assert_eq!(r2.to_display(), "1");

    let r = hash.get("r").unwrap();
    assert_eq!(r.to_display(), "1");

    let mean_x = hash.get("mean_x").unwrap();
    assert_eq!(mean_x.to_display(), "2.5 s");

    let mean_y = hash.get("mean_y").unwrap();
    assert_eq!(mean_y.to_display(), "25 m");
}

// ── Natural Language Dot Property Access (.intercept, .slope, .r2, etc.) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_dot_property_access_on_hash() {
    // Direct dot property access
    let slope = eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope").unwrap();
    assert_eq!(slope.to_display(), "10 m/s");

    let intercept =
        eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).intercept").unwrap();
    assert_eq!(intercept.to_display(), "5 m");

    let r2 = eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).r2").unwrap();
    assert_eq!(r2.to_display(), "1");

    let r = eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).r").unwrap();
    assert_eq!(r.to_display(), "1");

    // Property aliases (.m, .b)
    let m_alias = eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).m").unwrap();
    assert_eq!(m_alias.to_display(), "10 m/s");

    let b_alias = eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).b").unwrap();
    assert_eq!(b_alias.to_display(), "5 m");

    // Arithmetic chained with property access
    let intercept_plus =
        eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).intercept + 10 m").unwrap();
    assert_eq!(intercept_plus.to_display(), "15 m");

    let slope_times_time =
        eval_scalar("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s").unwrap();
    assert_eq!(slope_times_time.to_display(), "50 m");
}

// ── Linear Regression Slope (linreg_slope) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_slope_with_units() {
    let slope = eval_scalar("linreg_slope(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").unwrap();
    assert_eq!(slope.to_display(), "10 m/s");
    assert_eq!(slope.canonical, 10.0);
}

// ── Linear Regression Intercept (linreg_intercept) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_intercept_with_units() {
    let intercept =
        eval_scalar("linreg_intercept(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m)").unwrap();
    assert_eq!(intercept.to_display(), "5 m");
    assert_eq!(intercept.canonical, 5.0);
}

// ── Coefficient of Determination R^2 (linreg_r2, r2) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_r2_and_r() {
    let r2_val = eval_scalar("linreg_r2(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").unwrap();
    assert_eq!(r2_val.to_display(), "1");
    assert!((r2_val.canonical - 1.0).abs() < 1e-10);

    let r_val = eval_scalar("linreg_r(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").unwrap();
    assert_eq!(r_val.to_display(), "1");
    assert!((r_val.canonical - 1.0).abs() < 1e-10);

    let r_neg = eval_scalar("linreg_r(1 s, 2 s, 3 s, 4 s, 40 m, 30 m, 20 m, 10 m)").unwrap();
    assert!((r_neg.canonical - (-1.0)).abs() < 1e-10);
}

// ── Prediction (linreg_predict, predict) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_prediction() {
    let pred = eval_scalar("predict(10 s, 1 s, 2 s, 3 s, 4 s, 7 m, 12 m, 17 m, 22 m)").unwrap();
    assert_eq!(pred.to_display(), "52 m");

    let pred_end =
        eval_scalar("linreg_predict(1 s, 2 s, 3 s, 4 s, 7 m, 12 m, 17 m, 22 m, 10 s)").unwrap();
    assert_eq!(pred_end.to_display(), "52 m");
}

// ── Standard Error of Estimate (linreg_se) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_linear_regression_standard_error() {
    let se = eval_scalar("linreg_se(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").unwrap();
    assert!((se.canonical - 0.0).abs() < 1e-10);
}

// ── Error cases ──

#[test]
fn test_linear_regression_error_cases() {
    assert!(eval("linreg(1 s, 2 s, 3 s, 10 m, 20 m)").is_err());
    assert!(eval("linreg(1 s, 2 m, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)").is_err());
}
