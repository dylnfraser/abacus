use crate::units::dimensions::Dimensions;

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
        let mut numerator = self.numerator.clone();
        numerator.extend(rhs.numerator.iter().cloned());

        let mut denominator = self.denominator.clone();
        denominator.extend(rhs.denominator.iter().cloned());

        Self {
            numerator,
            denominator,
        }
    }

    #[must_use]
    pub fn divide(&self, rhs: &Self) -> Self {
        let mut numerator = self.numerator.clone();
        numerator.extend(rhs.denominator.iter().cloned());

        let mut denominator = self.denominator.clone();
        denominator.extend(rhs.numerator.iter().cloned());

        Self {
            numerator,
            denominator,
        }
    }

    #[must_use]
    pub fn simplified(mut self) -> Self {
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
        render_units(&self.numerator)
    }

    #[must_use]
    pub fn render_denominator(&self) -> String {
        render_units(&self.denominator)
    }
}

impl std::fmt::Display for UnitExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.numerator.is_empty(), self.denominator.is_empty()) {
            (true, true) => Ok(()),
            (false, true) => write_units(f, &self.numerator),
            (true, false) => {
                write!(f, "1/")?;
                write_units(f, &self.denominator)
            }
            (false, false) => {
                write_units(f, &self.numerator)?;
                write!(f, "/")?;
                write_units(f, &self.denominator)
            }
        }
    }
}

fn write_units(f: &mut std::fmt::Formatter<'_>, units: &[String]) -> std::fmt::Result {
    if units.is_empty() {
        return Ok(());
    }

    let mut counts = std::collections::HashMap::with_capacity(units.len());
    let mut order = Vec::with_capacity(units.len());

    for unit in units {
        let entry = counts.entry(unit.as_str()).or_insert(0usize);
        if *entry == 0 {
            order.push(unit.as_str());
        }
        *entry += 1;
    }

    for (i, unit) in order.into_iter().enumerate() {
        if i > 0 {
            write!(f, "*")?;
        }
        let count = counts[unit];
        if count == 1 {
            write!(f, "{unit}")?;
        } else {
            write!(f, "{unit}^{count}")?;
        }
    }
    Ok(())
}

fn render_units(units: &[String]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    if units.is_empty() {
        return s;
    }

    let mut counts = std::collections::HashMap::with_capacity(units.len());
    let mut order = Vec::with_capacity(units.len());

    for unit in units {
        let entry = counts.entry(unit.as_str()).or_insert(0usize);
        if *entry == 0 {
            order.push(unit.as_str());
        }
        *entry += 1;
    }

    for (i, unit) in order.into_iter().enumerate() {
        if i > 0 {
            let _ = write!(s, "*");
        }
        let count = counts[unit];
        if count == 1 {
            let _ = write!(s, "{unit}");
        } else {
            let _ = write!(s, "{unit}^{count}");
        }
    }
    s
}

use std::sync::Arc;

/// Largest scale ratio between two units that may be merged for display. Beyond this,
/// converting into the leading unit produces amounts that round to zero or show
/// spurious precision (e.g. `au*cm`), so the units are left as written.
const MAX_CONSOLIDATION_RATIO: f64 = 1e6;

type UnitLookup = Option<Arc<Unit>>;

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

/// Rewrites every unit in `symbols` that shares a dimension with an earlier unit into
/// that earlier unit, returning the factor by which the combined scalar grows.
fn consolidate_same_dimension(symbols: &mut [String], units: &mut [UnitLookup]) -> f64 {
    let mut factor = 1.0;
    for i in 0..symbols.len() {
        let Some(target) = units[i].clone() else {
            continue;
        };
        if !is_consolidatable(&symbols[i], &target) {
            continue;
        }
        for j in (i + 1)..symbols.len() {
            if symbols[j] == symbols[i] {
                continue;
            }
            let Some(other) = &units[j] else {
                continue;
            };
            if other.dimensions != target.dimensions || !is_consolidatable(&symbols[j], other) {
                continue;
            }
            let ratio = target.scalar / other.scalar;
            if !(1.0 / MAX_CONSOLIDATION_RATIO..=MAX_CONSOLIDATION_RATIO).contains(&ratio) {
                continue;
            }
            factor *= ratio;
            symbols[j] = symbols[i].clone();
            units[j] = Some(Arc::clone(&target));
        }
    }
    factor
}

impl Unit {
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
        let mut display = self.display.clone().simplified();
        display.numerator.retain(|unit| unit != "dimensionless");
        display.denominator.retain(|unit| unit != "dimensionless");

        let mut num_units: Vec<UnitLookup> =
            display.numerator.iter().map(|sym| lookup(sym)).collect();
        let mut den_units: Vec<UnitLookup> =
            display.denominator.iter().map(|sym| lookup(sym)).collect();

        // Cancel numerator/denominator pairs that share a dimension.
        let mut num_idx = 0;
        while num_idx < display.numerator.len() {
            let matched = num_units[num_idx].as_ref().and_then(|un1| {
                den_units
                    .iter()
                    .position(|un2| {
                        un2.as_ref().is_some_and(|un2| {
                            un1.dimensions == un2.dimensions
                                && un1.scalar.is_finite()
                                && un1.scalar != 0.0
                                && un2.scalar.is_finite()
                        })
                    })
                    .map(|den_idx| (den_idx, un1.scalar))
            });
            if let Some((den_idx, num_scalar)) = matched {
                let den_scalar = den_units[den_idx].as_ref().map_or(1.0, |u| u.scalar);
                scalar *= den_scalar / num_scalar;
                display.numerator.remove(num_idx);
                num_units.remove(num_idx);
                display.denominator.remove(den_idx);
                den_units.remove(den_idx);
            } else {
                num_idx += 1;
            }
        }

        if consolidate {
            scalar *= consolidate_same_dimension(&mut display.numerator, &mut num_units);
            scalar /= consolidate_same_dimension(&mut display.denominator, &mut den_units);
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
}
