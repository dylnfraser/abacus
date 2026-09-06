use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless, distributions::special::lgamma, operators::FunctionOp,
    },
};

fn poisson_pmf(lambda: f64, k: u64) -> f64 {
    if lambda == 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    let k_f = k as f64;
    let log_pmf = k_f * lambda.ln() - lambda - lgamma(k_f + 1.0);
    log_pmf.exp()
}

/// poissonpdf(lambda, k)
fn poissonpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let lambda = args[0].canonical;
    let k_val = args[1].canonical;

    if lambda < 0.0 || k_val < 0.0 || !lambda.is_finite() || !k_val.is_finite() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let k = k_val.round() as u64;
    let pdf = poisson_pmf(lambda, k);
    Ok(Value::dimensionless(pdf))
}

/// poissoncdf(lambda, k)
fn poissoncdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let lambda = args[0].canonical;
    let k_val = args[1].canonical;

    if lambda < 0.0 || k_val < 0.0 || !lambda.is_finite() || !k_val.is_finite() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let k = k_val.round() as u64;
    let mut cdf = 0.0;
    for i in 0..=k {
        cdf += poisson_pmf(lambda, i);
    }

    Ok(Value::dimensionless(cdf))
}

pub fn register_poisson() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("poissonpdf", 2, 2, poissonpdf_fn),
        FunctionOp::scalar("poissoncdf", 2, 2, poissoncdf_fn),
    ]
}
