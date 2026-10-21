use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::special::{factorial, lgamma, n_cr},
        math_helpers::integer_arg,
        operators::FunctionOp,
    },
};

fn overflow(what: &str) -> AbacusError {
    AbacusError::EvaluationError(format!("{what} result overflows"))
}

/// Number of r-permutations of n, n!/(n-r)!; `inf` on overflow.
fn n_pr(n: u64, r: u64) -> f64 {
    if r > n {
        return 0.0;
    }
    if r <= 200 {
        return ((n - r + 1)..=n).fold(1.0, |acc, x| acc * (x as f64));
    }
    let ln_p = lgamma(n as f64 + 1.0) - lgamma((n - r) as f64 + 1.0);
    if ln_p > f64::MAX.ln() {
        f64::INFINITY
    } else {
        ln_p.exp().round()
    }
}

/// Parses `(n, r)` as integers with 0 ≤ r ≤ n.
fn parse_n_r(args: &[Value]) -> Result<(u64, u64), AbacusError> {
    let n = integer_arg(&args[0])?;
    let r = integer_arg(&args[1])?;
    if r > n {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((n, r))
}

/// factorial(n) for integer 0 ≤ n ≤ 170 (171! overflows f64).
fn factorial_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let n = integer_arg(&args[0])?;
    if n > 170 {
        return Err(overflow("factorial"));
    }
    Ok(Value::dimensionless(factorial(n)))
}

/// nCr(n, r) or comb(n, r)
fn n_cr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (n, r) = parse_n_r(args)?;
    let c = n_cr(n, r);
    if c.is_infinite() {
        return Err(overflow("nCr"));
    }
    Ok(Value::dimensionless(c))
}

/// nPr(n, r) or perm(n, r)
fn n_pr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (n, r) = parse_n_r(args)?;
    let p = n_pr(n, r);
    if p.is_infinite() {
        return Err(overflow("nPr"));
    }
    Ok(Value::dimensionless(p))
}

pub fn register_combinatorics() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("factorial", 1, 1, factorial_fn),
        FunctionOp::scalar("nCr", 2, 2, n_cr_fn),
        FunctionOp::scalar("comb", 2, 2, n_cr_fn),
        FunctionOp::scalar("nPr", 2, 2, n_pr_fn),
        FunctionOp::scalar("perm", 2, 2, n_pr_fn),
    ]
}
