use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{
        math_helpers::delta_unit,
        operators::FunctionOp,
        stats::{compute_mean, parse_paired_data},
    },
    units::{eval_result::EvalResult, hash::Hash, unit::Unit, value::Value as AbacusValue},
};
use std::sync::Arc;

/// Parse `x_data` and `y_data` arrays from arguments by delegating to `parse_paired_data`.
fn parse_regression_inputs(args: &[Value]) -> Result<(&[Value], &[Value]), AbacusError> {
    let (x, y, _) = parse_paired_data(args)?;
    Ok((x, y))
}

/// Computes basic regression statistics (`mean_x`, `mean_y`, Sxx, Syy, Sxy, n)
struct RegStats {
    n: f64,
    mean_x: f64,
    mean_y: f64,
    sxx: f64,
    syy: f64,
    sxy: f64,
    x_unit: Arc<Unit>,
    y_unit: Arc<Unit>,
}

impl RegStats {
    fn compute(x_data: &[Value], y_data: &[Value]) -> Result<Self, AbacusError> {
        let n = x_data.len() as f64;
        if n < 2.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }

        let mean_x = compute_mean(x_data);
        let mean_y = compute_mean(y_data);

        let mut sxx = 0.0;
        let mut syy = 0.0;
        let mut sxy = 0.0;

        for (x, y) in x_data.iter().zip(y_data.iter()) {
            let dx = x.canonical - mean_x;
            let dy = y.canonical - mean_y;
            sxx += dx * dx;
            syy += dy * dy;
            sxy += dx * dy;
        }

        if sxx <= 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }

        Ok(Self {
            n,
            mean_x,
            mean_y,
            sxx,
            syy,
            sxy,
            x_unit: Arc::clone(&x_data[0].unit),
            y_unit: Arc::clone(&y_data[0].unit),
        })
    }

    fn slope(&self) -> f64 {
        self.sxy / self.sxx
    }

    fn intercept(&self) -> f64 {
        self.mean_y - self.slope() * self.mean_x
    }

    /// Pearson r; `None` when y is constant (r is undefined, matching `corr`).
    fn r(&self) -> Option<f64> {
        if self.syy <= 0.0 {
            None
        } else {
            Some(self.sxy / (self.sxx * self.syy).sqrt())
        }
    }

    fn r_or_err(&self) -> Result<f64, AbacusError> {
        // Correlation is undefined when y is constant (consistent with `corr`).
        self.r().ok_or(AbacusError::IncompatibleFunctionArguments)
    }

    /// Residual standard error s = √(SSE / (n − 2)); `None` for n ≤ 2.
    fn residual_se(&self) -> Option<f64> {
        (self.n > 2.0).then(|| {
            ((self.syy - self.slope() * self.sxy) / (self.n - 2.0))
                .max(0.0)
                .sqrt()
        })
    }

    fn slope_unit(&self) -> Arc<Unit> {
        Arc::new(Unit {
            scalar: self.y_unit.scalar / self.x_unit.scalar,
            offset: 0.0,
            dimensions: self.y_unit.dimensions - self.x_unit.dimensions,
            display: self.y_unit.display.divide(&self.x_unit.display),
        })
    }
}

// ── linreg(x_data..., y_data...) -> EvalResult::Hash ──
//
// Keys: `slope`, `intercept`, `r`, `r2`, `se` (residual standard error s = √(SSE/(n−2))),
// `slope_se` (standard error of the slope, s/√Sxx), `intercept_se`
// (s·√(1/n + x̄²/Sxx)), `mean_x`, `mean_y`. `se`/`slope_se`/`intercept_se` need n > 2;
// `r`/`r2` are omitted when y is constant (correlation is undefined).
fn linreg_hash_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    let (x_data, y_data) = parse_regression_inputs(args)?;
    let stats = RegStats::compute(x_data, y_data)?;

    let mut hash = Hash::new();
    hash.insert(
        "slope",
        AbacusValue::from_canonical(stats.slope(), stats.slope_unit()),
    );
    hash.insert(
        "intercept",
        AbacusValue::from_canonical(stats.intercept(), Arc::clone(&stats.y_unit)),
    );

    if let Some(r) = stats.r() {
        hash.insert("r2", Value::dimensionless(r * r));
        hash.insert("r", Value::dimensionless(r));
    }

    if let Some(s) = stats.residual_se() {
        let y_delta = delta_unit(&stats.y_unit);
        hash.insert("se", AbacusValue::from_canonical(s, Arc::clone(&y_delta)));
        hash.insert(
            "slope_se",
            AbacusValue::from_canonical(s / stats.sxx.sqrt(), stats.slope_unit()),
        );
        let intercept_se = s * (1.0 / stats.n + stats.mean_x * stats.mean_x / stats.sxx).sqrt();
        hash.insert(
            "intercept_se",
            AbacusValue::from_canonical(intercept_se, y_delta),
        );
    }

    hash.insert(
        "mean_x",
        AbacusValue::from_canonical(stats.mean_x, Arc::clone(&stats.x_unit)),
    );
    hash.insert(
        "mean_y",
        AbacusValue::from_canonical(stats.mean_y, Arc::clone(&stats.y_unit)),
    );

    Ok(EvalResult::Hash(hash))
}

fn compute_stats(args: &[Value]) -> Result<RegStats, AbacusError> {
    let (x_data, y_data) = parse_regression_inputs(args)?;
    RegStats::compute(x_data, y_data)
}

// ── linreg_slope(x_data..., y_data...) ──
fn linreg_slope_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    let stats = compute_stats(args)?;
    Ok(AbacusValue::from_canonical(
        stats.slope(),
        stats.slope_unit(),
    ))
}

// ── linreg_intercept(x_data..., y_data...) ──
fn linreg_intercept_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    let stats = compute_stats(args)?;
    Ok(AbacusValue::from_canonical(stats.intercept(), stats.y_unit))
}

// ── linreg_r2(x_data..., y_data...) ──
fn linreg_r2_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    let r = compute_stats(args)?.r_or_err()?;
    Ok(Value::dimensionless(r * r))
}

// ── linreg_r(x_data..., y_data...) ──
fn linreg_r_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    Ok(Value::dimensionless(compute_stats(args)?.r_or_err()?))
}

// ── linreg_se(x_data..., y_data...) — residual standard error ──
fn linreg_se_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    let stats = compute_stats(args)?;
    let s = stats
        .residual_se()
        .ok_or(AbacusError::IncompatibleFunctionArguments)?;
    Ok(AbacusValue::from_canonical(s, delta_unit(&stats.y_unit)))
}

fn predict_with(x_target: &Value, reg_args: &[Value]) -> Result<AbacusValue, AbacusError> {
    let (x_data, y_data) = parse_regression_inputs(reg_args)?;
    if !x_target.unit.is_compatible_with(&x_data[0].unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let stats = RegStats::compute(x_data, y_data)?;
    let y_pred = stats.slope() * x_target.canonical + stats.intercept();
    Ok(AbacusValue::from_canonical(y_pred, stats.y_unit))
}

fn check_predict_arity(args: &[Value]) -> Result<(), AbacusError> {
    if args.len() < 5 || !(args.len() - 1).is_multiple_of(2) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(())
}

// ── predict(x_target, x_data..., y_data...) — target first ──
fn predict_target_first_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    check_predict_arity(args)?;
    predict_with(&args[0], &args[1..])
}

// ── linreg_predict(x_data..., y_data..., x_target) — target last ──
fn predict_target_last_fn(args: &[Value]) -> Result<AbacusValue, AbacusError> {
    check_predict_arity(args)?;
    let (x_target, reg_args) = args.split_last().ok_or(AbacusError::UnexpectedEnd)?;
    predict_with(x_target, reg_args)
}

pub fn register_regression() -> Vec<FunctionOp> {
    let mut ops = Vec::new();

    // linreg / LinReg -> Returns EvalResult::Hash (see `linreg_hash_fn` for keys)
    for name in &["linreg", "LinReg"] {
        ops.push(FunctionOp::eval_result(name, 4, usize::MAX, linreg_hash_fn));
    }

    for name in &["linreg_slope", "LinRegSlope"] {
        ops.push(FunctionOp::scalar(name, 4, usize::MAX, linreg_slope_fn));
    }

    for name in &["linreg_intercept", "LinRegIntercept"] {
        ops.push(FunctionOp::scalar(name, 4, usize::MAX, linreg_intercept_fn));
    }

    for name in &["linreg_r2", "LinRegR2", "r2", "R2"] {
        ops.push(FunctionOp::scalar(name, 4, usize::MAX, linreg_r2_fn));
    }

    for name in &["linreg_r", "LinRegR"] {
        ops.push(FunctionOp::scalar(name, 4, usize::MAX, linreg_r_fn));
    }

    for name in &["linreg_se", "LinRegSE"] {
        ops.push(FunctionOp::scalar(name, 4, usize::MAX, linreg_se_fn));
    }

    // Each prediction name has one fixed argument order (no guessing from units).
    ops.push(FunctionOp::scalar(
        "predict",
        5,
        usize::MAX,
        predict_target_first_fn,
    ));
    for name in &["linreg_predict", "LinRegPredict"] {
        ops.push(FunctionOp::scalar(
            name,
            5,
            usize::MAX,
            predict_target_last_fn,
        ));
    }

    ops
}
