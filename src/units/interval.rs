use std::sync::Arc;

use crate::error::AbacusError;
use crate::registry::UnitRegistry;
use crate::units::unit::Unit;
use crate::units::value::{ExactDisplay, Value};

use crate::evaluation::tokenizer::registry::binary::arithmetic;
use crate::evaluation::tokenizer::registry::binary::operators::BinaryOp;
use crate::evaluation::tokenizer::registry::unary::operators::UnaryOp;

/// Display style for intervals: `[lo, hi]` (bracket) or `lo..hi` (range).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntervalStyle {
    #[default]
    Bracket,
    Range,
}

/// A physical interval `[lo, hi]` or `lo..hi` where both endpoints carry physical units.
///
/// Built-in arithmetic (`+ - * / per ^ %`) produces enclosures of the true result set:
/// inexact floating-point endpoints are rounded outward (via `next_down`/`next_up`), even
/// powers of intervals spanning zero include zero, and division by an interval containing
/// zero widens to infinity. As with all naive interval arithmetic, dependent operands are
/// treated independently (`[1,2] - [1,2]` is `[-1, 1]`, not `0`).
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "units")] {
/// use abacus::eval;
///
/// let res = eval("[1 m, 2 m] + 50 cm").unwrap();
/// assert_eq!(res.to_display(), "[1.5 m, 2.5 m]");
///
/// let range = eval("1..10 + 5").unwrap();
/// assert_eq!(range.to_display(), "6..15");
/// # }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Interval {
    pub lo: Value,
    pub hi: Value,
    pub style: IntervalStyle,
}

/// Exact rounding error of `a + b` (TwoSum): the true sum is `s + error`.
fn sum_error(a: f64, b: f64, s: f64) -> f64 {
    let bb = s - a;
    (a - (s - bb)) + (b - bb)
}

/// Exact rounding error of `a * b`: the true product is `p + error`.
fn product_error(a: f64, b: f64, p: f64) -> f64 {
    a.mul_add(b, -p)
}

/// Sign of the rounding error of `a / b`: the true quotient is above `q` if positive.
fn quotient_error(a: f64, b: f64, q: f64) -> f64 {
    let remainder = (-q).mul_add(b, a);
    remainder * b.signum()
}

/// A computed endpoint candidate and the sign of its rounding error
/// (`> 0`: true value is larger, `< 0`: smaller, `0`: exact, `NaN`: unknown).
#[derive(Clone, Copy)]
struct Candidate {
    value: f64,
    error: f64,
}

impl Candidate {
    fn exact(value: f64) -> Self {
        Self { value, error: 0.0 }
    }

    fn unknown(value: f64) -> Self {
        Self {
            value,
            error: f64::NAN,
        }
    }
}

/// Returns outward-rounded `(lo, hi)` bounds enclosing every candidate.
fn enclose(candidates: &[Candidate]) -> (f64, f64) {
    if candidates.iter().any(|c| c.value.is_nan()) {
        return (f64::NAN, f64::NAN);
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for c in candidates {
        let finite = c.value.is_finite();
        let down = if finite && (c.error.is_nan() || c.error < 0.0) {
            c.value.next_down()
        } else {
            c.value
        };
        let up = if finite && (c.error.is_nan() || c.error > 0.0) {
            c.value.next_up()
        } else {
            c.value
        };
        lo = lo.min(down);
        hi = hi.max(up);
    }
    (lo, hi)
}

fn is_plain(v: &Value) -> bool {
    !v.unit.is_percent() && !v.unit.is_affine()
}

/// Whether `x^y` computed by `powf` is exactly representable (small integer powers of integers).
fn is_exact_power(x: f64, y: f64, result: f64) -> bool {
    x.fract() == 0.0 && y.fract() == 0.0 && y >= 0.0 && result.abs() < 9_007_199_254_740_992.0
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

    fn is_degenerate(&self) -> bool {
        self.lo.canonical == self.hi.canonical
    }

    fn contains_zero(&self) -> bool {
        self.lo.canonical <= 0.0 && self.hi.canonical >= 0.0
    }

    fn from_bounds(lo: f64, hi: f64, unit: &Arc<Unit>, style: IntervalStyle) -> Interval {
        Interval {
            lo: Value::from_canonical(lo, Arc::clone(unit)),
            hi: Value::from_canonical(hi, Arc::clone(unit)),
            style,
        }
    }

    /// Apply a binary operator across two intervals, returning an enclosure of all results.
    ///
    /// Built-in `+`/`-` use endpoint fast paths, `*`/`/` the four corners, `^` additionally
    /// considers zero when the base spans it, and `%` is bounded by the divisor magnitude.
    /// All inexact endpoints are rounded outward. Custom operators are evaluated at the four
    /// corners and widened by one ulp, which is only an enclosure for monotonic operators.
    pub fn apply_binary(&self, op: &BinaryOp, other: &Interval) -> Result<Interval, AbacusError> {
        let style = if self.style == IntervalStyle::Range || other.style == IntervalStyle::Range {
            IntervalStyle::Range
        } else {
            IntervalStyle::Bracket
        };

        let all_plain =
            is_plain(&self.lo) && is_plain(&self.hi) && is_plain(&other.lo) && is_plain(&other.hi);

        if (arithmetic::is_division(op) || op.alias == "/") && other.contains_zero() {
            return self.divide_by_zero_spanning(other, style);
        }
        if arithmetic::is_power(op) {
            return self.power(op, other, style);
        }
        if arithmetic::is_remainder(op) && !(self.is_degenerate() && other.is_degenerate()) {
            return self.remainder(op, other, style);
        }

        let same_dims = self.lo.unit.dimensions == other.lo.unit.dimensions;
        if all_plain && same_dims && arithmetic::is_addition(op) {
            let unit = op.apply(self.lo.clone(), other.lo.clone())?.unit;
            let (a, b) = (self.lo.canonical, other.lo.canonical);
            let lo = a + b;
            let lo_c = Candidate {
                value: lo,
                error: sum_error(a, b, lo),
            };
            let (a, b) = (self.hi.canonical, other.hi.canonical);
            let hi = a + b;
            let hi_c = Candidate {
                value: hi,
                error: sum_error(a, b, hi),
            };
            let (lo, hi) = enclose(&[lo_c, hi_c]);
            return Ok(Self::from_bounds(lo, hi, &unit, style));
        }
        if all_plain && same_dims && arithmetic::is_subtraction(op) {
            let unit = op.apply(self.lo.clone(), other.hi.clone())?.unit;
            let (a, b) = (self.lo.canonical, -other.hi.canonical);
            let lo = a + b;
            let lo_c = Candidate {
                value: lo,
                error: sum_error(a, b, lo),
            };
            let (a, b) = (self.hi.canonical, -other.lo.canonical);
            let hi = a + b;
            let hi_c = Candidate {
                value: hi,
                error: sum_error(a, b, hi),
            };
            let (lo, hi) = enclose(&[lo_c, hi_c]);
            return Ok(Self::from_bounds(lo, hi, &unit, style));
        }

        let pairs = [
            (&self.lo, &other.lo),
            (&self.lo, &other.hi),
            (&self.hi, &other.lo),
            (&self.hi, &other.hi),
        ];
        let mut unit = None;
        let mut candidates = [Candidate::exact(0.0); 4];
        let is_mul = all_plain && arithmetic::is_multiplication(op);
        let is_div = all_plain && arithmetic::is_division(op);
        for (slot, (l, r)) in candidates.iter_mut().zip(pairs) {
            let result = op.apply(l.clone(), r.clone())?;
            let (a, b, v) = (l.canonical, r.canonical, result.canonical);
            *slot = if is_mul && v == a * b {
                Candidate {
                    value: v,
                    error: product_error(a, b, v),
                }
            } else if is_div && v == a / b {
                Candidate {
                    value: v,
                    error: quotient_error(a, b, v),
                }
            } else {
                Candidate::unknown(v)
            };
            unit.get_or_insert(result.unit);
        }
        let unit = unit.ok_or(AbacusError::IncompatibleDimensions)?;
        let (lo, hi) = enclose(&candidates);
        Ok(Self::from_bounds(lo, hi, &unit, style))
    }

    /// Division by an interval that contains zero crosses a singularity.
    fn divide_by_zero_spanning(
        &self,
        other: &Interval,
        style: IntervalStyle,
    ) -> Result<Interval, AbacusError> {
        let sample_rhs = if other.hi.canonical > 0.0 {
            other.hi.clone()
        } else if other.lo.canonical < 0.0 {
            other.lo.clone()
        } else {
            Value::new(1.0, Arc::clone(&other.lo.unit))
        };
        let sample_unit = (self.lo.clone() / sample_rhs)?.unit;

        let divisor_is_zero = other.lo.canonical == 0.0 && other.hi.canonical == 0.0;
        let dividend_is_zero = self.lo.canonical == 0.0 && self.hi.canonical == 0.0;

        let (lo_c, hi_c) = if dividend_is_zero {
            if divisor_is_zero {
                (f64::NAN, f64::NAN)
            } else {
                // 0 / x = 0 for every non-zero x in the divisor.
                (0.0, 0.0)
            }
        } else if divisor_is_zero || (other.lo.canonical < 0.0 && other.hi.canonical > 0.0) {
            (f64::NEG_INFINITY, f64::INFINITY)
        } else if other.lo.canonical == 0.0 {
            // Divisor is [0, d] with d > 0.
            if self.lo.canonical >= 0.0 {
                (self.lo.canonical / other.hi.canonical, f64::INFINITY)
            } else if self.hi.canonical <= 0.0 {
                (f64::NEG_INFINITY, self.hi.canonical / other.hi.canonical)
            } else {
                (f64::NEG_INFINITY, f64::INFINITY)
            }
        } else {
            // Divisor is [c, 0] with c < 0.
            if self.lo.canonical >= 0.0 {
                (f64::NEG_INFINITY, self.lo.canonical / other.lo.canonical)
            } else if self.hi.canonical <= 0.0 {
                (self.hi.canonical / other.lo.canonical, f64::INFINITY)
            } else {
                (f64::NEG_INFINITY, f64::INFINITY)
            }
        };

        // Finite bounds are single quotients `x / y` of endpoint values; check their exactness.
        let candidate = |v: f64| {
            if !v.is_finite() {
                return Candidate::exact(v);
            }
            for x in [self.lo.canonical, self.hi.canonical] {
                for y in [other.lo.canonical, other.hi.canonical] {
                    if y != 0.0 && x / y == v {
                        return Candidate {
                            value: v,
                            error: quotient_error(x, y, v),
                        };
                    }
                }
            }
            Candidate::unknown(v)
        };
        let (lo, hi) = enclose(&[candidate(lo_c), candidate(hi_c)]);
        Ok(Self::from_bounds(lo, hi, &sample_unit, style))
    }

    /// `[a, b] ^ [c, d]`, accounting for bases that span zero.
    fn power(
        &self,
        op: &BinaryOp,
        other: &Interval,
        style: IntervalStyle,
    ) -> Result<Interval, AbacusError> {
        if !other.is_degenerate() && !self.lo.unit.is_dimensionless() {
            // A varying exponent would give each endpoint different physical dimensions.
            return Err(AbacusError::IncompatibleDimensions);
        }
        let unit = op.apply(self.lo.clone(), other.lo.clone())?.unit;
        let (a, b) = (self.lo.canonical, self.hi.canonical);
        let (c, d) = (other.lo.canonical, other.hi.canonical);

        let spans_zero = a < 0.0 && b > 0.0;
        if spans_zero {
            let exps = if other.is_degenerate() {
                vec![c]
            } else {
                vec![c, d]
            };
            // Negative odd or fractional exponents of values near zero diverge in both directions.
            let diverges_both_ways = (!other.is_degenerate() && c < 0.0)
                || exps
                    .iter()
                    .any(|&e| e < 0.0 && (e.fract() != 0.0 || e % 2.0 != 0.0));
            if diverges_both_ways {
                return Ok(Self::from_bounds(
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    &unit,
                    style,
                ));
            }
        }

        let mut candidates = Vec::with_capacity(6);
        for &x in &[a, b] {
            for &y in &[c, d] {
                let v = x.powf(y);
                candidates.push(if is_exact_power(x, y, v) {
                    Candidate::exact(v)
                } else {
                    Candidate::unknown(v)
                });
            }
        }
        if spans_zero {
            for &y in &[c, d] {
                candidates.push(Candidate::exact(0.0f64.powf(y)));
            }
        }
        let (lo, hi) = enclose(&candidates);
        Ok(Self::from_bounds(lo, hi, &unit, style))
    }

    /// `[a, b] % [c, d]`: any remainder is bounded in magnitude by the largest divisor.
    fn remainder(
        &self,
        op: &BinaryOp,
        other: &Interval,
        style: IntervalStyle,
    ) -> Result<Interval, AbacusError> {
        let unit = op.apply(self.lo.clone(), other.hi.clone())?.unit;
        if unit.dimensions != other.lo.unit.dimensions {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let m = other.lo.canonical.abs().max(other.hi.canonical.abs());
        let (a, b) = (self.lo.canonical, self.hi.canonical);
        let (lo, hi) = if a >= 0.0 && other.lo.canonical > 0.0 {
            if b < other.lo.canonical {
                (a, b)
            } else {
                (0.0, b.min(m))
            }
        } else {
            (-m, m)
        };
        Ok(Self::from_bounds(lo, hi, &unit, style))
    }

    /// Apply a unary operator to an interval by evaluating both endpoints and normalizing.
    ///
    /// This is an enclosure only for monotonic operators, which all built-in unary operators
    /// (`-`, `sqrt`, `!`, `++`, `--`) are. Non-integer results other than negation are
    /// rounded outward by one ulp.
    pub fn apply_unary(&self, op: &UnaryOp) -> Result<Interval, AbacusError> {
        let a = op.apply(self.lo.clone())?;
        let b = op.apply(self.hi.clone())?;
        let (mut lo, mut hi) = if a.canonical <= b.canonical {
            (a, b)
        } else {
            (b, a)
        };
        if op.alias != "-" {
            if lo.canonical.is_finite() && lo.canonical.fract() != 0.0 {
                lo.canonical = lo.canonical.next_down();
            }
            if hi.canonical.is_finite() && hi.canonical.fract() != 0.0 {
                hi.canonical = hi.canonical.next_up();
            }
        }
        Ok(Interval {
            lo,
            hi,
            style: self.style,
        })
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

    fn render_with(&self, f: impl Fn(&Value) -> String) -> String {
        match self.style {
            IntervalStyle::Bracket => format!("[{}, {}]", f(&self.lo), f(&self.hi)),
            IntervalStyle::Range => format!("{}..{}", f(&self.lo), f(&self.hi)),
        }
    }

    /// Formats the interval with both endpoints in scientific notation.
    #[must_use]
    pub fn to_display_scientific(&self) -> String {
        self.render_with(Value::to_display_scientific)
    }

    /// Formats the interval with both endpoints in engineering notation.
    #[must_use]
    pub fn to_display_engineering(&self) -> String {
        self.render_with(Value::to_display_engineering)
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
            // Bounds print exactly so one-ulp outward rounding stays visible.
            IntervalStyle::Bracket => write!(
                f,
                "[{}, {}]",
                ExactDisplay(&self.lo),
                ExactDisplay(&self.hi)
            ),
            IntervalStyle::Range => {
                write!(f, "{}..{}", ExactDisplay(&self.lo), ExactDisplay(&self.hi))
            }
        }
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        self.lo == other.lo && self.hi == other.hi
    }
}
