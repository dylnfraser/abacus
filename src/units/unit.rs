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
                self.numerator.swap_remove(numerator_index);
                self.denominator.swap_remove(denominator_index);
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

impl Unit {
    pub fn simplify_display_with(&self, lookup: impl Fn(&str) -> Option<Arc<Unit>>) -> Unit {
        let un = self;
        let mut scalar = un.scalar;
        let mut display = un.display.clone().simplified();

        if self
            .display
            .numerator
            .iter()
            .all(|unit| unit == "dimensionless")
        {
            display = UnitExpr::dimensionless();
        }

        let mut num_idx = 0;
        while num_idx < display.numerator.len() {
            let num_sym = &display.numerator[num_idx];
            if let Some(un1) = lookup(num_sym) {
                let mut matched = false;
                for den_idx in 0..display.denominator.len() {
                    let den_sym = &display.denominator[den_idx];
                    if let Some(un2) = lookup(den_sym)
                        && un1.dimensions == un2.dimensions
                    {
                        scalar *= un2.scalar / un1.scalar;
                        display.numerator.swap_remove(num_idx);
                        display.denominator.swap_remove(den_idx);
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    num_idx += 1;
                }
            } else {
                num_idx += 1;
            }
        }

        // Consolidate same-dimension units within numerator to the leading unit
        let mut i = 0;
        while i < display.numerator.len() {
            let target_sym = display.numerator[i].clone();
            if let Some(target_un) = lookup(&target_sym)
                && !target_un.dimensions.is_dimensionless()
            {
                let mut j = i + 1;
                while j < display.numerator.len() {
                    let other_sym = &display.numerator[j];
                    if other_sym != &target_sym
                        && let Some(other_un) = lookup(other_sym)
                        && target_un.dimensions == other_un.dimensions
                        && other_un.scalar != 0.0
                        && other_un.scalar.is_finite()
                        && target_un.scalar.is_finite()
                    {
                        scalar *= target_un.scalar / other_un.scalar;
                        display.numerator[j] = target_sym.clone();
                    }
                    j += 1;
                }
            }
            i += 1;
        }

        // Consolidate same-dimension units within denominator to the leading unit
        let mut i = 0;
        while i < display.denominator.len() {
            let target_sym = display.denominator[i].clone();
            if let Some(target_un) = lookup(&target_sym)
                && !target_un.dimensions.is_dimensionless()
            {
                let mut j = i + 1;
                while j < display.denominator.len() {
                    let other_sym = &display.denominator[j];
                    if other_sym != &target_sym
                        && let Some(other_un) = lookup(other_sym)
                        && target_un.dimensions == other_un.dimensions
                        && other_un.scalar != 0.0
                        && other_un.scalar.is_finite()
                        && target_un.scalar.is_finite()
                    {
                        scalar /= target_un.scalar / other_un.scalar;
                        display.denominator[j] = target_sym.clone();
                    }
                    j += 1;
                }
            }
            i += 1;
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
