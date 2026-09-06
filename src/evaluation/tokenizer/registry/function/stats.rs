use crate::{
    AbacusError, Value, evaluation::tokenizer::registry::function::operators::FunctionOp,
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

fn sum_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let sum: f64 = args.iter().map(|v| v.canonical).sum();

    Ok(Value {
        canonical: sum,
        unit: Arc::clone(first_unit),
    })
}

fn mean_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let sum: f64 = args.iter().map(|v| v.canonical).sum();

    Ok(Value {
        canonical: sum / (args.len() as f64),
        unit: Arc::clone(first_unit),
    })
}

fn min_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let min_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::INFINITY, f64::min);

    Ok(Value {
        canonical: min_val,
        unit: Arc::clone(first_unit),
    })
}

fn max_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let max_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Value {
        canonical: max_val,
        unit: Arc::clone(first_unit),
    })
}

fn range_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let min_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::INFINITY, f64::min);
    let max_val = args
        .iter()
        .map(|v| v.canonical)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(Value {
        canonical: max_val - min_val,
        unit: Arc::clone(first_unit),
    })
}

fn median_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let mut values: Vec<f64> = args.iter().map(|v| v.canonical).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let len = values.len();
    let median_val = if len.is_multiple_of(2) {
        f64::midpoint(values[len / 2 - 1], values[len / 2])
    } else {
        values[len / 2]
    };

    Ok(Value {
        canonical: median_val,
        unit: Arc::clone(first_unit),
    })
}

fn mode_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if args.is_empty() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let mut max_count = 0;
    let mut mode_val = args[0].canonical;

    for (i, v1) in args.iter().enumerate() {
        let count = args
            .iter()
            .skip(i)
            .filter(|v2| (v1.canonical - v2.canonical).abs() < 1e-9)
            .count();
        if count > max_count {
            max_count = count;
            mode_val = v1.canonical;
        }
    }

    Ok(Value {
        canonical: mode_val,
        unit: Arc::clone(first_unit),
    })
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

/// Computes the (co)variance of a single dataset with delta-degrees-of-freedom `ddof`.
/// `ddof = 1` → sample variance, `ddof = 0` → population variance.
#[must_use]
pub fn compute_variance(args: &[Value], ddof: f64) -> f64 {
    let n = args.len() as f64;
    let mean = compute_mean(args);
    args.iter()
        .map(|v| (v.canonical - mean).powi(2))
        .sum::<f64>()
        / (n - ddof)
}

/// Computes Pearson correlation coefficient between two datasets.
pub fn compute_pearson_r(x_data: &[Value], y_data: &[Value]) -> Result<f64, AbacusError> {
    let n = x_data.len();
    let x_mean = compute_mean(x_data);
    let y_mean = compute_mean(y_data);

    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;

    for i in 0..n {
        let dx = x_data[i].canonical - x_mean;
        let dy = y_data[i].canonical - y_mean;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }

    let denom = (sxx * syy).sqrt();
    if denom == 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    Ok(sxy / denom)
}

/// Computes the cross-covariance sum and the xy-unit for a paired dataset.
fn compute_covariance(x_data: &[Value], y_data: &[Value], n: usize, ddof: f64) -> (f64, Arc<Unit>) {
    let x_unit = &x_data[0].unit;
    let y_unit = &y_data[0].unit;

    let x_mean = x_data.iter().map(|v| v.canonical).sum::<f64>() / (n as f64);
    let y_mean = y_data.iter().map(|v| v.canonical).sum::<f64>() / (n as f64);

    let cov_sum: f64 = (0..n)
        .map(|i| (x_data[i].canonical - x_mean) * (y_data[i].canonical - y_mean))
        .sum();

    let product_unit = Arc::new(Unit {
        scalar: x_unit.scalar * y_unit.scalar,
        offset: 0.0,
        dimensions: x_unit.dimensions + y_unit.dimensions,
        display: x_unit.display.multiply(&y_unit.display),
    });

    (cov_sum / (n as f64 - ddof), product_unit)
}

fn var_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if args.len() < 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let variance = compute_variance(args, 1.0);
    let squared_unit = Arc::new(Unit {
        scalar: first_unit.scalar * first_unit.scalar,
        offset: 0.0,
        dimensions: first_unit.dimensions * 2.0,
        display: first_unit.display.multiply(&first_unit.display),
    });
    Ok(Value {
        canonical: variance,
        unit: squared_unit,
    })
}

fn std_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if args.len() < 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value {
        canonical: compute_variance(args, 1.0).sqrt(),
        unit: Arc::clone(first_unit),
    })
}

fn var_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if args.is_empty() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let variance = compute_variance(args, 0.0);
    let squared_unit = Arc::new(Unit {
        scalar: first_unit.scalar * first_unit.scalar,
        offset: 0.0,
        dimensions: first_unit.dimensions * 2.0,
        display: first_unit.display.multiply(&first_unit.display),
    });
    Ok(Value {
        canonical: variance,
        unit: squared_unit,
    })
}

fn std_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    if args.is_empty() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value {
        canonical: compute_variance(args, 0.0).sqrt(),
        unit: Arc::clone(first_unit),
    })
}

/// Helper for linear interpolation quantile calculation
fn calc_quantile(data: &[Value], q: f64) -> f64 {
    let mut values: Vec<f64> = data.iter().map(|v| v.canonical).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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

/// quantile(data..., q) where q in [0, 1]
fn quantile_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() < 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let q_arg = &args[args.len() - 1];
    if !q_arg.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }

    let q = q_arg.canonical;
    if !(0.0..=1.0).contains(&q) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let data = &args[..args.len() - 1];
    let first_unit = check_compatible_units(data)?;
    let val = calc_quantile(data, q);

    Ok(Value {
        canonical: val,
        unit: Arc::clone(first_unit),
    })
}

/// percentile(data..., p) where p in [0, 100]
fn percentile_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() < 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let p_arg = &args[args.len() - 1];
    if !p_arg.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }

    let p = p_arg.canonical;
    if !(0.0..=100.0).contains(&p) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let data = &args[..args.len() - 1];
    let first_unit = check_compatible_units(data)?;
    let val = calc_quantile(data, p / 100.0);

    Ok(Value {
        canonical: val,
        unit: Arc::clone(first_unit),
    })
}

/// iqr(data...) -> Q3 - Q1
fn iqr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let q3 = calc_quantile(args, 0.75);
    let q1 = calc_quantile(args, 0.25);

    Ok(Value {
        canonical: q3 - q1,
        unit: Arc::clone(first_unit),
    })
}

/// `corr(x_range`, `y_range`) or corr(x1..xN, y1..yN)
fn corr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, _) = parse_paired_data(args)?;
    let r = compute_pearson_r(x_data, y_data)?;
    Ok(Value::dimensionless(r))
}

fn cov_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, n) = parse_paired_data(args)?;
    let (cov, unit) = compute_covariance(x_data, y_data, n, 1.0);
    Ok(Value {
        canonical: cov,
        unit,
    })
}

fn cov_p_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x_data, y_data, n) = parse_paired_data(args)?;
    check_compatible_units(x_data)?;
    check_compatible_units(y_data)?;
    let (cov, unit) = compute_covariance(x_data, y_data, n, 0.0);
    Ok(Value {
        canonical: cov,
        unit,
    })
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
    let gm = (log_sum / n).exp();

    Ok(Value {
        canonical: gm,
        unit: Arc::clone(first_unit),
    })
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
    let hm = n / inv_sum;

    Ok(Value {
        canonical: hm,
        unit: Arc::clone(first_unit),
    })
}

fn skew_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let _first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    if n < 3.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let mean = args.iter().map(|v| v.canonical).sum::<f64>() / n;
    let mut m2 = 0.0;
    let mut m3 = 0.0;

    for v in args {
        let diff = v.canonical - mean;
        m2 += diff * diff;
        m3 += diff * diff * diff;
    }

    let var = m2 / n;
    if var == 0.0 {
        return Ok(Value::dimensionless(0.0));
    }
    let skewness = (m3 / n) / var.powf(1.5);

    Ok(Value::dimensionless(skewness))
}

fn kurt_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let _first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    if n < 4.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let mean = args.iter().map(|v| v.canonical).sum::<f64>() / n;
    let mut m2 = 0.0;
    let mut m4 = 0.0;

    for v in args {
        let diff = v.canonical - mean;
        m2 += diff * diff;
        m4 += diff.powi(4);
    }

    let var = m2 / n;
    if var == 0.0 {
        return Ok(Value::dimensionless(0.0));
    }
    let excess_kurtosis = (m4 / n) / (var * var) - 3.0;

    Ok(Value::dimensionless(excess_kurtosis))
}

fn mad_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mean = args.iter().map(|v| v.canonical).sum::<f64>() / n;
    let mad_val = args.iter().map(|v| (v.canonical - mean).abs()).sum::<f64>() / n;

    Ok(Value {
        canonical: mad_val,
        unit: Arc::clone(first_unit),
    })
}

fn rms_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_unit = check_compatible_units(args)?;
    let n = args.len() as f64;
    let mean_sq = args.iter().map(|v| v.canonical * v.canonical).sum::<f64>() / n;
    let rms_val = mean_sq.sqrt();

    Ok(Value {
        canonical: rms_val,
        unit: Arc::clone(first_unit),
    })
}

fn zscore_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() != 3 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let x = &args[0];
    let mean = &args[1];
    let std = &args[2];

    if !x.unit.is_compatible_with(&mean.unit) || !x.unit.is_compatible_with(&std.unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }

    if std.canonical == 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let z = (x.canonical - mean.canonical) / std.canonical;
    Ok(Value::dimensionless(z))
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
