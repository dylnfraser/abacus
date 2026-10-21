use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::special::{norm_cdf, norm_pdf, norm_ppf, norm_sf},
        math_helpers::positive_spread,
        operators::FunctionOp,
    },
};
use std::sync::Arc;

/// Standard normal CDF Φ(z).
#[must_use]
pub fn std_normal_cdf(z: f64) -> f64 {
    norm_cdf(z)
}

/// Two-sided p-value `2·(1 − Φ(|z|))`, computed from the survival function so that very
/// small p-values keep their relative precision.
#[must_use]
pub fn two_sided_normal_p(z: f64) -> f64 {
    (2.0 * norm_sf(z.abs())).min(1.0)
}

/// Standard normal quantile Φ⁻¹(p).
#[must_use]
pub fn std_normal_quantile(p: f64) -> f64 {
    norm_ppf(p)
}

/// Parses `(x)` or `(x, mean, std)`. `mean` and `x` are readings; `std` is a spread, so for
/// affine units (°C/°F) it is interpreted as a temperature difference.
fn parse_norm_args(args: &[Value]) -> Result<(f64, f64, f64), AbacusError> {
    if args.len() == 1 {
        if !args[0].unit.is_dimensionless() {
            return Err(AbacusError::IncompatibleDimensions);
        }
        Ok((args[0].canonical, 0.0, 1.0))
    } else if args.len() == 3 {
        let first_unit = &args[0].unit;
        if !args[1].unit.is_compatible_with(first_unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let std = positive_spread(&args[2], first_unit)?;
        Ok((args[0].canonical, args[1].canonical, std))
    } else {
        Err(AbacusError::IncompatibleFunctionArguments)
    }
}

/// normpdf(x) or normpdf(x, mean, std)
fn normpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x, mean, std) = parse_norm_args(args)?;
    let z = (x - mean) / std;
    Ok(Value::dimensionless(norm_pdf(z) / std))
}

/// normcdf(x) or normcdf(x, mean, std)
fn normcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x, mean, std) = parse_norm_args(args)?;
    Ok(Value::dimensionless(norm_cdf((x - mean) / std)))
}

fn probability_arg(value: &Value) -> Result<f64, AbacusError> {
    if !value.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let p = value.canonical;
    if !(p > 0.0 && p < 1.0) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(p)
}

/// invnorm(p) or invnorm(p, mean, std)
fn invnorm_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let p = probability_arg(&args[0])?;
    let z = norm_ppf(p);
    match args.len() {
        1 => Ok(Value::dimensionless(z)),
        3 => {
            let mean = &args[1];
            let std = positive_spread(&args[2], &mean.unit)?;
            Ok(Value::from_canonical(
                mean.canonical + std * z,
                Arc::clone(&mean.unit),
            ))
        }
        _ => Err(AbacusError::IncompatibleFunctionArguments),
    }
}

pub fn register_normal() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("normpdf", 1, 3, normpdf_fn),
        FunctionOp::scalar("normcdf", 1, 3, normcdf_fn),
        FunctionOp::scalar("invnorm", 1, 3, invnorm_fn),
    ]
}
