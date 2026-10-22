use crate::{
    AbacusError, Unit, Value,
    evaluation::tokenizer::registry::function::distributions::special::factorial as fact_u64,
    evaluation::tokenizer::registry::unary::operators::UnaryOp,
};
use std::sync::Arc;

fn negate(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        // `-10 °C` negates the displayed amount, not the absolute temperature.
        return Ok(Value::new(-a.amount(), a.unit));
    }
    Ok(Value {
        canonical: -a.canonical,
        unit: a.unit,
    })
}

/// Error returned for the square root of a negative quantity (dimensionless or not).
pub(crate) fn negative_sqrt_error() -> AbacusError {
    AbacusError::EvaluationError("cannot take the square root of a negative value".to_string())
}

fn sqrt(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("sqrt"));
    }
    if a.canonical < 0.0 {
        return Err(negative_sqrt_error());
    }
    if a.unit.is_plain_number() || a.unit.is_percent() {
        return Ok(Value {
            canonical: a.canonical.sqrt(),
            unit: a.unit,
        });
    }

    let new_dims = a
        .unit
        .dimensions
        .checked_mul_f64(0.5)
        .ok_or(AbacusError::ExponentLimitExceeded)?;

    let new_unit = Arc::new(Unit {
        scalar: a.unit.scalar.sqrt(),
        offset: 0.0,
        dimensions: new_dims,
        display: a.unit.display.powf(0.5),
    });

    Ok(Value {
        canonical: a.canonical.sqrt(),
        unit: new_unit,
    })
}

/// Real cube root; unlike `sqrt` it accepts negative values (`cbrt(-27)` = -3) and divides
/// unit exponents by three (`cbrt(8 m^3)` = 2 m).
fn cbrt(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("cbrt"));
    }
    if a.unit.is_plain_number() || a.unit.is_percent() {
        return Ok(Value {
            canonical: a.canonical.cbrt(),
            unit: a.unit,
        });
    }
    let new_dims = a
        .unit
        .dimensions
        .checked_mul_f64(1.0 / 3.0)
        .ok_or(AbacusError::ExponentLimitExceeded)?;
    let new_unit = Arc::new(Unit {
        scalar: a.unit.scalar.cbrt(),
        offset: 0.0,
        dimensions: new_dims,
        display: a.unit.display.powf(1.0 / 3.0),
    });
    Ok(Value {
        canonical: a.canonical.cbrt(),
        unit: new_unit,
    })
}

fn factorial(a: Value) -> Result<Value, AbacusError> {
    if !a.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    if a.canonical < 0.0 || !a.canonical.is_finite() || a.canonical > 170.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let n = a.canonical.round() as u64;
    Ok(Value {
        canonical: fact_u64(n),
        unit: a.unit,
    })
}

fn increment(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("increment"));
    }
    Ok(Value {
        canonical: a.canonical + a.unit.scalar,
        unit: a.unit,
    })
}

fn decrement(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("decrement"));
    }
    Ok(Value {
        canonical: a.canonical - a.unit.scalar,
        unit: a.unit,
    })
}

#[must_use]
pub fn register_general() -> Vec<UnaryOp> {
    vec![
        // Binding powers are `precedence * 2 + 2`. Negation and sqrt (precedence 2 -> bp 6) bind
        // tighter than `*` (bp 4) but looser than `^` (left bp 9), so `-2^2` evaluates as
        // -(2^2) = -4, matching mathematical convention, and `sqrt -9` negates first.
        UnaryOp {
            alias: "-",
            func: negate,
            precedence: 2,
            prefix: true,
        },
        UnaryOp {
            alias: "sqrt",
            func: sqrt,
            precedence: 2,
            prefix: true,
        },
        UnaryOp {
            alias: "cbrt",
            func: cbrt,
            precedence: 2,
            prefix: true,
        },
        // Postfix factorial (precedence 4 -> bp 10) binds tighter than `^`: `2^3!` = 2^6.
        UnaryOp {
            alias: "!",
            func: factorial,
            precedence: 4,
            prefix: false,
        },
        // `++`/`--` are increment/decrement only at the start of an operand or at the end of a
        // (sub)expression; the lexer splits them elsewhere so `3--2` = 5 and `2++3` = 5.
        UnaryOp {
            alias: "++",
            func: increment,
            precedence: 2,
            prefix: true,
        },
        UnaryOp {
            alias: "--",
            func: decrement,
            precedence: 2,
            prefix: true,
        },
    ]
}
