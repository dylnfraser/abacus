//! Descriptive statistics.
//!
//! Conventions:
//! - `var`/`std`/`cov` (and the `_s` aliases) are *sample* statistics (n − 1 denominator);
//!   `var_p`/`std_p`/`cov_p` are population statistics.
//! - `skew` and `kurt` are the sample-adjusted estimators used by Excel's `SKEW` and `KURT`;
//!   `kurt` is *excess* kurtosis (0 for a normal distribution).
//! - `quantile`, `percentile` and `iqr` use linear interpolation between order statistics
//!   (Hyndman–Fan type 7, the same as Excel's `PERCENTILE.INC` and R's default).
//! - Affine units (°C, °F): location results (mean, median, min, …) are temperatures in the
//!   input unit; spread results (std, var, range, mad, iqr, …) are temperature differences and
//!   are reported in kelvin. `sum` of affine temperatures is rejected.

use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        math_helpers::{delta_unit, positive_spread},
        operators::FunctionOp,
    },
    units::unit::Unit,
};
use std::sync::Arc;

fn check_compatible_units(args: &[Value]) -> Result<&Arc<Unit>, AbacusError> {
    if args.is_empty() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let first_unit = &args[0].unit;
    for val in args {
        if !val.unit.is_compatible_with(first_unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
    }
    Ok(first_unit)
}

/// Builds the unit for the product of two spreads (variance, covariance).
fn product_unit(a: &Arc<Unit>, b: &Arc<Unit>) -> Arc<Unit> {
    let a = delta_unit(a);
    let b = delta_unit(b);
    Arc::new(Unit {
        scalar: a.scalar * b.scalar,
        offset: 0.0,
        dimensions: a.dimensions + b.dimensions,
        display: a.display.multiply(&b.display),
    })
}

fn spread_value(canonical: f64, unit: &Arc<Unit>) -> Value {
    Value::from_canonical(canonical, delta_unit(unit))
}

fn location_value(canonical: f64, unit: &Arc<Unit>) -> Value {
    Value::from_canonical(canonical, Arc::clone(unit))
}

fn sorted_canonicals(data: &[Value]) -> Vec<f64> {
    let mut values: Vec<f64> = data.iter().map(|v| v.canonical).collect();
    // `total_cmp` is a total order where only bitwise-identical floats compare equal, so an
    // unstable sort gives the same result as a stable one with a much smaller instantiation.
    values.sort_unstable_by(f64::total_cmp);
    values
}

fn sum_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if first_unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("sum"));
    }
    let sum: f64 = args.iter().map(|v| v.canonical).sum();
    Ok(location_value(sum, first_unit))
}

fn mean_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    Ok(location_value(compute_mean(args), first_unit))
}

fn min_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let min_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::INFINITY, f64::min);
    Ok(location_value(min_val, first_unit))
}

fn max_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let max_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(location_value(max_val, first_unit))
}

fn range_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let (min_val, max_val) = args
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.canonical), hi.max(v.canonical))
        });
    Ok(spread_value(max_val - min_val, first_unit))
}

fn median_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let values = sorted_canonicals(args);
    let len = values.len();
    let median_val = if len.is_multiple_of(2) {
        f64::midpoint(values[len / 2 - 1], values[len / 2])
    } else {
        values[len / 2]
    };
    Ok(location_value(median_val, first_unit))
}

/// mode(data...) — the most frequent value.
///
/// Values are compared with a relative tolerance of 1e-12 (so `0.1 + 0.2` and `0.3` count as
/// equal). If several values are equally frequent, the smallest is returned. If every value
/// occurs exactly once (and there is more than one value) there is no mode and an error is
/// returned, matching Excel's `MODE`.
fn mode_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let values = sorted_canonicals(args);
    let same = |a: f64, b: f64| a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs());

    let mut best = (values[0], 0usize);
    let mut i = 0;
    while i < values.len() {
        let start = values[i];
        let mut j = i + 1;
        while j < values.len() && same(start, values[j]) {
            j += 1;
        }
        if j - i > best.1 {
            best = (start, j - i);
        }
        i = j;
    }

    if best.1 == 1 && values.len() > 1 {
        return Err(AbacusError::EvaluationError(
            "mode is undefined: all values are distinct".to_string(),
        ));
    }
    Ok(location_value(best.0, first_unit))
}

/// Splits interleaved args into two equal halves: `(x_data, y_data, n)`.
/// Checks element count and verifies intra-slice unit consistency.
pub fn parse_paired_data(args: &[Value]) -> Result<(&[Value], &[Value], usize), AbacusError> {
    if args.len() < 4 || !args.len().is_multiple_of(2) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let n = args.len() / 2;
    let x_data = &args[..n];
    let y_data = &args[n..];
    check_compatible_units(x_data)?;
    check_compatible_units(y_data)?;
    Ok((x_data, y_data, n))
}

#[must_use]
pub fn compute_mean(args: &[Value]) -> f64 {
    args.iter().map(|v| v.canonical).sum::<f64>() / (args.len() as f64)
}

/// Computes the variance of a single dataset with delta-degrees-of-freedom `ddof`.
/// `ddof = 1` → sample variance, `ddof = 0` → population variance.
#[must_use]
pub fn compute_variance(args: &[Value], ddof: f64) -> f64 {
    let n = args.len() as f64;
    // Affine readings (°C, °F) are stored in kelvin on a large offset, so `20 °F` comes back
    // as 19.99999999999995 °F. When every reading shares one affine unit, compute on the
    // amounts as displayed (float noise removed) and rescale, avoiding that cancellation error.
    if let Some(first) = args.first()
        && first.unit.is_affine()
        && args
            .iter()
            .all(|v| v.unit.scalar == first.unit.scalar && v.unit.offset == first.unit.offset)
    {
        let amounts: Vec<f64> = args
            .iter()
            .map(|v| crate::units::value::snap_float_noise(v.amount()))
            .collect();
        let mean = amounts.iter().sum::<f64>() / n;
        let var = amounts.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / (n - ddof);
        return var * first.unit.scalar * first.unit.scalar;
    }
    let mean = compute_mean(args);
    args.iter()
        .map(|v| (v.canonical - mean).powi(2))
        .sum::<f64>()
        / (n - ddof)
}

/// Computes Pearson correlation coefficient between two datasets.
pub fn compute_pearson_r(x_data: &[Value], y_data: &[Value]) -> Result<f64, AbacusError> {
    let x_mean = compute_mean(x_data);
    let y_mean = compute_mean(y_data);

    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;

    for (x, y) in x_data.iter().zip(y_data) {
        let dx = x.canonical - x_mean;
        let dy = y.canonical - y_mean;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }

    let denom = (sxx * syy).sqrt();
    if denom == 0.0 {
        // Correlation is undefined when x or y is constant.
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    Ok(sxy / denom)
}

/// Computes the covariance and its unit (product of the two spread units) for paired data.
fn compute_covariance(x_data: &[Value], y_data: &[Value], ddof: f64) -> (f64, Arc<Unit>) {
    let n = x_data.len() as f64;
    let x_mean = compute_mean(x_data);
    let y_mean = compute_mean(y_data);

    let cov_sum: f64 = x_data
        .iter()
        .zip(y_data)
        .map(|(x, y)| (x.canonical - x_mean) * (y.canonical - y_mean))
        .sum();

    (
        cov_sum / (n - ddof),
        product_unit(&x_data[0].unit, &y_data[0].unit),
    )
}

fn variance_with(args: &[Value], ddof: f64) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if (args.len() as f64) <= ddof {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value::from_canonical(
        compute_variance(args, ddof),
        product_unit(first_unit, first_unit),
    ))
}

fn std_with(args: &[Value], ddof: f64) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if (args.len() as f64) <= ddof {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(spread_value(
        compute_variance(args, ddof).sqrt(),
        first_unit,
    ))
}

fn var_fn(args: &[Value]) -> Result<Value, AbacusError> {
    variance_with(args, 1.0)
}

fn std_fn(args: &[Value]) -> Result<Value, AbacusError> {
    std_with(args, 1.0)
}

fn var_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    variance_with(args, 0.0)
}

fn std_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    std_with(args, 0.0)
}

/// Type-7 quantile of already-sorted values.
fn quantile_sorted(values: &[f64], q: f64) -> f64 {
    if values.len() == 1 {
        return values[0];
    }
    let pos = q * ((values.len() - 1) as f64);
    let idx = pos.floor() as usize;
    let frac = pos - (idx as f64);

    if idx >= values.len() - 1 {
        values[values.len() - 1]
    } else {
        values[idx] + frac * (values[idx + 1] - values[idx])
    }
}

/// Shared implementation of `quantile(data..., q)` (q ∈ [0, 1]) and
/// `percentile(data..., p)` (p ∈ [0, 100]).
fn quantile_with_scale(args: &[Value], scale: f64) -> Result<Value, AbacusError> {
    if args.len() < 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let q_arg = &args[args.len() - 1];
    if !q_arg.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }

    let q = q_arg.canonical;
    if !(0.0..=scale).contains(&q) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let data = &args[..args.len() - 1];
    let first_unit = check_compatible_units(data)?;
    let val = quantile_sorted(&sorted_canonicals(data), q / scale);
    Ok(location_value(val, first_unit))
}

/// quantile(data..., q) where q in [0, 1]
fn quantile_fn(args: &[Value]) -> Result<Value, AbacusError> {
    quantile_with_scale(args, 1.0)
}

/// percentile(data..., p) where p in [0, 100]
fn percentile_fn(args: &[Value]) -> Result<Value, AbacusError> {
    quantile_with_scale(args, 100.0)
}

/// iqr(data...) -> Q3 - Q1
fn iqr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let values = sorted_canonicals(args);
    let iqr = quantile_sorted(&values, 0.75) - quantile_sorted(&values, 0.25);
    Ok(spread_value(iqr, first_unit))
}

/// `corr(x_range`, `y_range`) or corr(x1..xN, y1..yN)
fn corr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, _) = parse_paired_data(args)?;
    let r = compute_pearson_r(x_data, y_data)?;
    Ok(Value::dimensionless(r))
}

fn cov_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, _) = parse_paired_data(args)?;
    let (cov, unit) = compute_covariance(x_data, y_data, 1.0);
    Ok(Value::from_canonical(cov, unit))
}

fn cov_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, _) = parse_paired_data(args)?;
    let (cov, unit) = compute_covariance(x_data, y_data, 0.0);
    Ok(Value::from_canonical(cov, unit))
}

fn geomean_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mut log_sum = 0.0;
    for v in args {
        if v.canonical <= 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        log_sum += v.canonical.ln();
    }
    Ok(location_value((log_sum / n).exp(), first_unit))
}

fn harmean_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mut inv_sum = 0.0;
    for v in args {
        if v.canonical == 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        inv_sum += 1.0 / v.canonical;
    }
    Ok(location_value(n / inv_sum, first_unit))
}

/// Returns `(n, Σ((x - mean)/s)^power)` with `s` the sample standard deviation.
fn standardized_moment_sum(args: &[Value], power: i32) -> Result<(f64, f64), AbacusError> {
    check_compatible_units(args)?;
    let n = args.len() as f64;
    let mean = compute_mean(args);
    let s = compute_variance(args, 1.0).sqrt();
    if s.is_nan() || s <= 0.0 {
        return Err(AbacusError::EvaluationError(
            "skewness and kurtosis are undefined for constant data".to_string(),
        ));
    }
    let sum = args
        .iter()
        .map(|v| ((v.canonical - mean) / s).powi(power))
        .sum();
    Ok((n, sum))
}

/// skew(data...) — sample skewness `n / ((n-1)(n-2)) · Σ((x - x̄)/s)³` (Excel `SKEW`).
fn skew_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() < 3 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let (n, m3) = standardized_moment_sum(args, 3)?;
    Ok(Value::dimensionless(n / ((n - 1.0) * (n - 2.0)) * m3))
}

/// kurt(data...) — sample excess kurtosis (Excel `KURT`):
/// `n(n+1) / ((n-1)(n-2)(n-3)) · Σ((x - x̄)/s)⁴ − 3(n-1)² / ((n-2)(n-3))`.
fn kurt_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() < 4 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let (n, m4) = standardized_moment_sum(args, 4)?;
    let a = n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0));
    let b = 3.0 * (n - 1.0) * (n - 1.0) / ((n - 2.0) * (n - 3.0));
    Ok(Value::dimensionless(a * m4 - b))
}

/// mad(data...) — mean absolute deviation around the mean.
fn mad_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mean = compute_mean(args);
    let mad_val = args.iter().map(|v| (v.canonical - mean).abs()).sum::<f64>() / n;
    Ok(spread_value(mad_val, first_unit))
}

fn rms_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mean_sq = args.iter().map(|v| v.canonical * v.canonical).sum::<f64>() / n;
    Ok(location_value(mean_sq.sqrt(), first_unit))
}

/// zscore(x, mean, std) — `std` must be positive.
fn zscore_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = &args[0];
    let mean = &args[1];
    if !x.unit.is_compatible_with(&mean.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let std = positive_spread(&args[2], &x.unit)?;
    Ok(Value::dimensionless((x.canonical - mean.canonical) / std))
}

pub fn register_stats() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("sum", 1, usize::MAX, sum_fn),
        FunctionOp::scalar("mean", 1, usize::MAX, mean_fn),
        FunctionOp::scalar("geomean", 1, usize::MAX, geomean_fn),
        FunctionOp::scalar("harmean", 1, usize::MAX, harmean_fn),
        FunctionOp::scalar("min", 1, usize::MAX, min_fn),
        FunctionOp::scalar("max", 1, usize::MAX, max_fn),
        FunctionOp::scalar("range", 1, usize::MAX, range_fn),
        FunctionOp::scalar("median", 1, usize::MAX, median_fn),
        FunctionOp::scalar("mode", 1, usize::MAX, mode_fn),
        FunctionOp::scalar("var", 2, usize::MAX, var_fn),
        FunctionOp::scalar("var_s", 2, usize::MAX, var_fn),
        FunctionOp::scalar("var_p", 1, usize::MAX, var_p_fn),
        FunctionOp::scalar("variance", 2, usize::MAX, var_fn),
        FunctionOp::scalar("std", 2, usize::MAX, std_fn),
        FunctionOp::scalar("std_s", 2, usize::MAX, std_fn),
        FunctionOp::scalar("std_p", 1, usize::MAX, std_p_fn),
        FunctionOp::scalar("stdev", 2, usize::MAX, std_fn),
        FunctionOp::scalar("cov", 4, usize::MAX, cov_fn),
        FunctionOp::scalar("cov_s", 4, usize::MAX, cov_fn),
        FunctionOp::scalar("cov_p", 4, usize::MAX, cov_p_fn),
        FunctionOp::scalar("skew", 3, usize::MAX, skew_fn),
        FunctionOp::scalar("skewness", 3, usize::MAX, skew_fn),
        FunctionOp::scalar("kurt", 4, usize::MAX, kurt_fn),
        FunctionOp::scalar("kurtosis", 4, usize::MAX, kurt_fn),
        FunctionOp::scalar("mad", 1, usize::MAX, mad_fn),
        FunctionOp::scalar("rms", 1, usize::MAX, rms_fn),
        FunctionOp::scalar("zscore", 3, 3, zscore_fn),
        FunctionOp::scalar("standardize", 3, 3, zscore_fn),
        FunctionOp::scalar("quantile", 2, usize::MAX, quantile_fn),
        FunctionOp::scalar("percentile", 2, usize::MAX, percentile_fn),
        FunctionOp::scalar("iqr", 1, usize::MAX, iqr_fn),
        FunctionOp::scalar("corr", 4, usize::MAX, corr_fn),
        FunctionOp::scalar("correlation", 4, usize::MAX, corr_fn),
    ]
}
