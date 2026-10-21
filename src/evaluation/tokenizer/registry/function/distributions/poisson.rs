use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless,
        distributions::special::{gamma_q, poisson_pmf_raw},
        math_helpers::non_negative_integer,
        operators::FunctionOp,
    },
};

/// Validates `(lambda, k)`: finite rate λ ≥ 0 and integer k ≥ 0.
fn parse_poisson_args(args: &[Value]) -> Result<(f64, f64), AbacusError> {
    check_dimensionless(args)?;
    let lambda = args[0].canonical;
    if !(lambda >= 0.0 && lambda.is_finite()) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let k = non_negative_integer(args[1].canonical)? as f64;
    Ok((lambda, k))
}

/// poissonpdf(lambda, k)
fn poissonpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (lambda, k) = parse_poisson_args(args)?;
    Ok(Value::dimensionless(poisson_pmf_raw(k, lambda)))
}

/// poissoncdf(lambda, k) = P(X ≤ k) = Q(k + 1, λ), evaluated via the regularized upper
/// incomplete gamma function rather than by summing k + 1 terms.
fn poissoncdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (lambda, k) = parse_poisson_args(args)?;
    if lambda == 0.0 {
        return Ok(Value::dimensionless(1.0));
    }
    gamma_q(k + 1.0, lambda)
        .map(Value::dimensionless)
        .ok_or_else(|| AbacusError::EvaluationError("poissoncdf failed to converge".to_string()))
}

pub fn register_poisson() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("poissonpdf", 2, 2, poissonpdf_fn),
        FunctionOp::scalar("poissoncdf", 2, 2, poissoncdf_fn),
    ]
}
