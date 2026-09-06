use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::special::erfinv, operators::FunctionOp,
    },
};
use std::f64::consts::TAU;

#[must_use]
pub fn std_normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

fn parse_norm_args(args: &[Value]) -> Result<(f64, f64, f64), AbacusError> {
    if args.len() == 1 {
        if !args[0].unit.is_dimensionless() {
            return Err(AbacusError::IncompatibleDimensions);
        }
        Ok((args[0].canonical, 0.0, 1.0))
    } else if args.len() == 3 {
        let first_unit = &args[0].unit;
        if !args[1].unit.is_compatible_with(first_unit)
            || !args[2].unit.is_compatible_with(first_unit)
        {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let x = args[0].canonical;
        let mean = args[1].canonical;
        let std = args[2].canonical;
        if std <= 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        Ok((x, mean, std))
    } else {
        Err(AbacusError::IncompatibleFunctionArguments)
    }
}

/// normpdf(x) or normpdf(x, mean, std)
fn normpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x, mean, std) = parse_norm_args(args)?;
    let z = (x - mean) / std;
    let pdf = (1.0 / (std * TAU.sqrt())) * (-0.5 * z * z).exp();
    Ok(Value::dimensionless(pdf))
}

/// normcdf(x) or normcdf(x, mean, std)
fn normcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (x, mean, std) = parse_norm_args(args)?;
    let z = (x - mean) / std;
    let cdf = 0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2));
    Ok(Value::dimensionless(cdf))
}

/// invnorm(p) or invnorm(p, mean, std)
fn invnorm_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if args.len() == 1 {
        if !args[0].unit.is_dimensionless() {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let p = args[0].canonical;
        if p <= 0.0 || p >= 1.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        let z = std::f64::consts::SQRT_2 * erfinv(2.0 * p - 1.0);
        Ok(Value::dimensionless(z))
    } else if args.len() == 3 {
        let p_arg = &args[0];
        if !p_arg.unit.is_dimensionless() {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let p = p_arg.canonical;
        if p <= 0.0 || p >= 1.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }

        let mean_unit = &args[1].unit;
        if !args[2].unit.is_compatible_with(mean_unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }

        let mean = args[1].canonical;
        let std = args[2].canonical;
        if std <= 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }

        let z = std::f64::consts::SQRT_2 * erfinv(2.0 * p - 1.0);
        let x = mean + std * z;

        Ok(Value {
            canonical: x,
            unit: std::sync::Arc::clone(mean_unit),
        })
    } else {
        Err(AbacusError::IncompatibleFunctionArguments)
    }
}

pub fn register_normal() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("normpdf", 1, 3, normpdf_fn),
        FunctionOp::scalar("normcdf", 1, 3, normcdf_fn),
        FunctionOp::scalar("invnorm", 1, 3, invnorm_fn),
    ]
}
