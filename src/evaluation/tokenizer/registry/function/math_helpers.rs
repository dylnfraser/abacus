use crate::{
    AbacusError, Value, evaluation::tokenizer::registry::function::operators::FunctionOp,
    units::unit::Unit,
};
use std::sync::Arc;

/// Validates that all values in the slice are dimensionless.
pub fn check_dimensionless(args: &[Value]) -> Result<(), AbacusError> {
    if args.iter().all(|a| a.unit.is_dimensionless()) {
        Ok(())
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

/// Returns the unit to use for a *difference* (spread) of values measured in `unit`.
///
/// For affine units such as °C or °F, a difference is not itself a temperature reading, so it
/// is reported in the matching difference unit — K for °C, °R for °F — exactly as
/// subtraction (`30 °F - 20 °F` → `10 °R`) does. Other units are returned unchanged.
#[must_use]
pub fn delta_unit(unit: &Arc<Unit>) -> Arc<Unit> {
    if unit.is_affine() {
        crate::units::value::temperature_delta_unit(unit)
    } else {
        Arc::clone(unit)
    }
}

/// Interprets a value as a difference/spread (e.g. a standard deviation) and returns its
/// canonical magnitude. For affine units, `2 °C` as a spread means 2 K, not 275.15 K.
#[inline]
#[must_use]
pub fn delta_canonical(value: &Value) -> f64 {
    value.canonical - value.unit.offset
}

/// Validates a strictly positive spread parameter (σ, s) with units compatible with `unit`,
/// returning its canonical magnitude.
pub fn positive_spread(value: &Value, unit: &Unit) -> Result<f64, AbacusError> {
    if !value.unit.is_compatible_with(unit) {
        return Err(AbacusError::IncompatibleDimensions);
    }
    let s = delta_canonical(value);
    if !(s > 0.0 && s.is_finite()) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(s)
}

/// Largest integer that f64 represents exactly (2^53).
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// Returns `x` as a non-negative integer, rejecting fractional, negative, non-finite, or
/// inexact (> 2^53) input with `IncompatibleFunctionArguments`.
///
/// Error convention for the function registry: invalid *arguments* (out of domain, wrong
/// sign, non-integer) are `IncompatibleFunctionArguments`; failures of the *computation*
/// (overflow, non-convergence, an undefined result) are `EvaluationError` with a message.
pub fn non_negative_integer(x: f64) -> Result<u64, AbacusError> {
    let rounded = x.round();
    if !x.is_finite() || rounded < 0.0 || (x - rounded).abs() > 1e-9 || rounded > MAX_EXACT_INTEGER
    {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(rounded as u64)
}

/// Validates a dimensionless, non-negative integer argument.
pub fn integer_arg(value: &Value) -> Result<u64, AbacusError> {
    if !value.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    non_negative_integer(value.canonical)
}

/// Validates a sample-size argument `n`: dimensionless integer ≥ `min`.
pub fn sample_size(value: &Value, min: u64) -> Result<f64, AbacusError> {
    let n = integer_arg(value)?;
    if n < min {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    Ok(n as f64)
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn lcm_u64(a: u64, b: u64) -> Option<u64> {
    if a == 0 || b == 0 {
        Some(0)
    } else {
        (a / gcd_u64(a, b)).checked_mul(b)
    }
}

fn integer_magnitude(value: &Value) -> Result<u64, AbacusError> {
    if !value.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    non_negative_integer(value.canonical.abs())
}

/// clamp(x, min, max) — clamps x within [min, max]
fn clamp_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let x = &args[0];
    let min = &args[1];
    let max = &args[2];

    // Check unit compatibility or promote dimensionless bounds
    let min_canonical = if min.unit.is_compatible_with(&x.unit) {
        min.canonical
    } else if min.unit.is_dimensionless() && !x.unit.is_dimensionless() {
        let min_amount = min.amount();
        Value::new(min_amount, Arc::clone(&x.unit)).canonical
    } else {
        return Err(AbacusError::IncompatibleDimensions);
    };

    let max_canonical = if max.unit.is_compatible_with(&x.unit) {
        max.canonical
    } else if max.unit.is_dimensionless() && !x.unit.is_dimensionless() {
        let max_amount = max.amount();
        Value::new(max_amount, Arc::clone(&x.unit)).canonical
    } else {
        return Err(AbacusError::IncompatibleDimensions);
    };

    if min_canonical > max_canonical || min_canonical.is_nan() || max_canonical.is_nan() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let clamped_canonical = x.canonical.clamp(min_canonical, max_canonical);

    Ok(Value {
        canonical: clamped_canonical,
        unit: Arc::clone(&x.unit),
    })
}

/// gcd(a, b) — Greatest Common Divisor of two integers
fn gcd_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let a = integer_magnitude(&args[0])?;
    let b = integer_magnitude(&args[1])?;
    Ok(Value::dimensionless(gcd_u64(a, b) as f64))
}

/// lcm(a, b) — Least Common Multiple of two integers; errors if the result overflows u64.
fn lcm_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let a = integer_magnitude(&args[0])?;
    let b = integer_magnitude(&args[1])?;
    let l = lcm_u64(a, b)
        .ok_or_else(|| AbacusError::EvaluationError("lcm result overflows".to_string()))?;
    Ok(Value::dimensionless(l as f64))
}

/// Unit-aware floored modulo: `a - b * floor(a / b)`.
///
/// The result has the sign of the divisor (like Excel's `MOD` and Python's `%`), so
/// `mod(-7, 3) = 2`. The calculation is done in canonical units, so `mod(1 m, 30 cm)` is
/// `0.1 m`; the result is expressed in the unit of `a`. A dimensionless divisor applied to a
/// dimensional `a` is interpreted in `a`'s unit (`mod(7 m, 3) = 1 m`).
pub fn compute_modulo(a: &Value, b: &Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() || b.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("modulo"));
    }

    let divisor = if a.unit.is_compatible_with(&b.unit) {
        b.canonical
    } else if b.unit.is_dimensionless() {
        b.canonical * a.unit.scalar
    } else {
        return Err(AbacusError::IncompatibleDimensions);
    };

    if divisor == 0.0 || !divisor.is_finite() || !a.canonical.is_finite() {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let mut rem = a.canonical % divisor;
    if rem != 0.0 && (rem < 0.0) != (divisor < 0.0) {
        rem += divisor;
    }
    Ok(Value::from_canonical(rem, Arc::clone(&a.unit)))
}

fn modulo_fn(args: &[Value]) -> Result<Value, AbacusError> {
    compute_modulo(&args[0], &args[1])
}

pub fn register_math_helpers() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("clamp", 3, 3, clamp_fn),
        FunctionOp::scalar("gcd", 2, 2, gcd_fn),
        FunctionOp::scalar("lcm", 2, 2, lcm_fn),
        FunctionOp::scalar("mod", 2, 2, modulo_fn),
        FunctionOp::scalar("modulo", 2, 2, modulo_fn),
    ]
}
