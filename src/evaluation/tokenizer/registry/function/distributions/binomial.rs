use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless,
        distributions::special::{beta_inc_xy, binom_pmf_raw},
        math_helpers::non_negative_integer,
        operators::FunctionOp,
    },
};

/// Validates `(n, p, k)`: integers n ≥ k ≥ 0 and probability p ∈ [0, 1].
fn parse_binom_args(args: &[Value]) -> Result<(f64, f64, f64), AbacusError> {
    check_dimensionless(args)?;
    let n = non_negative_integer(args[0].canonical)? as f64;
    let p = args[1].canonical;
    let k = non_negative_integer(args[2].canonical)? as f64;
    if !(0.0..=1.0).contains(&p) || k > n {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((n, p, k))
}

/// binompdf(n, p, k) — computed with Loader's saddle-point method, so it stays accurate for
/// very large n instead of overflowing.
fn binompdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (n, p, k) = parse_binom_args(args)?;
    Ok(Value::dimensionless(binom_pmf_raw(k, n, p, 1.0 - p)))
}

/// binomcdf(n, p, k) = P(X ≤ k) = I_{1-p}(n - k, k + 1), evaluated in O(√n) via the
/// regularized incomplete beta function rather than by summing k + 1 terms.
fn binomcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (n, p, k) = parse_binom_args(args)?;
    if k >= n {
        return Ok(Value::dimensionless(1.0));
    }
    beta_inc_xy(n - k, k + 1.0, 1.0 - p, p)
        .map(Value::dimensionless)
        .ok_or_else(|| AbacusError::EvaluationError("binomcdf failed to converge".to_string()))
}

pub fn register_binomial() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("binompdf", 3, 3, binompdf_fn),
        FunctionOp::scalar("binomcdf", 3, 3, binomcdf_fn),
    ]
}
