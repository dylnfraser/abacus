use std::sync::Arc;

use crate::error::AbacusError;
use crate::registry::UnitRegistry;
use crate::units::unit::Unit;
use crate::units::value::Value;

use crate::evaluation::tokenizer::registry::binary::operators::BinaryOp;
use crate::evaluation::tokenizer::registry::unary::operators::UnaryOp;

/// A physical interval representing a guaranteed range `[lo, hi]` where both
/// endpoints carry physical units. Used for worst-case/best-case analysis in
/// engineering and safety-critical computations.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntervalStyle {
    #[default]
    Bracket,
    Range,
}

/// A physical interval representing a guaranteed range `[lo, hi]` or `lo..hi` where both
/// endpoints carry physical units. Used for worst-case/best-case analysis in
/// engineering and safety-critical computations.
///
/// # Examples
///
/// ```rust
/// use abacus::eval;
///
/// let res = eval("[1 m, 2 m] + 50 cm").unwrap();
/// assert_eq!(res.to_display(), "[1.5 m, 2.5 m]");
///
/// let range = eval("1..10 + 5").unwrap();
/// assert_eq!(range.to_display(), "6..15");
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Interval {
    pub lo: Value,
    pub hi: Value,
    pub style: IntervalStyle,
}

impl Interval {
    /// Create a new interval from two `Value` endpoints.
    /// Validates dimension compatibility and normalizes so `lo.canonical <= hi.canonical`.
    pub fn new(a: Value, b: Value) -> Result<Self, AbacusError> {
        Self::new_with_style(a, b, IntervalStyle::Bracket)
    }

    /// Create a new interval with an explicit display style (Bracket or Range).
    pub fn new_with_style(a: Value, b: Value, style: IntervalStyle) -> Result<Self, AbacusError> {
        let (a, b) = if a.unit.is_dimensionless() && !b.unit.is_dimensionless() {
            (Value::new(a.canonical, Arc::clone(&b.unit)), b)
        } else if !a.unit.is_dimensionless() && b.unit.is_dimensionless() {
            let unit = Arc::clone(&a.unit);
            (a, Value::new(b.canonical, unit))
        } else {
            (a, b)
        };

        if !a.unit.is_compatible_with(&b.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        if a.canonical <= b.canonical {
            Ok(Self {
                lo: a,
                hi: b,
                style,
            })
        } else {
            Ok(Self {
                lo: b,
                hi: a,
                style,
            })
        }
    }

    /// Promote a single scalar `Value` to a degenerate interval `[v, v]`.
    #[must_use]
    pub fn from_value(v: Value) -> Self {
        Self::from_value_with_style(v, IntervalStyle::Bracket)
    }

    /// Promote a single scalar `Value` to a degenerate interval with an explicit style.
    #[must_use]
    pub fn from_value_with_style(v: Value, style: IntervalStyle) -> Self {
        Self {
            lo: v.clone(),
            hi: v,
            style,
        }
    }

    /// Apply a binary operator across two intervals by evaluating all four corner
    /// combinations and taking the min/max of the results.
    ///
    /// For monotonic operations (e.g. addition) this is exact.
    /// For non-monotonic operations this is conservative (may overestimate the interval).
    pub fn apply_binary(&self, op: &BinaryOp, other: &Interval) -> Result<Interval, AbacusError> {
        let style = if self.style == IntervalStyle::Range || other.style == IntervalStyle::Range {
            IntervalStyle::Range
        } else {
            IntervalStyle::Bracket
        };

        // Division by an interval containing zero crosses a singularity / pole.
        if op.alias == "/" && other.lo.canonical <= 0.0 && other.hi.canonical >= 0.0 {
            let sample_rhs = if other.hi.canonical > 0.0 {
                other.hi.clone()
            } else if other.lo.canonical < 0.0 {
                other.lo.clone()
            } else {
                Value::new(1.0, Arc::clone(&other.lo.unit))
            };
            let sample_unit = (self.lo.clone() / sample_rhs)?.unit;

            let (lo_val, hi_val) = if (other.lo.canonical == 0.0 && other.hi.canonical == 0.0)
                || (other.lo.canonical < 0.0 && other.hi.canonical > 0.0)
            {
                (f64::NEG_INFINITY, f64::INFINITY)
            } else if other.lo.canonical == 0.0 {
                if self.lo.canonical > 0.0 {
                    (self.lo.canonical / other.hi.canonical, f64::INFINITY)
                } else if self.hi.canonical < 0.0 {
                    (f64::NEG_INFINITY, self.hi.canonical / other.hi.canonical)
                } else {
                    (f64::NEG_INFINITY, f64::INFINITY)
                }
            } else {
                if self.lo.canonical > 0.0 {
                    (f64::NEG_INFINITY, self.lo.canonical / other.lo.canonical)
                } else if self.hi.canonical < 0.0 {
                    (self.hi.canonical / other.lo.canonical, f64::INFINITY)
                } else {
                    (f64::NEG_INFINITY, f64::INFINITY)
                }
            };

            let lo = Value {
                canonical: lo_val,
                unit: Arc::clone(&sample_unit),
            };
            let hi = Value {
                canonical: hi_val,
                unit: sample_unit,
            };
            return Ok(Interval { lo, hi, style });
        }

        // Monotonic fast-paths:
        // Addition: [a, b] + [c, d] = [a + c, b + d] (2 additions instead of 4)
        if op.alias == "+" {
            let lo = op.apply(self.lo.clone(), other.lo.clone())?;
            let hi = op.apply(self.hi.clone(), other.hi.clone())?;
            return Ok(Interval { lo, hi, style });
        }

        // Subtraction: [a, b] - [c, d] = [a - d, b - c] (2 subtractions instead of 4)
        if op.alias == "-" {
            let lo = op.apply(self.lo.clone(), other.hi.clone())?;
            let hi = op.apply(self.hi.clone(), other.lo.clone())?;
            return Ok(Interval { lo, hi, style });
        }

        let corners = [
            op.apply(self.lo.clone(), other.lo.clone())?,
            op.apply(self.lo.clone(), other.hi.clone())?,
            op.apply(self.hi.clone(), other.lo.clone())?,
            op.apply(self.hi.clone(), other.hi.clone())?,
        ];

        let lo = corners
            .iter()
            .min_by(|a, b| {
                a.canonical
                    .partial_cmp(&b.canonical)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .ok_or(AbacusError::IncompatibleDimensions)?;

        let hi = corners
            .iter()
            .max_by(|a, b| {
                a.canonical
                    .partial_cmp(&b.canonical)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
            .ok_or(AbacusError::IncompatibleDimensions)?;

        Ok(Interval { lo, hi, style })
    }

    /// Apply a unary operator to an interval by evaluating both endpoints
    /// and normalizing the result.
    pub fn apply_unary(&self, op: &UnaryOp) -> Result<Interval, AbacusError> {
        let a = op.apply(self.lo.clone())?;
        let b = op.apply(self.hi.clone())?;

        if a.canonical <= b.canonical {
            Ok(Interval {
                lo: a,
                hi: b,
                style: self.style,
            })
        } else {
            Ok(Interval {
                lo: b,
                hi: a,
                style: self.style,
            })
        }
    }

    /// Convert both endpoints to a target unit.
    pub fn convert_to(&self, unit: Arc<Unit>) -> Result<Interval, AbacusError> {
        let lo = self.lo.convert_to(Arc::clone(&unit))?;
        let hi = self.hi.convert_to(unit)?;
        if lo.canonical <= hi.canonical {
            Ok(Interval {
                lo,
                hi,
                style: self.style,
            })
        } else {
            Ok(Interval {
                lo: hi,
                hi: lo,
                style: self.style,
            })
        }
    }

    /// Attempt automatic derived-unit reduction on both endpoints.
    pub fn to_derived(&self, registry: &UnitRegistry) -> Result<Interval, AbacusError> {
        let lo = self.lo.to_derived(registry)?;
        let hi = self.hi.to_derived(registry)?;
        Ok(Interval {
            lo,
            hi,
            style: self.style,
        })
    }

    /// Simplify the unit display on both endpoints.
    pub fn simplify_unit_display(&mut self, registry: &UnitRegistry) {
        self.lo.simplify_unit_display(registry);
        self.hi.simplify_unit_display(registry);
    }

    /// Render the interval as `[lo, hi]` or `lo..hi` depending on style.
    #[must_use]
    pub fn to_display(&self) -> String {
        self.to_string()
    }

    /// Returns a new `Interval` with both endpoints rounded to `sig_figs` significant figures.
    #[must_use]
    pub fn round_to_sig_figs(&self, sig_figs: usize) -> Self {
        Self {
            lo: self.lo.round_to_sig_figs(sig_figs),
            hi: self.hi.round_to_sig_figs(sig_figs),
            style: self.style,
        }
    }

    /// Formats the interval with both endpoints rounded to `sig_figs` significant figures.
    #[must_use]
    pub fn to_display_with_sig_figs(&self, sig_figs: usize) -> String {
        match self.style {
            IntervalStyle::Bracket => {
                format!(
                    "[{}, {}]",
                    self.lo.to_display_with_sig_figs(sig_figs),
                    self.hi.to_display_with_sig_figs(sig_figs)
                )
            }
            IntervalStyle::Range => {
                format!(
                    "{}..{}",
                    self.lo.to_display_with_sig_figs(sig_figs),
                    self.hi.to_display_with_sig_figs(sig_figs)
                )
            }
        }
    }

    /// Returns a new `Interval` with the specified display style.
    #[must_use]
    pub fn with_style(mut self, style: IntervalStyle) -> Self {
        self.style = style;
        self
    }

    /// Returns a new `Interval` with both endpoints rounded to `decimals` decimal places.
    #[must_use]
    pub fn round_to_decimals(&self, decimals: usize) -> Self {
        Self {
            lo: self.lo.round_to_decimals(decimals),
            hi: self.hi.round_to_decimals(decimals),
            style: self.style,
        }
    }

    /// Formats the interval with both endpoints rounded to `decimals` decimal places.
    #[must_use]
    pub fn to_display_with_decimals(&self, decimals: usize) -> String {
        match self.style {
            IntervalStyle::Bracket => {
                format!(
                    "[{}, {}]",
                    self.lo.to_display_with_decimals(decimals),
                    self.hi.to_display_with_decimals(decimals)
                )
            }
            IntervalStyle::Range => {
                format!(
                    "{}..{}",
                    self.lo.to_display_with_decimals(decimals),
                    self.hi.to_display_with_decimals(decimals)
                )
            }
        }
    }

    /// Returns a new `Interval` with display overrides applied to both endpoints.
    #[must_use]
    pub fn with_display_override(
        &self,
        overrides: &std::collections::HashMap<String, String>,
    ) -> Self {
        Self {
            lo: self.lo.with_display_override(overrides),
            hi: self.hi.with_display_override(overrides),
            style: self.style,
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.style {
            IntervalStyle::Bracket => write!(f, "[{}, {}]", self.lo, self.hi),
            IntervalStyle::Range => write!(f, "{}..{}", self.lo, self.hi),
        }
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        self.lo == other.lo && self.hi == other.hi
    }
}
