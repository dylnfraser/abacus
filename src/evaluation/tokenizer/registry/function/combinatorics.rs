use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        distributions::special::factorial, math_helpers::integer_arg, operators::FunctionOp,
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
    // n!/(n-r)! >= r!, and 171! already exceeds f64::MAX, so r > 170 always overflows.
    // Otherwise the exact product is used (it reaches `inf` on its own if it overflows).
    if r > 170 {
        return f64::INFINITY;
    }
    ((n - r + 1)..=n).fold(1.0, |acc, x| acc * (x as f64))
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

/// Binomial coefficient for the user-facing `nCr`: exact (u128) while it fits, otherwise a
/// product of `k` ratios in double-double arithmetic so the result keeps full precision
/// (the log-gamma route in `n_cr` is only good to ~1e-12 for large results). Since
/// `C(n, k) ≥ 2^k` for `k ≤ n/2`, any `k > 1100` overflows, which bounds the loop.
fn exact_n_cr(n: u64, k: u64) -> f64 {
    if k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut exact: u128 = 1;
    for i in 0..k {
        // C(n, i) · (n − i) / (i + 1) = C(n, i + 1), always an integer.
        match exact.checked_mul(u128::from(n - i)) {
            Some(p) => exact = p / u128::from(i + 1),
            None => return n_cr_double_double(n, k),
        }
    }
    exact as f64
}

fn n_cr_double_double(n: u64, k: u64) -> f64 {
    if k > 1100 {
        return f64::INFINITY;
    }
    // (hi, lo) represents hi + lo. Each step divides by (i + 1) and then multiplies by (n − i)
    // using error-free transformations (dividing first so the last step can't overflow early),
    // which keeps the accumulated rounding error near one ulp.
    let (mut hi, mut lo) = (1.0_f64, 0.0_f64);
    for i in 0..k {
        let d = (i + 1) as f64;
        let q = hi / d;
        let q_lo = ((-q).mul_add(d, hi) + lo) / d;
        let m = (n - i) as f64;
        let p = q * m;
        let p_lo = q.mul_add(m, -p) + q_lo * m;
        hi = p + p_lo;
        lo = p_lo - (hi - p);
        if !hi.is_finite() {
            return f64::INFINITY;
        }
    }
    hi + lo
}

/// nCr(n, r) or comb(n, r)
fn n_cr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (n, r) = parse_n_r(args)?;
    let c = exact_n_cr(n, r);
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
