use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};
use std::sync::Arc;

/// ln(x) — Natural logarithm
fn ln_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;
    let x = args[0].canonical;
    if x <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value::dimensionless(x.ln()))
}

/// log10(x) — Base-10 logarithm
fn log10_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;
    let x = args[0].canonical;
    if x <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value::dimensionless(x.log10()))
}

/// log2(x) — Base-2 logarithm
fn log2_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;
    let x = args[0].canonical;
    if x <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(Value::dimensionless(x.log2()))
}

/// log(x) [base 10] or log(x, base)
fn log_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;
    let x = args[0].canonical;
    if x <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    if args.len() == 1 {
        Ok(Value::dimensionless(x.log10()))
    } else {
        let base = args[1].canonical;
        if base <= 0.0 || base == 1.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        Ok(Value::dimensionless(x.log(base)))
    }
}

/// exp(x) — Exponential e^x
fn exp_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;
    let x = args[0].canonical;
    Ok(Value::dimensionless(x.exp()))
}

/// abs(x) — Absolute value (preserves unit)
fn abs_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let val = &args[0];
    if val.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("abs"));
    }
    let canonical = val.canonical.abs();
    Ok(Value {
        canonical,
        unit: Arc::clone(&val.unit),
    })
}

/// floor(x) — Floor function (preserves unit)
fn floor_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let val = &args[0];
    let unit_amount = val.amount();
    let floored_amount = unit_amount.floor();
    Ok(Value::new(floored_amount, Arc::clone(&val.unit)))
}

/// ceil(x) — Ceiling function (preserves unit)
fn ceil_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let val = &args[0];
    let unit_amount = val.amount();
    let ceiled_amount = unit_amount.ceil();
    Ok(Value::new(ceiled_amount, Arc::clone(&val.unit)))
}

/// round(x) — Rounding function (preserves unit)
fn round_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let val = &args[0];
    let unit_amount = val.amount();
    let rounded_amount = unit_amount.round();
    Ok(Value::new(rounded_amount, Arc::clone(&val.unit)))
}

/// sign(x) — Signum (1.0, 0.0, -1.0)
fn sign_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let val = &args[0];
    if val.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("sign"));
    }
    let x = val.amount();
    Ok(Value::dimensionless(x.signum()))
}

pub fn register_math() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("ln", 1, 1, ln_fn),
        FunctionOp::scalar("log10", 1, 1, log10_fn),
        FunctionOp::scalar("log2", 1, 1, log2_fn),
        FunctionOp::scalar("log", 1, 2, log_fn),
        FunctionOp::scalar("exp", 1, 1, exp_fn),
        FunctionOp::scalar("abs", 1, 1, abs_fn),
        FunctionOp::scalar("floor", 1, 1, floor_fn),
        FunctionOp::scalar("ceil", 1, 1, ceil_fn),
        FunctionOp::scalar("round", 1, 1, round_fn),
        FunctionOp::scalar("sign", 1, 1, sign_fn),
    ]
}
