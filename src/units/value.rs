use crate::registry::UnitRegistry;
use crate::{error::AbacusError, units::unit::Unit};

use std::{
    fmt,
    ops::{Add, Div, Mul, Sub},
    sync::Arc,
};

/// A dimensional quantity consisting of a canonical SI scalar and an associated physical unit.
///
/// # Examples
///
/// ```rust
/// use abacus::Abacus;
///
/// let abacus = Abacus::standard();
/// let length = abacus.units.value(5.0, "km").unwrap();
/// assert_eq!(length.to_display(), "5 km");
/// assert_eq!(length.canonical, 5000.0);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Value {
    pub canonical: f64,
    pub unit: Arc<Unit>,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.unit.dimensions == other.unit.dimensions
            && (self.canonical - other.canonical).abs() <= 1e-12 * self.canonical.abs().max(1.0)
    }
}

pub static DIMENSIONLESS_UNIT: std::sync::LazyLock<Arc<Unit>> =
    std::sync::LazyLock::new(Unit::dimensionless_arc);

#[allow(dead_code)]
impl Value {
    #[must_use]
    pub fn new(value: f64, unit: Arc<Unit>) -> Self {
        Self {
            canonical: value * unit.scalar + unit.offset,
            unit,
        }
    }

    /// Creates a dimensionless `Value` with the given scalar.
    #[must_use]
    pub fn dimensionless(val: f64) -> Self {
        Self {
            canonical: val,
            unit: Arc::clone(&DIMENSIONLESS_UNIT),
        }
    }

    /// Returns the display-unit amount: `(canonical − offset) / scalar`.
    #[inline]
    #[must_use]
    pub fn amount(&self) -> f64 {
        (self.canonical - self.unit.offset) / self.unit.scalar
    }

    /// Constructs a `Value` directly from a pre-computed canonical value.
    #[inline]
    #[must_use]
    pub fn from_canonical(canonical: f64, unit: Arc<Unit>) -> Self {
        Self { canonical, unit }
    }

    pub fn convert_to(&self, unit: Arc<Unit>) -> Result<Self, AbacusError> {
        if !self.unit.is_compatible_with(&unit) {
            if self.unit.is_dimensionless() {
                let amount = self.amount();
                return Ok(Self::new(amount, unit));
            }
            return Err(AbacusError::IncompatibleDimensions);
        }

        Ok(Self {
            canonical: self.canonical,
            unit,
        })
    }

    pub fn to(&self, registry: &UnitRegistry, symbol: &str) -> Result<Self, AbacusError> {
        self.convert_to(registry.unit(symbol)?)
    }

    pub fn as_unit(&self, registry: &UnitRegistry, symbol: &str) -> Result<Self, AbacusError> {
        self.to(registry, symbol)
    }

    pub fn to_derived(&self, registry: &UnitRegistry) -> Result<Self, AbacusError> {
        if self.unit.is_dimensionless() {
            return Ok(self.clone());
        }
        if let Some(derived_unit) = registry.find_unit_by_dimensions(&self.unit.dimensions) {
            self.convert_to(derived_unit)
        } else {
            Ok(self.clone())
        }
    }

    #[must_use]
    pub fn to_display(&self) -> String {
        self.to_string()
    }

    #[must_use]
    pub fn to_units_display(&self) -> String {
        self.unit.display.render()
    }

    pub fn simplify_unit_display(&mut self, unit_registry: &UnitRegistry) {
        if self.unit.is_dimensionless() && self.unit.display.is_empty() {
            return;
        }
        let unit = self
            .unit
            .simplify_display_with(|sym| unit_registry.get(sym));
        self.unit = Arc::new(unit);
    }

    #[must_use]
    pub fn to_human_display(&self) -> String {
        if self.unit.dimensions == crate::units::dimensions::Dimensions::TIME {
            format_human_duration(self.canonical)
        } else {
            self.to_display()
        }
    }

    /// Returns a new `Value` with the displayed quantity rounded to `sig_figs` significant figures.
    #[must_use]
    pub fn round_to_sig_figs(&self, sig_figs: usize) -> Self {
        let amt = self.amount();
        let rounded_amt = round_f64_sig_figs(amt, sig_figs);
        let new_canonical = rounded_amt * self.unit.scalar + self.unit.offset;
        Self {
            canonical: new_canonical,
            unit: Arc::clone(&self.unit),
        }
    }

    /// Renders this `Value` formatted to `sig_figs` significant figures.
    #[must_use]
    pub fn to_display_with_sig_figs(&self, sig_figs: usize) -> String {
        let formatted_amt = format_f64_sig_figs(self.amount(), sig_figs);
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Returns a new `Value` with the displayed quantity rounded to `decimals` decimal places.
    #[must_use]
    pub fn round_to_decimals(&self, decimals: usize) -> Self {
        if decimals >= 308 {
            return self.clone();
        }
        let amt = self.amount();
        let scale = 10.0f64.powi(decimals as i32);
        let rounded_amt = (amt * scale).round() / scale;
        let new_canonical = rounded_amt * self.unit.scalar + self.unit.offset;
        Self {
            canonical: new_canonical,
            unit: Arc::clone(&self.unit),
        }
    }

    /// Renders this `Value` formatted to `decimals` decimal places.
    #[must_use]
    pub fn to_display_with_decimals(&self, decimals: usize) -> String {
        let formatted_amt = format!("{:.precision$}", self.amount(), precision = decimals);
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Renders this `Value` in scientific notation.
    #[must_use]
    pub fn to_display_scientific(&self) -> String {
        let formatted_amt = format!("{:e}", self.amount());
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Renders this `Value` in engineering notation (powers of 10 that are multiples of 3).
    #[must_use]
    pub fn to_display_engineering(&self) -> String {
        let formatted_amt = format_engineering(self.amount());
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Returns a copy of this `Value` with its rendered unit replaced if it matches
    /// any entry in `overrides` (e.g. `"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`).
    #[must_use]
    pub fn with_display_override(
        &self,
        overrides: &std::collections::HashMap<String, String>,
    ) -> Self {
        if overrides.is_empty() {
            return self.clone();
        }
        let current_unit_str = self.unit.display.render();
        let normalized = current_unit_str.replace(' ', "");
        if let Some(replacement) = overrides
            .get(&current_unit_str)
            .or_else(|| overrides.get(&normalized))
        {
            let mut new_unit = (*self.unit).clone();
            new_unit.display = crate::units::unit::UnitExpr::single(replacement);
            Self {
                canonical: self.canonical,
                unit: std::sync::Arc::new(new_unit),
            }
        } else {
            self.clone()
        }
    }
}

/// Returns true if `sym` should be rendered as a prefix before the number (e.g. `$`, `€`, `£`, `¥`).
#[must_use]
pub fn is_prefix_symbol(sym: &str) -> bool {
    matches!(
        sym,
        "$" | "€" | "£" | "¥" | "₹" | "₩" | "₺" | "₪" | "฿" | "R$"
    )
}

/// Returns the standard number of decimal places for a currency (0 for JPY, KRW, etc., 2 for USD, EUR, GBP, etc.).
#[must_use]
pub fn currency_decimal_places(unit_str: &str) -> usize {
    match unit_str {
        "JPY" | "jpy" | "yen" | "¥" | "KRW" | "krw" | "won" | "₩" | "HUF" | "huf" | "IDR"
        | "idr" | "rupiah" | "ISK" | "isk" => 0,
        _ => 2,
    }
}

/// Helper to format a value with its unit.
#[must_use]
pub fn format_with_unit(amount: &str, unit: &str) -> String {
    if unit.is_empty() {
        amount.to_string()
    } else if is_prefix_symbol(unit) {
        if let Some(stripped) = amount.strip_prefix('-') {
            format!("-{unit}{stripped}")
        } else {
            format!("{unit}{amount}")
        }
    } else if unit == "%" {
        format!("{amount}%")
    } else if unit == "+%" {
        if amount.starts_with('-') {
            format!("{amount}%")
        } else {
            format!("+{amount}%")
        }
    } else {
        format!("{amount} {unit}")
    }
}

/// Formats a number in engineering notation where exponents are multiples of 3.
#[must_use]
pub fn format_engineering(val: f64) -> String {
    if !val.is_finite() || val == 0.0 {
        return val.to_string();
    }
    let abs_val = val.abs();
    let exp = abs_val.log10().floor() as i32;
    let eng_exp = if exp >= 0 {
        (exp / 3) * 3
    } else {
        ((exp - 2) / 3) * 3
    };
    let mantissa = val / 10.0f64.powi(eng_exp);
    let mantissa_rounded = (mantissa * 1e12).round() / 1e12;
    if eng_exp == 0 {
        format!("{mantissa_rounded}")
    } else {
        format!("{mantissa_rounded}e{eng_exp}")
    }
}

/// Rounds a floating-point number to a specified number of significant figures.
#[must_use]
pub fn round_f64_sig_figs(val: f64, sig_figs: usize) -> f64 {
    if sig_figs == 0 || !val.is_finite() || val == 0.0 {
        return val;
    }
    let magnitude = val.abs().log10().floor();
    let scale = 10.0f64.powf(sig_figs as f64 - 1.0 - magnitude);
    if !scale.is_finite() || scale == 0.0 {
        return val;
    }
    (val * scale).round() / scale
}

/// Formats a floating-point number to a specified number of significant figures,
/// preserving trailing zeros.
#[must_use]
pub fn format_f64_sig_figs(val: f64, sig_figs: usize) -> String {
    if sig_figs == 0 || !val.is_finite() {
        return val.to_string();
    }
    if val == 0.0 {
        if sig_figs > 1 {
            return format!("0.{:0<width$}", "", width = sig_figs - 1);
        }
        return "0".to_string();
    }

    let rounded = round_f64_sig_figs(val, sig_figs);
    if rounded == 0.0 {
        if sig_figs > 1 {
            return format!("0.{:0<width$}", "", width = sig_figs - 1);
        }
        return "0".to_string();
    }

    let magnitude = rounded.abs().log10().floor() as i32;
    let sig_figs_i32 = sig_figs as i32;

    if magnitude >= sig_figs_i32 - 1 {
        if rounded.abs() >= 1e15 {
            format!("{:.precision$e}", rounded, precision = sig_figs - 1)
        } else {
            format!("{:.0}", rounded)
        }
    } else {
        let decimals = (sig_figs_i32 - 1 - magnitude) as usize;
        if rounded.abs() < 1e-4 {
            format!("{:.precision$e}", rounded, precision = sig_figs - 1)
        } else {
            format!("{:.decimals$}", rounded, decimals = decimals)
        }
    }
}

#[must_use]
pub fn format_human_duration(seconds_f64: f64) -> String {
    let is_negative = seconds_f64 < 0.0;
    let mut total = seconds_f64.abs().round() as i64;
    if total == 0 {
        return "0 seconds".to_string();
    }

    const DURATION_UNITS: &[(&str, i64)] = &[
        ("year", 31_536_000),
        ("day", 86_400),
        ("hour", 3_600),
        ("minute", 60),
        ("second", 1),
    ];

    let mut parts = Vec::new();
    for &(name, divisor) in DURATION_UNITS {
        let count = total / divisor;
        total %= divisor;
        if count > 0 {
            parts.push(format!(
                "{count} {name}{suffix}",
                suffix = if count == 1 { "" } else { "s" }
            ));
        }
    }

    let result = parts.join(", ");
    if is_negative {
        format!("-{result}")
    } else {
        result
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.amount();
        let nearest_integer = value.round();
        let is_integer =
            value.is_finite() && (value - nearest_integer).abs() <= 1e-12 * value.abs().max(1.0);
        let display_value = if is_integer { nearest_integer } else { value };

        let unit_str = self.unit.display.render();
        let val_str = if !is_integer
            && self.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY
        {
            let dec = currency_decimal_places(&unit_str);
            format!("{display_value:.dec$}")
        } else {
            display_value.to_string()
        };
        write!(f, "{}", format_with_unit(&val_str, &unit_str))
    }
}

impl Value {
    /// Resolves canonical operand values and target unit for additive operations (+ and -),
    /// handling dimensionless promotion and percentage unit normalization.
    fn resolve_additive_operands(&self, rhs: &Value) -> Result<(f64, f64, Arc<Unit>), AbacusError> {
        if !self.unit.is_compatible_with(&rhs.unit) {
            if rhs.unit.is_dimensionless() && !self.unit.is_dimensionless() {
                let rhs_promoted = Value::new(rhs.amount(), Arc::clone(&self.unit));
                return Ok((
                    self.canonical,
                    rhs_promoted.canonical,
                    Arc::clone(&self.unit),
                ));
            } else if self.unit.is_dimensionless() && !rhs.unit.is_dimensionless() {
                let self_promoted = Value::new(self.amount(), Arc::clone(&rhs.unit));
                return Ok((
                    self_promoted.canonical,
                    rhs.canonical,
                    Arc::clone(&rhs.unit),
                ));
            }
            return Err(AbacusError::IncompatibleDimensions);
        }

        let unit = if self.unit.display.render() == "+%" && rhs.unit.display.render() == "%" {
            Arc::clone(&rhs.unit)
        } else {
            Arc::clone(&self.unit)
        };

        Ok((self.canonical, rhs.canonical, unit))
    }
}

// Add implementations
impl Add<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn add(self, rhs: &Value) -> Self::Output {
        if self.unit.is_affine() || rhs.unit.is_affine() {
            return Err(AbacusError::AffineUnitOperation("add"));
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * (1.0 + rhs.canonical),
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_percent() && !rhs.unit.is_percent() {
            return Ok(Value {
                canonical: rhs.canonical * (1.0 + self.canonical),
                unit: Arc::clone(&rhs.unit),
            });
        }

        let (lhs_can, rhs_can, unit) = self.resolve_additive_operands(rhs)?;
        Ok(Value {
            canonical: lhs_can + rhs_can,
            unit,
        })
    }
}

// Sub implementations
impl Sub<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn sub(self, rhs: &Value) -> Self::Output {
        if self.unit.is_affine() || rhs.unit.is_affine() {
            return Err(AbacusError::AffineUnitOperation("subtract"));
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * (1.0 - rhs.canonical),
                unit: Arc::clone(&self.unit),
            });
        }

        let (lhs_can, rhs_can, unit) = self.resolve_additive_operands(rhs)?;
        Ok(Value {
            canonical: lhs_can - rhs_can,
            unit,
        })
    }
}

// Mul implementations
impl Mul<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn mul(self, rhs: &Value) -> Self::Output {
        if self.unit.is_affine() || rhs.unit.is_affine() {
            return Err(AbacusError::AffineUnitOperation("multiply"));
        }

        if self.unit.is_percent() && !rhs.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * rhs.canonical,
                unit: Arc::clone(&rhs.unit),
            });
        }
        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }

        let unit = Unit {
            scalar: self.unit.scalar * rhs.unit.scalar,
            offset: 0.0,
            dimensions: self.unit.dimensions + rhs.unit.dimensions,
            display: self.unit.display.multiply(&rhs.unit.display),
        };

        Ok(Value {
            canonical: self.canonical * rhs.canonical,
            unit: Arc::new(unit),
        })
    }
}

// Div implementations
impl Div<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn div(self, rhs: &Value) -> Self::Output {
        if self.unit.is_affine() || rhs.unit.is_affine() {
            return Err(AbacusError::AffineUnitOperation("divide"));
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical / rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_percent() && !rhs.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical / rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }

        let unit = Unit {
            scalar: self.unit.scalar / rhs.unit.scalar,
            offset: 0.0,
            dimensions: self.unit.dimensions - rhs.unit.dimensions,
            display: self.unit.display.divide(&rhs.unit.display),
        };

        Ok(Value {
            canonical: self.canonical / rhs.canonical,
            unit: Arc::new(unit),
        })
    }
}

/// Generates the three ownership-coercing forwarding impls for a binary operator whose
/// canonical implementation is `&Value op &Value`.
macro_rules! impl_op_forwarding {
    ($trait:ident, $method:ident) => {
        impl $trait<Value> for Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: Self) -> Self::Output {
                <&Value as $trait<&Value>>::$method(&self, &rhs)
            }
        }
        impl $trait<&Value> for Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: &Value) -> Self::Output {
                <&Value as $trait<&Value>>::$method(&self, rhs)
            }
        }
        impl $trait<Value> for &Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: Value) -> Self::Output {
                <&Value as $trait<&Value>>::$method(self, &rhs)
            }
        }
    };
}

impl_op_forwarding!(Add, add);
impl_op_forwarding!(Sub, sub);
impl_op_forwarding!(Mul, mul);
impl_op_forwarding!(Div, div);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{dimensions::Dimensions, unit::UnitExpr};

    fn unit(scalar: f64, dimensions: Dimensions, display: &str) -> Arc<Unit> {
        Arc::new(Unit {
            scalar,
            dimensions,
            offset: 0f64,
            display: UnitExpr::single(display),
        })
    }

    #[test]
    fn converts_display_values_to_canonical_values() {
        let distance = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let duration = Value::new(1.0, unit(3_600.0, Dimensions::TIME, "h"));

        assert_eq!(distance.canonical, 5_000.0);
        assert_eq!(duration.canonical, 3_600.0);
    }

    #[test]
    fn divides_values_and_units() {
        let distance = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let duration = Value::new(1.0, unit(3_600.0, Dimensions::TIME, "h"));
        let speed = (distance / duration).unwrap();

        assert!((speed.canonical - 5_000.0 / 3_600.0).abs() < f64::EPSILON);
        assert!((speed.unit.scalar - 1_000.0 / 3_600.0).abs() < f64::EPSILON);
        assert_eq!(speed.unit.dimensions, Dimensions::LENGTH - Dimensions::TIME);
        assert_eq!(speed.to_display(), "5 km/h");
    }

    #[test]
    fn adds_and_subtracts_only_compatible_values() {
        let sum = (Value::new(2.0, unit(1_000.0, Dimensions::LENGTH, "km"))
            + Value::new(500.0, unit(1.0, Dimensions::LENGTH, "m")))
        .unwrap();
        assert_eq!(sum.to_display(), "2.5 km");

        let difference = (sum - Value::new(1.0, unit(1_000.0, Dimensions::LENGTH, "km"))).unwrap();
        assert_eq!(difference.to_display(), "1.5 km");

        let incompatible = Value::new(1.0, unit(1.0, Dimensions::LENGTH, "m"))
            + Value::new(1.0, unit(1.0, Dimensions::TIME, "s"));
        assert!(incompatible.is_err());
    }

    #[test]
    fn multiplies_values_with_different_dimensions() {
        let area = (Value::new(2.0, unit(1.0, Dimensions::LENGTH, "m"))
            * Value::new(3.0, unit(1.0, Dimensions::LENGTH, "m")))
        .unwrap();

        assert_eq!(area.canonical, 6.0);
        assert_eq!(
            area.unit.dimensions,
            Dimensions::LENGTH + Dimensions::LENGTH
        );
        assert_eq!(area.to_display(), "6 m^2");
    }

    #[test]
    fn simplifies_compound_unit_displays() {
        let speed = (Value::new(5.0, unit(1.0, Dimensions::LENGTH, "m"))
            / Value::new(1.0, unit(1.0, Dimensions::TIME, "s")))
        .unwrap();
        assert_eq!(speed.to_display(), "5 m/s");

        let mut distance = (speed * Value::new(5.0, unit(1.0, Dimensions::TIME, "s"))).unwrap();
        let std_unit_reg = UnitRegistry::standard();
        distance.simplify_unit_display(&std_unit_reg);

        assert_eq!(distance.canonical, 25.0);
        assert_eq!(distance.unit.dimensions, Dimensions::LENGTH);
        assert_eq!(distance.to_display(), "25 m");
    }

    #[test]
    fn supports_reference_arithmetic() {
        let a = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let b = Value::new(500.0, unit(1.0, Dimensions::LENGTH, "m"));

        let sum = (&a + &b).unwrap();
        assert_eq!(sum.to_display(), "5.5 km");
        // Verify a and b are not consumed
        assert_eq!(a.to_display(), "5 km");
        assert_eq!(b.to_display(), "500 m");
    }

    #[test]
    fn supports_partial_eq_and_display() {
        let a = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let b = Value::new(5000.0, unit(1.0, Dimensions::LENGTH, "m"));

        assert_eq!(a, b);
        assert_eq!(format!("{a}"), "5 km");
    }

    #[test]
    fn automatically_converts_to_matching_derived_units() {
        let registry = UnitRegistry::standard();

        let mass = registry.value(2.0, "kg").unwrap();
        let accel = (registry.value(9.8, "m").unwrap()
            / (registry.value(1.0, "s").unwrap() * registry.value(1.0, "s").unwrap()).unwrap())
        .unwrap();

        let force = (&mass * &accel).unwrap();
        let force_derived = force.to_derived(&registry).unwrap();
        assert_eq!(force_derived.to_display(), "19.6 N");

        let distance = registry.value(5.0, "m").unwrap();
        let work = (&force_derived * &distance).unwrap();
        let work_derived = work.to_derived(&registry).unwrap();
        assert_eq!(work_derived.to_display(), "98 J");
    }

    #[test]
    fn converts_dimensionless_values_to_target_units() {
        let registry = UnitRegistry::standard();
        let dimless = Value::new(10.0, Arc::new(Unit::dimensionless()));
        let converted = dimless.to(&registry, "m").unwrap();
        assert_eq!(converted.to_display(), "10 m");
    }
}
