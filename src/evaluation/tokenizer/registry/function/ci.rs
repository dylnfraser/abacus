//! Confidence intervals in the style of the TI-84.
//!
//! Each name has exactly one signature. "Stats" forms take summary statistics and an optional
//! confidence level `C` (a fraction such as `0.9` or a percentage such as `90%`). "Data" forms
//! take the raw sample; because a trailing number could be a data point, their confidence
//! level can only be given as a **percentage** (`TInterval(1, 2, 3, 90%)`). The default
//! confidence level is 95%.
//!
//! | Function                          | Arguments                          |
//! |-----------------------------------|------------------------------------|
//! | `TInterval` = `TIntervalData`     | `(x₁, x₂, …, [C%])`                |
//! | `TIntervalStats`                  | `(x̄, s, n, [C])`                   |
//! | `ZInterval` = `ZIntervalStats`    | `(x̄, σ, n, [C])`                   |
//! | `ZIntervalData`                   | `(σ, x₁, x₂, …, [C%])`             |
//! | `1-PropZInt`                      | `(x, n, [C])` (Wald, as on the TI-84) |
//! | `2-SampTInt`                      | `(x̄₁, s₁, n₁, x̄₂, s₂, n₂, [C])` (Welch) |
//! | `2-SampZInt`                      | `(x̄₁, σ₁, n₁, x̄₂, σ₂, n₂, [C])`    |
//! | `2-PropZInt`                      | `(x₁, n₁, x₂, n₂, [C])` (Wald)     |
//! | `moe`                             | `(x₁, x₂, …, [C%])` (t-based)      |
//! | `tmoe` / `zmoe`                   | `(s, n, [C])` / `(σ, n, [C])`      |

use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::{normal::std_normal_quantile, student_t::compute_invt},
        math_helpers::{delta_unit, integer_arg, positive_spread, sample_size},
        operators::FunctionOp,
        stats::{compute_mean, compute_variance},
    },
    units::{eval_result::EvalResult, interval::Interval, unit::Unit},
};
use std::sync::Arc;

const DEFAULT_CONFIDENCE: f64 = 0.95;

/// Parses an explicit confidence level: a dimensionless fraction in (0, 1) or a percentage.
fn confidence_arg(value: &Value) -> Result<f64, AbacusError> {
    if !value.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let c = value.canonical;
    if !(c > 0.0 && c < 1.0) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(c)
}

/// Optional confidence level at `args[index]` for the summary-statistics forms.
fn optional_confidence(args: &[Value], index: usize) -> Result<f64, AbacusError> {
    args.get(index)
        .map_or(Ok(DEFAULT_CONFIDENCE), confidence_arg)
}

/// Splits a data-form argument list into `(data, confidence)`. Only a trailing *percentage*
/// is treated as a confidence level, so no data point is ever mistaken for one.
fn split_data_and_confidence(
    args: &[Value],
    min_data: usize,
) -> Result<(&[Value], f64), AbacusError> {
    let (data, conf) = match args.split_last() {
        Some((last, rest)) if last.unit.is_percent() => (rest, confidence_arg(last)?),
        _ => (args, DEFAULT_CONFIDENCE),
    };
    if data.len() < min_data {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let unit = &data[0].unit;
    if data.iter().any(|v| !v.unit.is_compatible_with(unit)) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    Ok((data, conf))
}

/// Two-sided critical value z* = Φ⁻¹((1 + C) / 2).
fn z_critical(confidence: f64) -> f64 {
    std_normal_quantile(f64::midpoint(1.0, confidence))
}

/// Two-sided critical value t* for the given confidence and degrees of freedom.
fn t_critical(confidence: f64, df: f64) -> Result<f64, AbacusError> {
    compute_invt(f64::midpoint(1.0, confidence), df).ok_or_else(|| {
        AbacusError::EvaluationError("t critical value failed to converge".to_string())
    })
}

/// `center ± moe` as an interval of readings in `unit`.
fn interval_around(center: f64, moe: f64, unit: &Arc<Unit>) -> Result<EvalResult, AbacusError> {
    let lo = Value::from_canonical(center - moe, Arc::clone(unit));
    let hi = Value::from_canonical(center + moe, Arc::clone(unit));
    Ok(EvalResult::Interval(Interval::new(lo, hi)?))
}

/// Sample mean, sample standard deviation and size of a data slice.
fn sample_summary(data: &[Value]) -> (f64, f64, f64) {
    (
        compute_mean(data),
        compute_variance(data, 1.0).sqrt(),
        data.len() as f64,
    )
}

// ── TInterval / TIntervalData(x₁, x₂, …, [C%]) ──
fn t_interval_data_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (data, conf) = split_data_and_confidence(args, 2)?;
    let (mean, s, n) = sample_summary(data);
    let moe = t_critical(conf, n - 1.0)? * s / n.sqrt();
    interval_around(mean, moe, &data[0].unit)
}

// ── TIntervalStats(x̄, s, n, [C]) ──
fn t_interval_stats_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mean = &args[0];
    let s = positive_spread(&args[1], &mean.unit)?;
    let n = sample_size(&args[2], 2)?;
    let conf = optional_confidence(args, 3)?;
    let moe = t_critical(conf, n - 1.0)? * s / n.sqrt();
    interval_around(mean.canonical, moe, &mean.unit)
}

// ── ZInterval / ZIntervalStats(x̄, σ, n, [C]) ──
fn z_interval_stats_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let mean = &args[0];
    let sigma = positive_spread(&args[1], &mean.unit)?;
    let n = sample_size(&args[2], 1)?;
    let conf = optional_confidence(args, 3)?;
    let moe = z_critical(conf) * sigma / n.sqrt();
    interval_around(mean.canonical, moe, &mean.unit)
}

// ── ZIntervalData(σ, x₁, x₂, …, [C%]) ──
fn z_interval_data_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (data, conf) = split_data_and_confidence(&args[1..], 1)?;
    let sigma = positive_spread(&args[0], &data[0].unit)?;
    let n = data.len() as f64;
    let moe = z_critical(conf) * sigma / n.sqrt();
    interval_around(compute_mean(data), moe, &data[0].unit)
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

// ── 1-PropZInt(x, n, [C]) — Wald interval p̂ ± z*·√(p̂(1−p̂)/n), as on the TI-84 ──
fn one_prop_z_int_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (x, n) = count_and_size(&args[0], &args[1])?;
    let conf = optional_confidence(args, 2)?;

    let p = x / n;
    let moe = z_critical(conf) * (p * (1.0 - p) / n).sqrt();

    let lo = Value::dimensionless(p - moe);
    let hi = Value::dimensionless(p + moe);
    Ok(EvalResult::Interval(Interval::new(lo, hi)?))
}

/// Parses `(x̄₁, s₁, n₁, x̄₂, s₂, n₂)` with each n ≥ `min_n`.
fn two_sample_args(
    args: &[Value],
    min_n: u64,
) -> Result<(f64, f64, f64, f64, f64, f64), AbacusError> {
    let mean1 = &args[0];
    let mean2 = &args[3];
    if !mean2.unit.is_compatible_with(&mean1.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let s1 = positive_spread(&args[1], &mean1.unit)?;
    let s2 = positive_spread(&args[4], &mean1.unit)?;
    let n1 = sample_size(&args[2], min_n)?;
    let n2 = sample_size(&args[5], min_n)?;
    Ok((mean1.canonical, s1, n1, mean2.canonical, s2, n2))
}

// ── 2-SampTInt(x̄₁, s₁, n₁, x̄₂, s₂, n₂, [C]) — interval for μ₁ − μ₂ (Welch) ──
fn two_samp_t_int_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (m1, s1, n1, m2, s2, n2) = two_sample_args(args, 2)?;
    let conf = optional_confidence(args, 6)?;

    let v1 = (s1 * s1) / n1;
    let v2 = (s2 * s2) / n2;
    let se = (v1 + v2).sqrt();

    // Welch–Satterthwaite degrees of freedom
    let num = (v1 + v2).powi(2);
    let den = (v1 * v1) / (n1 - 1.0) + (v2 * v2) / (n2 - 1.0);
    let df = num / den;

    let moe = t_critical(conf, df)? * se;
    interval_around(m1 - m2, moe, &delta_unit(&args[0].unit))
}

// ── 2-SampZInt(x̄₁, σ₁, n₁, x̄₂, σ₂, n₂, [C]) — interval for μ₁ − μ₂ ──
fn two_samp_z_int_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (m1, s1, n1, m2, s2, n2) = two_sample_args(args, 1)?;
    let conf = optional_confidence(args, 6)?;

    let se = ((s1 * s1) / n1 + (s2 * s2) / n2).sqrt();
    let moe = z_critical(conf) * se;
    interval_around(m1 - m2, moe, &delta_unit(&args[0].unit))
}

// ── 2-PropZInt(x₁, n₁, x₂, n₂, [C]) — Wald interval for p₁ − p₂ ──
fn two_prop_z_int_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (x1, n1) = count_and_size(&args[0], &args[1])?;
    let (x2, n2) = count_and_size(&args[2], &args[3])?;
    let conf = optional_confidence(args, 4)?;

    let p1 = x1 / n1;
    let p2 = x2 / n2;
    let diff = p1 - p2;
    let se = ((p1 * (1.0 - p1)) / n1 + (p2 * (1.0 - p2)) / n2).sqrt();
    let moe = z_critical(conf) * se;

    let lo = Value::dimensionless(diff - moe);
    let hi = Value::dimensionless(diff + moe);
    Ok(EvalResult::Interval(Interval::new(lo, hi)?))
}

// ── moe(x₁, x₂, …, [C%]) — t-based margin of error of the sample mean ──
fn moe_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (data, conf) = split_data_and_confidence(args, 2)?;
    let (_, s, n) = sample_summary(data);
    let moe = t_critical(conf, n - 1.0)? * s / n.sqrt();
    Ok(Value::from_canonical(moe, delta_unit(&data[0].unit)))
}

// ── tmoe(s, n, [C]) ──
fn tmoe_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let unit = &args[0].unit;
    let s = positive_spread(&args[0], unit)?;
    let n = sample_size(&args[1], 2)?;
    let conf = optional_confidence(args, 2)?;
    let moe = t_critical(conf, n - 1.0)? * s / n.sqrt();
    Ok(Value::from_canonical(moe, delta_unit(unit)))
}

// ── zmoe(σ, n, [C]) ──
fn zmoe_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let unit = &args[0].unit;
    let sigma = positive_spread(&args[0], unit)?;
    let n = sample_size(&args[1], 1)?;
    let conf = optional_confidence(args, 2)?;
    let moe = z_critical(conf) * sigma / n.sqrt();
    Ok(Value::from_canonical(moe, delta_unit(unit)))
}

pub fn register_ci() -> Vec<FunctionOp> {
    let mut ops = Vec::new();

    for name in &[
        "TInterval",
        "tinterval",
        "TInt",
        "t_interval",
        "ci",
        "tci",
        "TIntervalData",
        "tinterval_data",
    ] {
        ops.push(FunctionOp::eval_result(
            name,
            2,
            usize::MAX,
            t_interval_data_fn,
        ));
    }
    for name in &["TIntervalStats", "tinterval_stats"] {
        ops.push(FunctionOp::eval_result(name, 3, 4, t_interval_stats_fn));
    }

    for name in &[
        "ZInterval",
        "zinterval",
        "ZInt",
        "z_interval",
        "zci",
        "ZIntervalStats",
        "zinterval_stats",
    ] {
        ops.push(FunctionOp::eval_result(name, 3, 4, z_interval_stats_fn));
    }
    for name in &["ZIntervalData", "zinterval_data"] {
        ops.push(FunctionOp::eval_result(
            name,
            2,
            usize::MAX,
            z_interval_data_fn,
        ));
    }

    for name in &[
        "1-PropZInt",
        "1_PropZInt",
        "1PropZInt",
        "1_prop_z_int",
        "propzint",
        "propci",
    ] {
        ops.push(FunctionOp::eval_result(name, 2, 3, one_prop_z_int_fn));
    }

    for name in &[
        "2-SampTInt",
        "2_SampTInt",
        "2SampTInt",
        "2_samp_t_int",
        "samptint2",
    ] {
        ops.push(FunctionOp::eval_result(name, 6, 7, two_samp_t_int_fn));
    }

    for name in &[
        "2-SampZInt",
        "2_SampZInt",
        "2SampZInt",
        "2_samp_z_int",
        "sampzint2",
    ] {
        ops.push(FunctionOp::eval_result(name, 6, 7, two_samp_z_int_fn));
    }

    for name in &[
        "2-PropZInt",
        "2_PropZInt",
        "2PropZInt",
        "2_prop_z_int",
        "propzint2",
    ] {
        ops.push(FunctionOp::eval_result(name, 4, 5, two_prop_z_int_fn));
    }

    ops.push(FunctionOp::scalar("moe", 2, usize::MAX, moe_fn));
    ops.push(FunctionOp::scalar("tmoe", 2, 3, tmoe_fn));
    ops.push(FunctionOp::scalar("zmoe", 2, 3, zmoe_fn));

    ops
}
