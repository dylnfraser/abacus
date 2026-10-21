use std::sync::{Arc, OnceLock};

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

/// Built-in binary operators that need special handling (e.g. sound interval arithmetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Rem,
}

/// The standard binary operators, created once and shared by every standard [`TokenRegistry`],
/// each paired with its [`OpKind`].
///
/// Built-ins are identified by the address of this shared `BinaryOp` rather than by comparing
/// function pointers, which Rust does not guarantee to be unique (identical functions may be
/// merged) or stable (one function may have several addresses across codegen units). Operators
/// registered by users — including ones that copy a built-in's `func` — are custom (`None`).
///
/// [`TokenRegistry`]: crate::evaluation::tokenizer::registry::token_registry::TokenRegistry
pub(crate) fn standard_binary_ops() -> &'static [(Arc<BinaryOp>, Option<OpKind>)] {
    static OPS: OnceLock<Vec<(Arc<BinaryOp>, Option<OpKind>)>> = OnceLock::new();
    OPS.get_or_init(|| {
        register_arithmetic()
            .into_iter()
            .map(|op| {
                let kind = match op.alias {
                    "+" => Some(OpKind::Add),
                    "-" => Some(OpKind::Sub),
                    "*" => Some(OpKind::Mul),
                    "/" | "per" => Some(OpKind::Div),
                    "^" => Some(OpKind::Pow),
                    "%" => Some(OpKind::Rem),
                    _ => None,
                };
                (Arc::new(op), kind)
            })
            .collect()
    })
}

/// The built-in kind of `op`, or `None` for custom operators.
///
/// Built-ins are recognised by the identity of the shared operator instances installed by
/// `TokenRegistry::standard()`, not by comparing function pointers; operators registered by
/// users are always custom, even if they reuse a built-in's `func`.
#[must_use]
pub fn op_kind(op: &BinaryOp) -> Option<OpKind> {
    standard_binary_ops()
        .iter()
        .find(|(builtin, _)| std::ptr::eq(&**builtin, op))
        .and_then(|&(_, kind)| kind)
}

/// Returns `true` if `op` is the built-in division (`/` or `per`).
#[must_use]
pub fn is_division(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Div)
}

/// Returns `true` if `op` is the built-in addition.
#[must_use]
pub fn is_addition(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Add)
}

/// Returns `true` if `op` is the built-in subtraction.
#[must_use]
pub fn is_subtraction(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Sub)
}

/// Returns `true` if `op` is the built-in multiplication.
#[must_use]
pub fn is_multiplication(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Mul)
}

/// Returns `true` if `op` is the built-in exponentiation.
#[must_use]
pub fn is_power(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Pow)
}

/// Returns `true` if `op` is the built-in remainder.
#[must_use]
pub fn is_remainder(op: &BinaryOp) -> bool {
    op_kind(op) == Some(OpKind::Rem)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::tokenizer::registry::token_registry::TokenRegistry;

    #[test]
    fn standard_registry_ops_have_builtin_kinds() {
        let reg = TokenRegistry::standard();
        let kind = |alias: &str| op_kind(&reg.binary_operators[alias]);
        assert_eq!(kind("+"), Some(OpKind::Add));
        assert_eq!(kind("-"), Some(OpKind::Sub));
        assert_eq!(kind("*"), Some(OpKind::Mul));
        assert_eq!(kind("/"), Some(OpKind::Div));
        assert_eq!(kind("per"), Some(OpKind::Div));
        assert_eq!(kind("^"), Some(OpKind::Pow));
        assert_eq!(kind("%"), Some(OpKind::Rem));
        assert_eq!(kind("of"), None);
        // Every standard registry shares the same instances.
        let other = TokenRegistry::standard();
        assert!(Arc::ptr_eq(
            &reg.binary_operators["/"],
            &other.binary_operators["/"]
        ));
    }

    #[test]
    fn user_operators_are_custom_even_with_a_builtin_func() {
        let reg = TokenRegistry::standard();
        let copied = BinaryOp {
            alias: "divide",
            func: reg.binary_operators["/"].func,
            precedence: 1,
            right_associative: false,
        };
        assert_eq!(op_kind(&copied), None);
        assert!(!is_division(&copied));
    }
}
