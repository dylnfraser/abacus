use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless, math_helpers::non_negative_integer, operators::FunctionOp,
    },
};

/// Validates `(p, k)`: success probability p ∈ (0, 1] and trial number k ≥ 1 (an integer).
fn parse_geom_args(args: &[Value]) -> Result<(f64, f64), AbacusError> {
    check_dimensionless(args)?;
    let p = args[0].canonical;
    let k = non_negative_integer(args[1].canonical)? as f64;
    if !(p > 0.0 && p <= 1.0) || k < 1.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((p, k))
}

/// geompdf(p, k) = (1-p)^(k-1) · p — probability the first success is on trial k.
fn geompdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (p, k) = parse_geom_args(args)?;
    let pdf = if p == 1.0 {
        if k == 1.0 { 1.0 } else { 0.0 }
    } else {
        ((k - 1.0) * (-p).ln_1p()).exp() * p
    };
    Ok(Value::dimensionless(pdf))
}

/// geomcdf(p, k) = 1 - (1-p)^k
fn geomcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (p, k) = parse_geom_args(args)?;
    let cdf = if p == 1.0 {
        1.0
    } else {
        -(k * (-p).ln_1p()).exp_m1()
    };
    Ok(Value::dimensionless(cdf))
}

pub fn register_geometric() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("geompdf", 2, 2, geompdf_fn),
        FunctionOp::scalar("geomcdf", 2, 2, geomcdf_fn),
    ]
}
