use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        check_dimensionless,
        distributions::special::{beta_inc_xy, ln_beta},
        operators::FunctionOp,
    },
};

fn parse_f_args(args: &[Value]) -> Result<(f64, f64, f64), AbacusError> {
    check_dimensionless(args)?;
    let df1 = args[0].canonical;
    let df2 = args[1].canonical;
    let x = args[2].canonical;
    if df1.is_nan() || df1 <= 0.0 || df2.is_nan() || df2 <= 0.0 || x.is_nan() || x < 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok((df1, df2, x))
}

/// fpdf(df1, df2, x)
fn fpdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (df1, df2, x) = parse_f_args(args)?;

    if x == 0.0 {
        // The density at 0 is infinite for df1 < 2, 1 for df1 = 2, and 0 for df1 > 2.
        let pdf = match df1.partial_cmp(&2.0) {
            Some(std::cmp::Ordering::Less) => f64::INFINITY,
            Some(std::cmp::Ordering::Equal) => 1.0,
            _ => 0.0,
        };
        return Ok(Value::dimensionless(pdf));
    }

    // f(x) = (d1/d2)^{d1/2} x^{d1/2 - 1} (1 + d1 x / d2)^{-(d1+d2)/2} / B(d1/2, d2/2)
    let half1 = df1 / 2.0;
    let log_pdf = half1 * (df1 / df2).ln() + (half1 - 1.0) * x.ln()
        - f64::midpoint(df1, df2) * (df1 * x / df2).ln_1p()
        - ln_beta(half1, df2 / 2.0);
    Ok(Value::dimensionless(log_pdf.exp()))
}

/// fcdf(df1, df2, x)
fn fcdf_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let (df1, df2, x) = parse_f_args(args)?;
    if x.is_infinite() {
        return Ok(Value::dimensionless(1.0));
    }
    let denom = df1 * x + df2;
    beta_inc_xy(df1 / 2.0, df2 / 2.0, df1 * x / denom, df2 / denom)
        .map(Value::dimensionless)
        .ok_or_else(|| AbacusError::EvaluationError("fcdf failed to converge".to_string()))
}

pub fn register_f_dist() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("fpdf", 3, 3, fpdf_fn),
        FunctionOp::scalar("fcdf", 3, 3, fcdf_fn),
    ]
}
