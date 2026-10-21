use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless, distributions::special::binom_pmf_raw,
        math_helpers::non_negative_integer, operators::FunctionOp,
    },
};

/// Validates `(N, K, n, k)`: population N, successes K ≤ N, draws n ≤ N, observed k.
fn parse_hypgeom_args(args: &[Value]) -> Result<(f64, f64, f64, f64), AbacusError> {
    check_dimensionless(args)?;
    let big_n = non_negative_integer(args[0].canonical)?;
    let big_k = non_negative_integer(args[1].canonical)?;
    let n = non_negative_integer(args[2].canonical)?;
    let k = non_negative_integer(args[3].canonical)?;
    if big_k > big_n || n > big_n || k > n || k > big_k {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((big_n as f64, big_k as f64, n as f64, k as f64))
}

/// P(X = x) for x successes in n draws from `nr` successes and `nb` failures, expressed as a
/// ratio of binomial saddle-point terms (as in R's `dhyper`) so it never overflows.
fn hyper_pmf(x: f64, nr: f64, nb: f64, n: f64) -> f64 {
    if x < 0.0 || x > n || x > nr || n - x > nb {
        return 0.0;
    }
    if n == 0.0 {
        return 1.0;
    }
    let total = nr + nb;
    let p = n / total;
    let q = (total - n) / total;
    let p1 = binom_pmf_raw(x, nr, p, q);
    let p2 = binom_pmf_raw(n - x, nb, p, q);
    let p3 = binom_pmf_raw(n, total, p, q);
    p1 * p2 / p3
}

/// Σ_{i ≤ x} P(X = i) / P(X = x), summed downwards until the terms become negligible.
fn hyper_tail_ratio(mut x: f64, nr: f64, nb: f64, n: f64) -> f64 {
    let mut sum = 0.0;
    let mut term = 1.0;
    while x > 0.0 && term >= f64::EPSILON * sum {
        term *= x * (nb - n + x) / (n + 1.0 - x) / (nr + 1.0 - x);
        sum += term;
        x -= 1.0;
    }
    1.0 + sum
}

/// P(X ≤ x), following R's `phyper`: always sums over the shorter tail.
fn hyper_cdf(x: f64, nr: f64, nb: f64, n: f64) -> f64 {
    let (mut x, mut nr, mut nb, mut lower) = (x, nr, nb, true);
    if x * (nr + nb) > n * nr {
        std::mem::swap(&mut nr, &mut nb);
        x = n - x - 1.0;
        lower = false;
    }
    if x < 0.0 || x < n - nb {
        return if lower { 0.0 } else { 1.0 };
    }
    if x >= nr || x >= n {
        return if lower { 1.0 } else { 0.0 };
    }
    let d = hyper_pmf(x, nr, nb, n);
    let pd = hyper_tail_ratio(x, nr, nb, n);
    if lower { d * pd } else { 1.0 - d * pd }
}

/// hypgeompdf(N, K, n, k)
fn hypgeompdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (big_n, big_k, n, k) = parse_hypgeom_args(args)?;
    Ok(Value::dimensionless(hyper_pmf(k, big_k, big_n - big_k, n)))
}

/// hypgeomcdf(N, K, n, k)
fn hypgeomcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (big_n, big_k, n, k) = parse_hypgeom_args(args)?;
    Ok(Value::dimensionless(hyper_cdf(k, big_k, big_n - big_k, n)))
}

pub fn register_hypergeometric() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("hypgeompdf", 4, 4, hypgeompdf_fn),
        FunctionOp::scalar("hypgeomcdf", 4, 4, hypgeomcdf_fn),
    ]
}
