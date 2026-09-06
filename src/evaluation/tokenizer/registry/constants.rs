use crate::units::{eval_result::EvalResult, value::Value};
use std::collections::HashMap;

/// Golden ratio constant \(\phi = \frac{1 + \sqrt{5}}{2}\).
pub const PHI: f64 = 1.618_033_988_749_895;

/// Canonical table of built-in mathematical constants: name and numeric value.
pub const STANDARD_CONSTANTS: &[(&str, f64)] = &[
    ("pi", std::f64::consts::PI),
    ("PI", std::f64::consts::PI),
    ("e", std::f64::consts::E),
    ("E", std::f64::consts::E),
    ("tau", std::f64::consts::TAU),
    ("TAU", std::f64::consts::TAU),
    ("phi", PHI),
    ("PHI", PHI),
];

/// Returns true if `sym` is the name of a standard mathematical constant.
#[inline]
#[must_use]
pub fn is_standard_constant(sym: &str) -> bool {
    STANDARD_CONSTANTS.iter().any(|&(name, _)| name == sym)
}

/// Retrieve the numeric value of a standard constant by name.
#[inline]
#[must_use]
pub fn get_standard_constant_value(name: &str) -> Option<f64> {
    STANDARD_CONSTANTS
        .iter()
        .find(|&&(n, _)| n == name)
        .map(|&(_, v)| v)
}

/// Retrieve an `EvalResult::Scalar` dimensionless value for a standard constant.
#[inline]
#[must_use]
pub fn get_standard_constant(name: &str) -> Option<EvalResult> {
    get_standard_constant_value(name).map(|v| EvalResult::Scalar(Value::dimensionless(v)))
}

/// Construct a map of standard constant variables for evaluation environments.
#[must_use]
pub fn standard_variables_map() -> HashMap<String, EvalResult> {
    STANDARD_CONSTANTS
        .iter()
        .map(|&(name, val)| {
            (
                name.to_string(),
                EvalResult::Scalar(Value::dimensionless(val)),
            )
        })
        .collect()
}
