use crate::units::dimensions::Dimensions;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unit {
    pub dimensions: Dimensions,
    pub scalar: f64,
    pub offset: f64,
    pub display: UnitExpr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnitExpr {
    pub numerator: Vec<String>,
    pub denominator: Vec<String>,
}

/// Integer exponents up to this magnitude are stored as repeated symbols (`m*m*m`),
/// larger or fractional exponents as a single `base^exp` entry.
const MAX_REPEATED_EXPONENT: f64 = 8.0;

/// Parses a unit exponent: a plain number (`2`, `-1.5`, `0.5`) or a parenthesized
/// rational (`(1/3)`, `(-2/7)`). Returns `None` for NaN, infinite or unparseable input.
#[must_use]
pub fn parse_exponent(s: &str) -> Option<f64> {
    if s.is_empty() || s.len() > 32 {
        return None;
    }
    let value = if let Some(inner) = s.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        if let Some((p, q)) = inner.split_once('/') {
            let p: f64 = p.trim().parse().ok()?;
            let q: f64 = q.trim().parse().ok()?;
            if q == 0.0 {
                return None;
            }
            p / q
        } else {
            inner.trim().parse().ok()?
        }
    } else {
        s.parse().ok()?
    };
    value.is_finite().then_some(value)
}

/// Formats an exponent for display: integers plainly, short decimals as-is (`0.5`, `1.25`),
/// and other small-denominator rationals as `(p/q)` (e.g. `(1/3)`).
#[must_use]
pub fn format_exponent(e: f64) -> String {
    let rounded = e.round();
    if (e - rounded).abs() < 1e-9 {
        return format!("{rounded}");
    }
    let millis = e * 1000.0;
    if (millis - millis.round()).abs() < 1e-9 {
        return format!("{}", millis.round() / 1000.0);
    }
    for q in 2..=12_i32 {
        let p = e * f64::from(q);
        if (p - p.round()).abs() < 1e-9 {
            return format!("({}/{q})", p.round());
        }
    }
    format!("{e}")
}

/// Splits `base^exp` into its base symbol and exponent; plain symbols have exponent 1.
fn split_power(sym: &str) -> (&str, f64) {
    if let Some((base, exp)) = sym.rsplit_once('^')
        && !base.is_empty()
        && let Some(e) = parse_exponent(exp)
    {
        return (base, e);
    }
    (sym, 1.0)
}

impl UnitExpr {
    #[must_use]
    pub fn dimensionless() -> Self {
        Self {
            numerator: Vec::new(),
            denominator: Vec::new(),
        }
    }

    pub fn single(unit: impl Into<String>) -> Self {
        Self {
            numerator: vec![unit.into()],
            denominator: Vec::new(),
        }
    }

    #[must_use]
    pub fn multiply(&self, rhs: &Self) -> Self {
        let mut numerator = Vec::with_capacity(self.numerator.len() + rhs.numerator.len());
        numerator.extend_from_slice(&self.numerator);
        numerator.extend_from_slice(&rhs.numerator);

        let mut denominator = Vec::with_capacity(self.denominator.len() + rhs.denominator.len());
        denominator.extend_from_slice(&self.denominator);
        denominator.extend_from_slice(&rhs.denominator);

        Self {
            numerator,
            denominator,
        }
    }

    #[must_use]
    pub fn divide(&self, rhs: &Self) -> Self {
        let mut numerator = Vec::with_capacity(self.numerator.len() + rhs.denominator.len());
        numerator.extend_from_slice(&self.numerator);
        numerator.extend_from_slice(&rhs.denominator);

        let mut denominator = Vec::with_capacity(self.denominator.len() + rhs.numerator.len());
        denominator.extend_from_slice(&self.denominator);
        denominator.extend_from_slice(&rhs.numerator);

        Self {
            numerator,
            denominator,
        }
    }

    /// Returns `true` if this expression is exactly the single symbol `sym`.
    #[must_use]
    pub fn is_single(&self, sym: &str) -> bool {
        self.denominator.is_empty() && self.numerator.len() == 1 && self.numerator[0] == sym
    }

    /// Returns `true` if this expression is a single plain unit symbol (no denominator,
    /// no exponent), e.g. `atm` or `GHz` but not `m^2` or `m/s`.
    #[must_use]
    pub fn is_atomic(&self) -> bool {
        self.denominator.is_empty() && self.numerator.len() == 1 && !self.numerator[0].contains('^')
    }

    /// Collects `(base, exponent)` pairs in order of first appearance, with
    /// numerator entries counted positively and denominator entries negatively.
    fn exponents(&self) -> Vec<(&str, f64)> {
        let mut out: Vec<(&str, f64)> = Vec::new();
        let terms = self
            .numerator
            .iter()
            .map(|s| (s, 1.0))
            .chain(self.denominator.iter().map(|s| (s, -1.0)));
        for (sym, sign) in terms {
            let (base, e) = split_power(sym);
            if let Some(entry) = out.iter_mut().find(|(b, _)| *b == base) {
                entry.1 += sign * e;
            } else {
                out.push((base, sign * e));
            }
        }
        out
    }

    fn from_exponents(exps: &[(&str, f64)]) -> Self {
        let mut expr = Self::dimensionless();
        for &(base, e) in exps {
            if e.abs() < 1e-9 {
                continue;
            }
            let target = if e > 0.0 {
                &mut expr.numerator
            } else {
                &mut expr.denominator
            };
            let mag = e.abs();
            if (mag - mag.round()).abs() < 1e-9 && mag <= MAX_REPEATED_EXPONENT {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                for _ in 0..(mag.round() as usize) {
                    target.push(base.to_string());
                }
            } else {
                target.push(format!("{base}^{}", format_exponent(mag)));
            }
        }
        expr
    }

    /// Raises every unit in this expression to `exp`, e.g. `(m^2/s)^0.5` → `m/s^0.5`.
    #[must_use]
    pub fn powf(&self, exp: f64) -> Self {
        let exps: Vec<(&str, f64)> = self
            .exponents()
            .into_iter()
            .map(|(b, e)| (b, e * exp))
            .collect();
        Self::from_exponents(&exps)
    }

    /// Cancels identical symbols between numerator and denominator and merges
    /// powers of the same base (`m^9*m` → `m^10`, `m^0.5*m^0.5` → `m`).
    #[must_use]
    pub fn simplified(mut self) -> Self {
        if self
            .numerator
            .iter()
            .chain(self.denominator.iter())
            .any(|s| s.contains('^'))
        {
            return Self::from_exponents(&self.exponents());
        }

        let mut numerator_index = 0;

        while numerator_index < self.numerator.len() {
            if let Some(denominator_index) = self
                .denominator
                .iter()
                .position(|unit| unit == &self.numerator[numerator_index])
            {
                self.numerator.remove(numerator_index);
                self.denominator.remove(denominator_index);
            } else {
                numerator_index += 1;
            }
        }

        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.numerator.is_empty() && self.denominator.is_empty()
    }

    #[must_use]
    pub fn render(&self) -> String {
        self.to_string()
    }

    #[must_use]
    pub fn render_numerator(&self) -> String {
        UnitList(&self.numerator).to_string()
    }

    #[must_use]
    pub fn render_denominator(&self) -> String {
        UnitList(&self.denominator).to_string()
    }
}

impl std::fmt::Display for UnitExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.numerator.is_empty(), self.denominator.is_empty()) {
            (true, true) => Ok(()),
            (false, true) => write!(f, "{}", UnitList(&self.numerator)),
            (true, false) => write!(f, "1/{}", UnitList(&self.denominator)),
            (false, false) => write!(
                f,
                "{}/{}",
                UnitList(&self.numerator),
                UnitList(&self.denominator)
            ),
        }
    }
}

/// Renders a list of unit symbols, collapsing repeats into exponents (`m*m` → `m^2`).
struct UnitList<'a>(&'a [String]);

impl std::fmt::Display for UnitList<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let units = self.0;
        // Unit lists are short; a linear scan avoids allocating a HashMap per render.
        for (i, unit) in units.iter().enumerate() {
            if units[..i].contains(unit) {
                continue;
            }
            if i > 0 {
                write!(f, "*")?;
            }
            let count = units[i..].iter().filter(|u| *u == unit).count();
            if count == 1 {
                write!(f, "{unit}")?;
            } else {
                write!(f, "{unit}^{count}")?;
            }
        }
        Ok(())
    }
}

/// Largest scale ratio between two units that may be merged for display. Beyond this,
/// converting into the leading unit produces amounts that round to zero or show
/// spurious precision (e.g. `au*cm`), so the units are left as written.
const MAX_CONSOLIDATION_RATIO: f64 = 1e6;

/// Whether a unit has a fixed, well-defined conversion factor that makes it safe to
/// fold into another unit of the same dimension purely for display.
fn is_consolidatable(sym: &str, unit: &Unit) -> bool {
    !unit.dimensions.is_dimensionless()
        && unit.dimensions != crate::units::dimensions::Dimensions::CURRENCY
        && unit.offset == 0.0
        && unit.scalar.is_finite()
        && unit.scalar > 0.0
        && !crate::registry::units::temporal_units::is_business_day_symbol(sym)
}

impl Unit {
    /// Simplifies this unit's display: cancels same-dimension units between numerator and
    /// denominator (`km/m` → dimensionless), relabels same-dimension units to the leading one
    /// (`m*ft` → `m^2`), and merges powers. The scalar is recomputed from the final display
    /// when every symbol is known, avoiding accumulated rounding noise.
    pub fn simplify_display_with(&self, lookup: impl Fn(&str) -> Option<Arc<Unit>>) -> Unit {
        self.simplify_display_with_options(lookup, true)
    }

    /// Simplifies the display expression. When `consolidate` is false, units of the same
    /// dimension are kept as written (used when the user explicitly requested a unit).
    pub(crate) fn simplify_display_with_options(
        &self,
        lookup: impl Fn(&str) -> Option<Arc<Unit>>,
        consolidate: bool,
    ) -> Unit {
        let mut scalar = self.scalar;
        let mut display = self.display.clone();
        display.numerator.retain(|u| u != "dimensionless");
        display.denominator.retain(|u| u != "dimensionless");
        let mut display = display.simplified();

        // Cancel numerator/denominator pairs that share a dimension, preserving symbol order.
        let mut num_idx = 0;
        while num_idx < display.numerator.len() {
            let matched = lookup(&display.numerator[num_idx]).and_then(|un1| {
                display
                    .denominator
                    .iter()
                    .enumerate()
                    .find_map(|(den_idx, sym)| {
                        let un2 = lookup(sym)?;
                        (un1.dimensions == un2.dimensions
                            && un1.scalar.is_finite()
                            && un1.scalar != 0.0
                            && un2.scalar.is_finite())
                        .then(|| (den_idx, un2.scalar / un1.scalar))
                    })
            });
            if let Some((den_idx, ratio)) = matched {
                scalar *= ratio;
                display.numerator.remove(num_idx);
                display.denominator.remove(den_idx);
            } else {
                num_idx += 1;
            }
        }

        if consolidate {
            consolidate_same_dimension(&mut display.numerator, &lookup, &mut scalar, false);
            consolidate_same_dimension(&mut display.denominator, &lookup, &mut scalar, true);
        }
        let display = display.simplified();

        if let Some(exact) = scalar_from_display(&display, &lookup)
            && exact.is_finite()
            && exact != 0.0
            && relative_close(exact, scalar, 1e-9)
        {
            scalar = exact;
        }

        Unit {
            dimensions: self.dimensions,
            scalar,
            offset: self.offset,
            display,
        }
    }

    #[must_use]
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions
    }

    #[must_use]
    pub fn is_affine(&self) -> bool {
        self.offset != 0.0
    }

    #[must_use]
    pub fn dimensionless() -> Self {
        Self {
            dimensions: Dimensions::DIMENSIONLESS,
            scalar: 1.0,
            offset: 0.0,
            display: UnitExpr::dimensionless(),
        }
    }

    pub fn dimensionless_arc_ref() -> &'static Arc<Self> {
        static DIMENSIONLESS: std::sync::OnceLock<Arc<Unit>> = std::sync::OnceLock::new();
        DIMENSIONLESS.get_or_init(|| Arc::new(Self::dimensionless()))
    }

    #[must_use]
    pub fn dimensionless_arc() -> Arc<Self> {
        Arc::clone(Self::dimensionless_arc_ref())
    }

    #[must_use]
    pub fn is_dimensionless(&self) -> bool {
        self.dimensions == Dimensions::DIMENSIONLESS
    }

    /// Returns `true` for a plain dimensionless unit with no display (a bare number).
    #[must_use]
    pub fn is_plain_number(&self) -> bool {
        self.is_dimensionless() && self.display.is_empty() && self.scalar == 1.0
    }

    #[must_use]
    pub fn is_percent(&self) -> bool {
        self.is_dimensionless()
            && self.display.denominator.is_empty()
            && self.display.numerator.len() == 1
            && matches!(
                self.display.numerator[0].as_str(),
                "%" | "+%" | "percent" | "pct"
            )
    }

    #[must_use]
    pub fn is_business_day_unit(&self) -> bool {
        self.dimensions == Dimensions::TIME
            && self.display.denominator.is_empty()
            && self.display.numerator.len() == 1
            && matches!(
                self.display.numerator[0].as_str(),
                "business_days"
                    | "bdays"
                    | "workdays"
                    | "business_day"
                    | "business day"
                    | "business days"
                    | "workday"
                    | "work day"
                    | "work days"
                    | "work_day"
                    | "work_days"
                    | "working day"
                    | "working days"
                    | "working_day"
                    | "working_days"
                    | "bday"
            )
    }

    #[must_use]
    pub fn is_standard_duration_unit(&self) -> bool {
        self.dimensions == Dimensions::TIME
            && self.display.denominator.is_empty()
            && match self.display.numerator.as_slice() {
                [] => true,
                [s] => matches!(
                    s.as_str(),
                    "s" | "h" | "min" | "d" | "minute" | "hour" | "second" | "day"
                ),
                _ => false,
            }
    }
}

fn relative_close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * a.abs().max(b.abs())
}

/// Relabels units in `list` that share dimensions with an earlier unit to that unit,
/// adjusting `scalar` accordingly. Units without a fixed linear factor (currencies, affine
/// temperatures, business days) and pairs whose scales differ by more than
/// [`MAX_CONSOLIDATION_RATIO`] are left as written.
fn consolidate_same_dimension(
    list: &mut [String],
    lookup: &impl Fn(&str) -> Option<Arc<Unit>>,
    scalar: &mut f64,
    in_denominator: bool,
) {
    for i in 0..list.len() {
        let Some(target_un) = lookup(&list[i]) else {
            continue;
        };
        if !is_consolidatable(&list[i], &target_un) {
            continue;
        }
        for j in (i + 1)..list.len() {
            if list[j] == list[i] {
                continue;
            }
            if let Some(other_un) = lookup(&list[j])
                && target_un.dimensions == other_un.dimensions
                && is_consolidatable(&list[j], &other_un)
                && (1.0 / MAX_CONSOLIDATION_RATIO..=MAX_CONSOLIDATION_RATIO)
                    .contains(&(target_un.scalar / other_un.scalar))
            {
                let ratio = target_un.scalar / other_un.scalar;
                if in_denominator {
                    *scalar /= ratio;
                } else {
                    *scalar *= ratio;
                }
                let target = list[i].clone();
                list[j] = target;
            }
        }
    }
}

/// Computes a unit scalar directly from its display symbols, if all are known.
fn scalar_from_display(
    display: &UnitExpr,
    lookup: &impl Fn(&str) -> Option<Arc<Unit>>,
) -> Option<f64> {
    let mut scalar = 1.0;
    for sym in &display.numerator {
        scalar *= lookup(sym)?.scalar;
    }
    for sym in &display.denominator {
        scalar /= lookup(sym)?.scalar;
    }
    Some(scalar)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_dimensionless_unit_expr() {
        assert_eq!(UnitExpr::dimensionless().render(), "");
    }

    #[test]
    fn renders_repeated_units_as_exponents() {
        let area = UnitExpr::single("m").multiply(&UnitExpr::single("m"));
        let volume = area.multiply(&UnitExpr::single("m"));

        assert_eq!(area.render(), "m^2");
        assert_eq!(volume.render(), "m^3");
    }

    #[test]
    fn renders_denominator_only_expressions() {
        let inverse_seconds = UnitExpr::dimensionless().divide(&UnitExpr::single("s"));

        assert_eq!(inverse_seconds.render(), "1/s");
    }

    #[test]
    fn powers_merge_and_simplify() {
        let m2 = UnitExpr::single("m").multiply(&UnitExpr::single("m"));
        assert_eq!(m2.powf(0.5).render(), "m");
        assert_eq!(UnitExpr::single("m").powf(0.5).render(), "m^0.5");
        assert_eq!(UnitExpr::single("m").powf(1.0 / 3.0).render(), "m^(1/3)");
        let merged = UnitExpr::single("m^9").multiply(&UnitExpr::single("m"));
        assert_eq!(merged.simplified().render(), "m^10");
        let halves = UnitExpr::single("m^0.5").multiply(&UnitExpr::single("m^0.5"));
        assert_eq!(halves.simplified().render(), "m");
    }

    #[test]
    fn parses_exponents() {
        assert_eq!(parse_exponent("2"), Some(2.0));
        assert_eq!(parse_exponent("(1/4)"), Some(0.25));
        assert_eq!(parse_exponent("NaN"), None);
        assert_eq!(parse_exponent("inf"), None);
        assert_eq!(parse_exponent("(1/0)"), None);
    }
}
