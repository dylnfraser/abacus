use crate::{AbacusError, Value, evaluation::tokenizer::registry::binary::operators::BinaryOp};

pub(crate) fn add(a: Value, b: Value) -> Result<Value, AbacusError> {
    a + b
}

pub(crate) fn sub(a: Value, b: Value) -> Result<Value, AbacusError> {
    a - b
}

pub(crate) fn mul(a: Value, b: Value) -> Result<Value, AbacusError> {
    a * b
}

pub(crate) fn div(a: Value, b: Value) -> Result<Value, AbacusError> {
    a / b
}

pub(crate) fn pow(a: Value, b: Value) -> Result<Value, AbacusError> {
    if b.unit.is_affine() || !b.unit.is_dimensionless() {
        return Err(AbacusError::IncompatibleDimensions);
    }
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("exponentiate"));
    }
    let exp = b.canonical;
    let canonical = a.canonical.powf(exp);

    // Percentages are plain ratios: (5%)^2 = 0.25%.
    if a.unit.is_percent() || a.unit.is_plain_number() {
        return Ok(Value {
            canonical,
            unit: a.unit,
        });
    }

    let dimensions = a
        .unit
        .dimensions
        .checked_mul_f64(exp)
        .ok_or(AbacusError::ExponentLimitExceeded)?;
    let new_unit = std::sync::Arc::new(crate::units::unit::Unit {
        scalar: a.unit.scalar.powf(exp),
        offset: 0.0,
        dimensions,
        display: a.unit.display.powf(exp),
    });

    Ok(Value {
        canonical,
        unit: new_unit,
    })
}

/// Returns `true` if `op` divides its operands (`/`, `per`, or a custom alias of division).
#[must_use]
pub fn is_division(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, div as fn(Value, Value) -> Result<Value, AbacusError>)
}

/// Returns `true` if `op` is the built-in addition.
#[must_use]
pub fn is_addition(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, add as fn(Value, Value) -> Result<Value, AbacusError>)
}

/// Returns `true` if `op` is the built-in subtraction.
#[must_use]
pub fn is_subtraction(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, sub as fn(Value, Value) -> Result<Value, AbacusError>)
}

/// Returns `true` if `op` is the built-in multiplication.
#[must_use]
pub fn is_multiplication(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, mul as fn(Value, Value) -> Result<Value, AbacusError>)
}

/// Returns `true` if `op` is the built-in exponentiation.
#[must_use]
pub fn is_power(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, pow as fn(Value, Value) -> Result<Value, AbacusError>)
}

/// Returns `true` if `op` is the built-in remainder.
#[must_use]
pub fn is_remainder(op: &BinaryOp) -> bool {
    std::ptr::fn_addr_eq(op.func, rem as fn(Value, Value) -> Result<Value, AbacusError>)
}

fn rem(a: Value, b: Value) -> Result<Value, AbacusError> {
    crate::evaluation::tokenizer::registry::function::math_helpers::compute_modulo(&a, &b)
}

fn percent_of(a: Value, b: Value) -> Result<Value, AbacusError> {
    a * b
}

fn percent_off(a: Value, b: Value) -> Result<Value, AbacusError> {
    if a.unit.is_percent() {
        let factor = 1.0 - a.canonical;
        Ok(Value {
            canonical: b.canonical * factor,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else if b.unit.is_percent() {
        let factor = 1.0 - b.canonical;
        Ok(Value {
            canonical: a.canonical * factor,
            unit: std::sync::Arc::clone(&a.unit),
        })
    } else if a.unit.is_compatible_with(&b.unit) {
        Ok(Value {
            canonical: b.canonical - a.canonical,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

fn more_than(a: Value, b: Value) -> Result<Value, AbacusError> {
    if a.unit.is_percent() {
        let factor = 1.0 + a.canonical;
        Ok(Value {
            canonical: b.canonical * factor,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else if b.unit.is_percent() {
        let factor = 1.0 + b.canonical;
        Ok(Value {
            canonical: a.canonical * factor,
            unit: std::sync::Arc::clone(&a.unit),
        })
    } else if a.unit.is_compatible_with(&b.unit) {
        Ok(Value {
            canonical: b.canonical + a.canonical,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

fn less_than(a: Value, b: Value) -> Result<Value, AbacusError> {
    if a.unit.is_percent() {
        let factor = 1.0 - a.canonical;
        Ok(Value {
            canonical: b.canonical * factor,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else if b.unit.is_percent() {
        let factor = 1.0 - b.canonical;
        Ok(Value {
            canonical: a.canonical * factor,
            unit: std::sync::Arc::clone(&a.unit),
        })
    } else if a.unit.is_compatible_with(&b.unit) {
        Ok(Value {
            canonical: b.canonical - a.canonical,
            unit: std::sync::Arc::clone(&b.unit),
        })
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
}

#[must_use]
pub fn register_arithmetic() -> Vec<BinaryOp> {
    vec![
        BinaryOp {
            alias: "+",
            func: add,
            precedence: 0,
            right_associative: false,
        },
        BinaryOp {
            alias: "-",
            func: sub,
            precedence: 0,
            right_associative: false,
        },
        BinaryOp {
            alias: "*",
            func: mul,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "/",
            func: div,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "per",
            func: div,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "%",
            func: rem,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "of",
            func: percent_of,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "off",
            func: percent_off,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "more than",
            func: more_than,
            precedence: 1,
            right_associative: false,
        },
        BinaryOp {
            alias: "less than",
            func: less_than,
            precedence: 1,
            right_associative: false,
        },
        // Exponentiation has higher precedence than multiplication and is
        // right-associative: 2^3^2 = 2^(3^2) = 512, not (2^3)^2 = 64.
        BinaryOp {
            alias: "^",
            func: pow,
            precedence: 3,
            right_associative: true,
        },
    ]
}
