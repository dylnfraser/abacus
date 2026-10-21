//! Hypothesis tests in the style of the TI-84.
//!
//! Every test is **two-sided** (H₁: μ ≠ μ₀, p ≠ p₀, …). Each function name has exactly one
//! signature; summary-statistics ("Stats") and raw-data ("Data") inputs use different names
//! rather than being guessed from the argument count:
//!
//! | Function                         | Arguments                          |
//! |----------------------------------|------------------------------------|
//! | `ZTest` = `ZTestStats`           | `(μ₀, x̄, σ, n)`                    |
//! | `ZTestData`                      | `(μ₀, σ, x₁, x₂, …)`               |
//! | `TTest` = `TTestStats`           | `(μ₀, x̄, s, n)`                    |
//! | `TTestData`                      | `(μ₀, x₁, x₂, …)`                  |
//! | `1-PropZTest`                    | `(p₀, x, n)`                       |
//! | `2-SampZTest`                    | `(σ₁, σ₂, x̄₁, n₁, x̄₂, n₂)`         |
//! | `2-SampTTest`                    | `(x̄₁, s₁, n₁, x̄₂, s₂, n₂)` (Welch) |
//! | `2-PropZTest`                    | `(x₁, n₁, x₂, n₂)` (pooled)        |
//! | `Chi2Test`                       | `(o₁, …, oₖ, e₁, …, eₖ)`           |

use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::{
            chi_square::compute_chisq_sf, normal::two_sided_normal_p, student_t::two_sided_t_p,
        },
        math_helpers::{delta_unit, integer_arg, positive_spread, sample_size},
        operators::FunctionOp,
        stats::{compute_mean, compute_variance},
    },
    units::{eval_result::EvalResult, hash::Hash, unit::Unit},
};
use std::sync::Arc;

fn non_convergent() -> AbacusError {
    AbacusError::EvaluationError("p-value computation failed to converge".to_string())
}

fn check_same_dims(values: &[Value], unit: &Unit) -> Result<(), AbacusError> {
    if values.iter().all(|v| v.unit.is_compatible_with(unit)) {
        Ok(())
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

/// Builds the result hash shared by the one-sample tests.
fn one_sample_hash(
    stat_name: &str,
    stat: f64,
    p_value: f64,
    mean: f64,
    unit: &Arc<Unit>,
    n: f64,
) -> Hash {
    let mut hash = Hash::new();
    hash.insert(stat_name, Value::dimensionless(stat));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("mean", Value::from_canonical(mean, Arc::clone(unit)));
    hash.insert("n", Value::dimensionless(n));
    hash
}

fn z_test_core(mu0: &Value, xbar: f64, sigma: f64, n: f64) -> Result<EvalResult, AbacusError> {
    let z = (xbar - mu0.canonical) / (sigma / n.sqrt());
    let p_value = two_sided_normal_p(z);
    Ok(EvalResult::Hash(one_sample_hash(
        "z", z, p_value, xbar, &mu0.unit, n,
    )))
}

// ── ZTest / ZTestStats(μ₀, x̄, σ, n) ──
fn z_test_stats_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mu0 = &args[0];
    if !args[1].unit.is_compatible_with(&mu0.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let sigma = positive_spread(&args[2], &mu0.unit)?;
    let n = sample_size(&args[3], 1)?;
    z_test_core(mu0, args[1].canonical, sigma, n)
}

// ── ZTestData(μ₀, σ, x₁, x₂, …) ──
fn z_test_data_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mu0 = &args[0];
    let sigma = positive_spread(&args[1], &mu0.unit)?;
    let data = &args[2..];
    check_same_dims(data, &mu0.unit)?;
    z_test_core(mu0, compute_mean(data), sigma, data.len() as f64)
}

fn t_test_core(mu0: &Value, xbar: f64, s: f64, n: f64) -> Result<EvalResult, AbacusError> {
    let t = (xbar - mu0.canonical) / (s / n.sqrt());
    let df = n - 1.0;
    let p_value = two_sided_t_p(df, t).ok_or_else(non_convergent)?;
    let mut hash = one_sample_hash("t", t, p_value, xbar, &mu0.unit, n);
    hash.insert("df", Value::dimensionless(df));
    hash.insert("s", Value::from_canonical(s, delta_unit(&mu0.unit)));
    Ok(EvalResult::Hash(hash))
}

// ── TTest / TTestStats(μ₀, x̄, s, n) ──
fn t_test_stats_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mu0 = &args[0];
    if !args[1].unit.is_compatible_with(&mu0.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let s = positive_spread(&args[2], &mu0.unit)?;
    let n = sample_size(&args[3], 2)?;
    t_test_core(mu0, args[1].canonical, s, n)
}

// ── TTestData(μ₀, x₁, x₂, …) ──
fn t_test_data_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mu0 = &args[0];
    let data = &args[1..];
    check_same_dims(data, &mu0.unit)?;
    let s = compute_variance(data, 1.0).sqrt();
    if s.is_nan() || s <= 0.0 {
        return Err(AbacusError::EvaluationError(
            "t-test is undefined for constant data (s = 0)".to_string(),
        ));
    }
    t_test_core(mu0, compute_mean(data), s, data.len() as f64)
}

/// Validates a proportion count `x` (integer, 0 ≤ x ≤ n) and sample size `n` (integer ≥ 1).
fn count_and_size(x: &Value, n: &Value) -> Result<(f64, f64), AbacusError> {
    let x = integer_arg(x)? as f64;
    let n = sample_size(n, 1)?;
    if x > n {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((x, n))
}

// ── 1-PropZTest(p₀, x, n) ──
fn one_prop_z_test_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    if !args[0].unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let p0 = args[0].canonical;
    if !(p0 > 0.0 && p0 < 1.0) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let (x, n) = count_and_size(&args[1], &args[2])?;

    let phat = x / n;
    let se = (p0 * (1.0 - p0) / n).sqrt();
    let z = (phat - p0) / se;
    let p_value = two_sided_normal_p(z);

    let mut hash = Hash::new();
    hash.insert("z", Value::dimensionless(z));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("phat", Value::dimensionless(phat));
    hash.insert("n", Value::dimensionless(n));

    Ok(EvalResult::Hash(hash))
}

// ── 2-SampZTest(σ₁, σ₂, x̄₁, n₁, x̄₂, n₂) ──
fn two_samp_z_test_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let x1 = &args[2];
    let x2 = &args[4];
    if !x2.unit.is_compatible_with(&x1.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let s1 = positive_spread(&args[0], &x1.unit)?;
    let s2 = positive_spread(&args[1], &x1.unit)?;
    let n1 = sample_size(&args[3], 1)?;
    let n2 = sample_size(&args[5], 1)?;

    let se = (s1 * s1 / n1 + s2 * s2 / n2).sqrt();
    let diff = x1.canonical - x2.canonical;
    let z = diff / se;
    let p_value = two_sided_normal_p(z);

    let mut hash = Hash::new();
    hash.insert("z", Value::dimensionless(z));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("diff", Value::from_canonical(diff, delta_unit(&x1.unit)));

    Ok(EvalResult::Hash(hash))
}

// ── 2-SampTTest(x̄₁, s₁, n₁, x̄₂, s₂, n₂) — Welch's unequal-variance test ──
fn two_samp_t_test_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let x1 = &args[0];
    let x2 = &args[3];
    if !x2.unit.is_compatible_with(&x1.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let s1 = positive_spread(&args[1], &x1.unit)?;
    let s2 = positive_spread(&args[4], &x1.unit)?;
    let n1 = sample_size(&args[2], 2)?;
    let n2 = sample_size(&args[5], 2)?;

    let v1 = (s1 * s1) / n1;
    let v2 = (s2 * s2) / n2;
    let se = (v1 + v2).sqrt();
    let diff = x1.canonical - x2.canonical;
    let t = diff / se;

    // Welch–Satterthwaite degrees of freedom
    let num = (v1 + v2) * (v1 + v2);
    let den = (v1 * v1) / (n1 - 1.0) + (v2 * v2) / (n2 - 1.0);
    let df = num / den;

    let p_value = two_sided_t_p(df, t).ok_or_else(non_convergent)?;

    let mut hash = Hash::new();
    hash.insert("t", Value::dimensionless(t));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("df", Value::dimensionless(df));
    hash.insert("diff", Value::from_canonical(diff, delta_unit(&x1.unit)));

    Ok(EvalResult::Hash(hash))
}

// ── 2-PropZTest(x₁, n₁, x₂, n₂) — pooled standard error ──
fn two_prop_z_test_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (x1, n1) = count_and_size(&args[0], &args[1])?;
    let (x2, n2) = count_and_size(&args[2], &args[3])?;

    let p1 = x1 / n1;
    let p2 = x2 / n2;
    let p_pooled = (x1 + x2) / (n1 + n2);
    if p_pooled == 0.0 || p_pooled == 1.0 {
        return Err(AbacusError::EvaluationError(
            "2-PropZTest is undefined when the pooled proportion is 0 or 1".to_string(),
        ));
    }
    let se = (p_pooled * (1.0 - p_pooled) * (1.0 / n1 + 1.0 / n2)).sqrt();
    let diff = p1 - p2;
    let z = diff / se;
    let p_value = two_sided_normal_p(z);

    let mut hash = Hash::new();
    hash.insert("z", Value::dimensionless(z));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("p1", Value::dimensionless(p1));
    hash.insert("p2", Value::dimensionless(p2));
    hash.insert("diff", Value::dimensionless(diff));

    Ok(EvalResult::Hash(hash))
}

// ── Chi2Test(o₁, …, oₖ, e₁, …, eₖ) ──
// χ² goodness-of-fit test: the first half of the arguments are the observed counts and the
// second half the expected counts, with df = k − 1. (This differs from the TI-84's χ²-Test,
// which is a test of independence on a matrix.)
fn chi2_test_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    if args.len() < 4 || !args.len().is_multiple_of(2) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    if args.iter().any(|v| !v.unit.is_dimensionless()) {
        return Err(AbacusError::IncompatibleDimensions);
    }

    let k = args.len() / 2;
    let (obs_slice, exp_slice) = args.split_at(k);

    let mut chi2 = 0.0;
    for (o, e) in obs_slice.iter().zip(exp_slice) {
        let e_val = e.canonical;
        if e_val.is_nan() || e_val <= 0.0 || o.canonical.is_nan() || o.canonical < 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        let diff = o.canonical - e_val;
        chi2 += (diff * diff) / e_val;
    }

    let df = (k - 1) as f64;
    let p_value = compute_chisq_sf(df, chi2).ok_or_else(non_convergent)?;

    let mut hash = Hash::new();
    hash.insert("chi2", Value::dimensionless(chi2));
    hash.insert("p", Value::dimensionless(p_value));
    hash.insert("p_value", Value::dimensionless(p_value));
    hash.insert("df", Value::dimensionless(df));

    Ok(EvalResult::Hash(hash))
}

pub fn register_hypothesis() -> Vec<FunctionOp> {
    let mut ops = Vec::new();

    for name in &["ZTest", "ztest", "Z_Test", "ZTestStats", "ztest_stats"] {
        ops.push(FunctionOp::eval_result(name, 4, 4, z_test_stats_fn));
    }
    for name in &["ZTestData", "ztest_data"] {
        ops.push(FunctionOp::eval_result(name, 3, usize::MAX, z_test_data_fn));
    }

    for name in &["TTest", "ttest", "T_Test", "TTestStats", "ttest_stats"] {
        ops.push(FunctionOp::eval_result(name, 4, 4, t_test_stats_fn));
    }
    for name in &["TTestData", "ttest_data"] {
        ops.push(FunctionOp::eval_result(name, 3, usize::MAX, t_test_data_fn));
    }

    for name in &["1-PropZTest", "1_PropZTest", "1PropZTest", "propztest"] {
        ops.push(FunctionOp::eval_result(name, 3, 3, one_prop_z_test_fn));
    }

    for name in &["2-SampZTest", "2_SampZTest", "2SampZTest", "sampztest2"] {
        ops.push(FunctionOp::eval_result(name, 6, 6, two_samp_z_test_fn));
    }

    for name in &["2-SampTTest", "2_SampTTest", "2SampTTest", "sampttest2"] {
        ops.push(FunctionOp::eval_result(name, 6, 6, two_samp_t_test_fn));
    }

    for name in &["2-PropZTest", "2_PropZTest", "2PropZTest", "propztest2"] {
        ops.push(FunctionOp::eval_result(name, 4, 4, two_prop_z_test_fn));
    }

    for name in &["Chi2Test", "chi2test", "Chi2_Test"] {
        ops.push(FunctionOp::eval_result(name, 4, usize::MAX, chi2_test_fn));
    }

    ops
}
