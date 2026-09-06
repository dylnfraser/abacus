use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};

/// unifpdf(a, b, x)
fn unifpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let a = args[0].canonical;
    let b = args[1].canonical;
    let x = args[2].canonical;

    if a >= b {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let pdf = if x >= a && x <= b { 1.0 / (b - a) } else { 0.0 };

    Ok(Value::dimensionless(pdf))
}

/// unifcdf(a, b, x)
fn unifcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let a = args[0].canonical;
    let b = args[1].canonical;
    let x = args[2].canonical;

    if a >= b {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let cdf = if x < a {
        0.0
    } else if x > b {
        1.0
    } else {
        (x - a) / (b - a)
    };

    Ok(Value::dimensionless(cdf))
}

/// invunif(p, a, b)
fn invunif_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let a = args[1].canonical;
    let b = args[2].canonical;

    if !(0.0..=1.0).contains(&p) || a >= b {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let x = a + p * (b - a);
    Ok(Value::dimensionless(x))
}

pub fn register_uniform() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("unifpdf", 3, 3, unifpdf_fn),
        FunctionOp::scalar("unifcdf", 3, 3, unifcdf_fn),
        FunctionOp::scalar("invunif", 3, 3, invunif_fn),
    ]
}
