use crate::{
    AbacusError, Unit, Value,
    evaluation::tokenizer::registry::function::distributions::special::factorial as fact_u64,
    evaluation::tokenizer::registry::unary::operators::UnaryOp,
};
use std::sync::Arc;

fn negate(a: Value) -> Result<Value, AbacusError> {
    if a.unit.is_affine() {
        return Err(AbacusError::AffineUnitOperation("negate"));
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

    // Divide all dimensions by 2.0
    let new_dims = a.unit.dimensions * 0.5;

    let mut new_display = a.unit.display.clone();

    let can_halve_counts = |list: &[String]| -> Option<Vec<String>> {
        let mut counts = std::collections::HashMap::with_capacity(list.len());
        let mut order = Vec::with_capacity(list.len());
        for item in list {
            let entry = counts.entry(item.as_str()).or_insert(0usize);
            if *entry == 0 {
                order.push(item.as_str());
            }
            *entry += 1;
        }
        if counts.values().all(|&c| c.is_multiple_of(2)) {
            let mut halved = Vec::with_capacity(list.len() / 2);
            for item in order {
                let count = counts[item];
                for _ in 0..(count / 2) {
                    halved.push(item.to_string());
                }
            }
            Some(halved)
        } else {
            None
        }
    };

    if let (Some(num), Some(den)) = (
        can_halve_counts(&new_display.numerator),
        can_halve_counts(&new_display.denominator),
    ) {
        new_display.numerator = num;
        new_display.denominator = den;
    } else if !a.unit.is_dimensionless() {
        let current = a.unit.display.render();
        new_display.numerator.clear();
        new_display.denominator.clear();
        new_display.numerator.push(format!("({current})^0.5"));
    }

    let new_unit = Arc::new(Unit {
        scalar: a.unit.scalar.sqrt(),
        offset: 0.0,
        dimensions: new_dims,
        display: new_display,
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
