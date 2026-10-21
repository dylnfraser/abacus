use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless,
        distributions::special::{beta_inc_xy, lgamma, norm_ppf, solve_increasing},
        operators::FunctionOp,
    },
};

fn non_convergent(what: &str) -> AbacusError {
    AbacusError::EvaluationError(format!(
        "{what} has no finite solution (result out of range) or failed to converge"
    ))
}

fn compute_tpdf(df: f64, t: f64) -> f64 {
    let num_log = lgamma(f64::midpoint(df, 1.0));
    let den_log = (df * std::f64::consts::PI).ln() * 0.5 + lgamma(df / 2.0);
    let power = -f64::midpoint(df, 1.0) * (t * t / df).ln_1p();
    (num_log - den_log + power).exp()
}

/// Lower-tail probability P(T ≤ -|t|) for Student's t, i.e. one tail.
fn t_tail(df: f64, t: f64) -> Option<f64> {
    let denom = df + t * t;
    beta_inc_xy(df / 2.0, 0.5, df / denom, t * t / denom).map(|ib| 0.5 * ib)
}

/// Student's t CDF. Returns `None` if the incomplete beta fails to converge.
#[must_use]
pub fn compute_tcdf(df: f64, t: f64) -> Option<f64> {
    if t.is_infinite() {
        return Some(if t > 0.0 { 1.0 } else { 0.0 });
    }
    let tail = t_tail(df, t)?;
    Some(if t >= 0.0 { 1.0 - tail } else { tail })
}

/// Two-sided p-value `2·P(T ≥ |t|)`, computed directly from the tail.
#[must_use]
pub fn two_sided_t_p(df: f64, t: f64) -> Option<f64> {
    if t.is_infinite() {
        return Some(0.0);
    }
    t_tail(df, t).map(|tail| (2.0 * tail).min(1.0))
}

/// Inverse Student's t CDF, solved by safeguarded Newton iteration on the tail probability.
#[must_use]
pub fn compute_invt(p: f64, df: f64) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) || df.is_nan() || df <= 0.0 {
        return None;
    }
    if p == 0.5 {
        return Some(0.0);
    }
    // Solve in the lower tail for q = min(p, 1 - p) (1 - p is exact for p ≥ 0.5), then mirror.
    let (q, sign) = if p < 0.5 { (p, 1.0) } else { (1.0 - p, -1.0) };
    let guess = norm_ppf(q).min(-1e-3);
    let root = solve_increasing(
        |t| t_tail(df, t).map(|tail| if t <= 0.0 { tail - q } else { 1.0 - tail - q }),
        |t| compute_tpdf(df, t),
        guess,
        f64::NEG_INFINITY,
        0.0,
    )?;
    Some(sign * root)
}

/// tpdf(df, t)
fn tpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let df = args[0].canonical;
    let t = args[1].canonical;

    if df.is_nan() || df <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    Ok(Value::dimensionless(compute_tpdf(df, t)))
}

/// tcdf(df, t)
fn tcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let df = args[0].canonical;
    let t = args[1].canonical;

    if df.is_nan() || df <= 0.0 || t.is_nan() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    compute_tcdf(df, t)
        .map(Value::dimensionless)
        .ok_or_else(|| non_convergent("tcdf"))
}

/// invt(p, df)
fn invt_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let df = args[1].canonical;

    if !(p > 0.0 && p < 1.0) || df.is_nan() || df <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    compute_invt(p, df)
        .map(Value::dimensionless)
        .ok_or_else(|| non_convergent("invt"))
}

pub fn register_student_t() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("tpdf", 2, 2, tpdf_fn),
        FunctionOp::scalar("tcdf", 2, 2, tcdf_fn),
        FunctionOp::scalar("invt", 2, 2, invt_fn),
    ]
}
