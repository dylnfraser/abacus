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

fn sqrt(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("sqrt"));
    }
    if a.canonical < 0.0 {
        return Err(AbacusError::IncompatibleDimensions);
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
        // Negation shares precedence 2 with sqrt so that `sqrt -9` correctly
        // negates first, then takes the root. Both run before `^` (precedence 3)
        // so `-2^2` evaluates as -(2^2) = -4, matching mathematical convention.
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
            alias: "!",
            func: factorial,
            precedence: 3,
            prefix: false,
        },
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
