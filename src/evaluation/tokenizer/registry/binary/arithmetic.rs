use crate::{AbacusError, Value, evaluation::tokenizer::registry::binary::operators::BinaryOp};

fn add(a: Value, b: Value) -> Result<Value, AbacusError> {
    a + b
}

fn sub(a: Value, b: Value) -> Result<Value, AbacusError> {
    a - b
}

fn mul(a: Value, b: Value) -> Result<Value, AbacusError> {
    a * b
}

fn div(a: Value, b: Value) -> Result<Value, AbacusError> {
    a / b
}

fn pow(a: Value, b: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() || b.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("multiply"));
    }

    if b.unit.is_dimensionless() {
        let mut new_display = a.unit.display.clone();
        if !a.unit.is_dimensionless() {
            let exp = b.canonical;
            // Small integer powers repeat each symbol (`km^2` -> `km*km`) so the result
            // simplifies the same way as the equivalent product; others stay opaque.
            if exp == exp.trunc() && exp.abs() <= 8.0 {
                let n = exp.abs() as usize;
                let (num, den) = if exp >= 0.0 {
                    (&a.unit.display.numerator, &a.unit.display.denominator)
                } else {
                    (&a.unit.display.denominator, &a.unit.display.numerator)
                };
                new_display.numerator = num.iter().cycle().take(num.len() * n).cloned().collect();
                new_display.denominator = den.iter().cycle().take(den.len() * n).cloned().collect();
            } else {
                let current = a.unit.display.render();
                new_display.numerator.clear();
                new_display.denominator.clear();
                new_display
                    .numerator
                    .push(format!("({})^{}", current, b.canonical));
            }
        }

        let new_unit = std::sync::Arc::new(crate::units::unit::Unit {
            scalar: a.unit.scalar.powf(b.canonical),
            offset: 0.0, // exponents drop the offset for affine units, but we already errored out if affine
            dimensions: a.unit.dimensions * b.canonical,
            display: new_display,
        });

        Ok(Value {
            canonical: a.canonical.powf(b.canonical),
            unit: new_unit,
        })
    } else {
        Err(AbacusError::IncompatibleDimensions)
    }
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
