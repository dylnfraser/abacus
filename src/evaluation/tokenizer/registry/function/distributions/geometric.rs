use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};

/// geompdf(p, k)
fn geompdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let k_val = args[1].canonical;

    if p <= 0.0 || p > 1.0 || k_val < 1.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let k = k_val.round() as i32;
    let pdf = (1.0 - p).powi(k - 1) * p;
    Ok(Value::dimensionless(pdf))
}

/// geomcdf(p, k)
fn geomcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let k_val = args[1].canonical;

    if p <= 0.0 || p > 1.0 || k_val < 1.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let k = k_val.round() as i32;
    let cdf = 1.0 - (1.0 - p).powi(k);
    Ok(Value::dimensionless(cdf))
}

pub fn register_geometric() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("geompdf", 2, 2, geompdf_fn),
        FunctionOp::scalar("geomcdf", 2, 2, geomcdf_fn),
    ]
}
