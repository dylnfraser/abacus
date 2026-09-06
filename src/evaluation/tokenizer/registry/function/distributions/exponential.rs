use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};

/// exppdf(lambda, x)
fn exppdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let lambda = args[0].canonical;
    let x = args[1].canonical;

    if lambda <= 0.0 || x < 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let pdf = lambda * (-lambda * x).exp();
    Ok(Value::dimensionless(pdf))
}

/// expcdf(lambda, x)
fn expcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let lambda = args[0].canonical;
    let x = args[1].canonical;

    if lambda <= 0.0 || x < 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let cdf = 1.0 - (-lambda * x).exp();
    Ok(Value::dimensionless(cdf))
}

/// invexp(p, lambda)
fn invexp_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let lambda = args[1].canonical;

    if p <= 0.0 || p >= 1.0 || lambda <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let x = -(1.0 - p).ln() / lambda;
    Ok(Value::dimensionless(x))
}

pub fn register_exponential() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("exppdf", 2, 2, exppdf_fn),
        FunctionOp::scalar("expcdf", 2, 2, expcdf_fn),
        FunctionOp::scalar("invexp", 2, 2, invexp_fn),
    ]
}
