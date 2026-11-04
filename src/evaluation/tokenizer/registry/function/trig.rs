use crate::{
    AbacusError, AngleMode, EvalResult, Unit, Value,
    evaluation::tokenizer::registry::function::operators::{FunctionContext, FunctionOp},
    units::{dimensions::Dimensions, unit::UnitExpr},
};
use std::sync::Arc;

fn parse_angle_radians(val: &Value) -> Result<f64, AbacusError> {
    if val.unit.is_dimensionless() {
        Ok(val.canonical)
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

fn make_angle_rad(radians: f64) -> Value {
    Value {
        canonical: radians,
        unit: Arc::new(Unit {
            scalar: 1.0,
            offset: 0.0,
            dimensions: Dimensions::DIMENSIONLESS,
            display: UnitExpr::single("rad"),
        }),
    }
}

/// If `rad` is (to within rounding) an integer multiple of π/2, returns that multiple mod 4.
///
/// Angles such as `180 deg` or `pi` reach the trig functions as the nearest double to the
/// exact multiple of π/2, so `sin(180 deg)` would otherwise print `1.2246e-16` instead of 0.
fn quarter_turns(rad: f64) -> Option<u8> {
    let k = rad / std::f64::consts::FRAC_PI_2;
    let rounded = k.round();
    if rounded == 0.0 {
        // Tiny angles are not "almost zero": sin(1e-13) = 1e-13, so only exact zero snaps.
        return (rad == 0.0).then_some(0);
    }
    if rounded.abs() < 1e15 && (k - rounded).abs() <= 1e-12 * rounded.abs() {
        Some(rounded.rem_euclid(4.0) as u8)
    } else {
        None
    }
}

fn sin_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    let s = match quarter_turns(rad) {
        Some(0 | 2) => 0.0,
        Some(1) => 1.0,
        Some(3) => -1.0,
        _ => rad.sin(),
    };
    Ok(Value::dimensionless(s))
}

fn cos_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    let c = match quarter_turns(rad) {
        Some(0) => 1.0,
        Some(1 | 3) => 0.0,
        Some(2) => -1.0,
        _ => rad.cos(),
    };
    Ok(Value::dimensionless(c))
}

fn tan_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    let t = match quarter_turns(rad) {
        Some(0 | 2) => 0.0,
        Some(_) => {
            return Err(AbacusError::EvaluationError(
                "tan is undefined at odd multiples of 90°".to_string(),
            ));
        }
        None => rad.tan(),
    };
    Ok(Value::dimensionless(t))
}

/// Validates a dimensionless argument and checks it lies in `domain`.
fn ranged_arg(val: &Value, domain: impl Fn(f64) -> bool) -> Result<f64, AbacusError> {
    if !val.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let x = val.canonical;
    if !domain(x) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(x)
}

fn asin_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| (-1.0..=1.0).contains(&x))?;
    Ok(make_angle_rad(x.asin()))
}

fn acos_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| (-1.0..=1.0).contains(&x))?;
    Ok(make_angle_rad(x.acos()))
}

fn atan_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| !x.is_nan())?;
    Ok(make_angle_rad(x.atan()))
}

fn atan2_fn(args: &[Value]) -> Result<Value, AbacusError> {
    if !args[0].unit.is_compatible_with(&args[1].unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let y = args[0].canonical;
    let x = args[1].canonical;
    Ok(make_angle_rad(y.atan2(x)))
}

fn sinh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    Ok(Value::dimensionless(rad.sinh()))
}

fn cosh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    Ok(Value::dimensionless(rad.cosh()))
}

fn tanh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let rad = parse_angle_radians(&args[0])?;
    Ok(Value::dimensionless(rad.tanh()))
}

// Inverse hyperbolic functions return plain numbers: their results are not angles.
fn asinh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| !x.is_nan())?;
    Ok(Value::dimensionless(x.asinh()))
}

fn acosh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| x >= 1.0)?;
    Ok(Value::dimensionless(x.acosh()))
}

fn atanh_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = ranged_arg(&args[0], |x| x > -1.0 && x < 1.0)?;
    Ok(Value::dimensionless(x.atanh()))
}

/// Reads a bare number (no unit written) as an angle in the configured angle mode.
fn angle_arg(ctx: &FunctionContext<'_>, arg: &EvalResult) -> Result<Value, AbacusError> {
    let mut v = arg.as_scalar()?.clone();
    if v.unit.is_dimensionless() && v.unit.display.is_empty() {
        v.canonical *= match ctx.angle_mode {
            AngleMode::Degrees => std::f64::consts::PI / 180.0,
            AngleMode::Gradians => std::f64::consts::PI / 200.0,
            AngleMode::Radians => 1.0,
        };
    }
    Ok(v)
}

/// A trig function whose argument honours the angle mode.
fn forward(name: &'static str, f: fn(&[Value]) -> Result<Value, AbacusError>) -> FunctionOp {
    FunctionOp::with_context(name, 1, 1, move |ctx, args| {
        f(&[angle_arg(ctx, &args[0])?]).map(EvalResult::Scalar)
    })
}

/// An inverse trig function whose result is expressed in the angle mode's unit.
fn inverse(
    name: &'static str,
    arity: usize,
    f: fn(&[Value]) -> Result<Value, AbacusError>,
) -> FunctionOp {
    FunctionOp::with_context(name, arity, arity, move |ctx, args| {
        let values = args
            .iter()
            .map(|a| a.as_scalar().cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let rad = f(&values)?;
        let unit = match ctx.angle_mode {
            AngleMode::Degrees => "deg",
            AngleMode::Gradians => "grad",
            AngleMode::Radians => return Ok(EvalResult::Scalar(rad)),
        };
        Ok(EvalResult::Scalar(rad.convert_to(ctx.units.unit(unit)?)?))
    })
}

pub fn register_trig() -> Vec<FunctionOp> {
    vec![
        forward("sin", sin_fn),
        forward("cos", cos_fn),
        forward("tan", tan_fn),
        inverse("asin", 1, asin_fn),
        inverse("acos", 1, acos_fn),
        inverse("atan", 1, atan_fn),
        inverse("atan2", 2, atan2_fn),
        FunctionOp::scalar("sinh", 1, 1, sinh_fn),
        FunctionOp::scalar("cosh", 1, 1, cosh_fn),
        FunctionOp::scalar("tanh", 1, 1, tanh_fn),
        FunctionOp::scalar("asinh", 1, 1, asinh_fn),
        FunctionOp::scalar("acosh", 1, 1, acosh_fn),
        FunctionOp::scalar("atanh", 1, 1, atanh_fn),
    ]
}
