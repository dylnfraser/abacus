use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless,
        distributions::special::{gamma_p, gamma_q, lgamma, norm_ppf, solve_increasing},
        operators::FunctionOp,
    },
};

fn non_convergent(what: &str) -> AbacusError {
    AbacusError::EvaluationError(format!(
        "{what} has no finite solution (result out of range) or failed to converge"
    ))
}

fn compute_chisqpdf(df: f64, x: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    if x == 0.0 {
        // The density at 0 is infinite for df < 2, 1/2 for df = 2, and 0 for df > 2.
        return match df.partial_cmp(&2.0) {
            Some(std::cmp::Ordering::Less) => f64::INFINITY,
            Some(std::cmp::Ordering::Equal) => 0.5,
            _ => 0.0,
        };
    }
    let log_pdf = (df / 2.0 - 1.0) * x.ln()
        - x / 2.0
        - (df / 2.0) * std::f64::consts::LN_2
        - lgamma(df / 2.0);
    log_pdf.exp()
}

/// Chi-squared CDF P(X ≤ x). `None` if the incomplete gamma fails to converge.
#[must_use]
pub fn compute_chisqcdf(df: f64, x: f64) -> Option<f64> {
    if x <= 0.0 {
        return Some(0.0);
    }
    gamma_p(df / 2.0, x / 2.0)
}

/// Chi-squared survival function P(X > x), accurate for small upper-tail probabilities.
#[must_use]
pub fn compute_chisq_sf(df: f64, x: f64) -> Option<f64> {
    if x <= 0.0 {
        return Some(1.0);
    }
    gamma_q(df / 2.0, x / 2.0)
}

/// chisqpdf(df, x)
fn chisqpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let df = args[0].canonical;
    let x = args[1].canonical;

    if df.is_nan() || df <= 0.0 || x.is_nan() || x < 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    Ok(Value::dimensionless(compute_chisqpdf(df, x)))
}

/// chisqcdf(df, x)
fn chisqcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let df = args[0].canonical;
    let x = args[1].canonical;

    if df.is_nan() || df <= 0.0 || x.is_nan() || x < 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    compute_chisqcdf(df, x)
        .map(Value::dimensionless)
        .ok_or_else(|| non_convergent("chisqcdf"))
}

/// invchisq(p, df)
fn invchisq_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(args)?;

    let p = args[0].canonical;
    let df = args[1].canonical;

    if !(p > 0.0 && p < 1.0) || df.is_nan() || df <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    // Wilson–Hilferty initial guess.
    let z = norm_ppf(p);
    let h = 2.0 / (9.0 * df);
    let term = 1.0 - h + z * h.sqrt();
    let guess = (df * term * term * term).max(1e-8 * df.max(1.0));

    // Work on whichever tail is smaller so tiny tail probabilities keep their precision.
    let root = if p <= 0.5 {
        solve_increasing(
            |x| compute_chisqcdf(df, x).map(|c| c - p),
            |x| compute_chisqpdf(df, x),
            guess,
            0.0,
            f64::INFINITY,
        )
    } else {
        let q = 1.0 - p;
        solve_increasing(
            |x| compute_chisq_sf(df, x).map(|s| q - s),
            |x| compute_chisqpdf(df, x),
            guess,
            0.0,
            f64::INFINITY,
        )
    };

    root.map(Value::dimensionless)
        .ok_or_else(|| non_convergent("invchisq"))
}

pub fn register_chi_square() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("chisqpdf", 2, 2, chisqpdf_fn),
        FunctionOp::scalar("chisqcdf", 2, 2, chisqcdf_fn),
        FunctionOp::scalar("invchisq", 2, 2, invchisq_fn),
    ]
}
