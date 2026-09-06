use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless, distributions::special::n_cr, operators::FunctionOp,
    },
};

/// hypgeompdf(N, K, n, k)
fn hypgeompdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    for arg in args {
        let v = arg.canonical;
        if v < 0.0 || !v.is_finite() {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
    }

    let big_n = args[0].canonical.round() as u64;
    let big_k = args[1].canonical.round() as u64;
    let n = args[2].canonical.round() as u64;
    let k = args[3].canonical.round() as u64;

    if big_k > big_n || n > big_n || k > n || k > big_k {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let pdf = (n_cr(big_k, k) * n_cr(big_n - big_k, n - k)) / n_cr(big_n, n);
    Ok(Value::dimensionless(pdf))
}

/// hypgeomcdf(N, K, n, k)
fn hypgeomcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    for arg in args {
        let v = arg.canonical;
        if v < 0.0 || !v.is_finite() {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
    }

    let big_n = args[0].canonical.round() as u64;
    let big_k = args[1].canonical.round() as u64;
    let n = args[2].canonical.round() as u64;
    let k = args[3].canonical.round() as u64;

    if big_k > big_n || n > big_n || k > n || k > big_k {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let mut cdf = 0.0;
    for i in 0..=k {
        if i <= big_k && (n - i) <= (big_n - big_k) {
            cdf += (n_cr(big_k, i) * n_cr(big_n - big_k, n - i)) / n_cr(big_n, n);
        }
    }

    Ok(Value::dimensionless(cdf))
}

pub fn register_hypergeometric() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("hypgeompdf", 4, 4, hypgeompdf_fn),
        FunctionOp::scalar("hypgeomcdf", 4, 4, hypgeomcdf_fn),
    ]
}
