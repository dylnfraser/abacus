#[cfg(feature = "currencies")]
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

#[cfg(feature = "units")]
use crate::registry::units::metric_units::register_metric_units;
use crate::{
    error::AbacusError,
    registry::helpers::fx_hash::FxHashMap,
    units::{
        dimensions::{Dimensions, MAX_REPRESENTABLE_EXPONENT},
        unit::{ExponentDisplay, Unit, UnitExpr, parse_exponent},
        value::Value,
    },
};

/// Derived SI units that composite results are automatically reduced to, in priority order.
///
/// Units whose dimensions are ambiguous without tracking angles/steradians are deliberately
/// excluded: `Gy`/`Sv` (= m²/s², also specific energy and velocity²), `Bq` (= 1/s, same as Hz),
/// `lm` (= cd, since sr is dimensionless) and `lx` (= cd/m², also luminance).
const PRIORITY_DERIVED_SYMBOLS: [&str; 15] = [
    "N", "J", "W", "Pa", "Hz", "V", "C", "F", "Ω", "S", "Wb", "T", "H", "A", "kat",
];

/// Maximum number of generated `base^exp` units kept in the lookup cache.
const MAX_CACHED_EXPONENT_UNITS: usize = 4_096;

/// Unit lookups happen several times per token, so the internal maps use the fast Fx hasher.
type UnitMap = FxHashMap<String, Arc<Unit>>;

/// The standard unit table is immutable once built, so every [`UnitRegistry::standard`]
/// shares one copy; mutations go through [`Arc::make_mut`] (copy-on-write).
fn standard_unit_map() -> &'static Arc<UnitMap> {
    static STANDARD: std::sync::OnceLock<Arc<UnitMap>> = std::sync::OnceLock::new();
    STANDARD.get_or_init(|| {
        #[cfg(feature = "units")]
        let units: UnitMap = register_metric_units().into_iter().collect();
        #[cfg(not(feature = "units"))]
        let units = UnitMap::default();
        Arc::new(units)
    })
}

#[derive(Debug)]
pub struct UnitRegistry {
    units: Arc<UnitMap>,
    cache: RwLock<FxHashMap<String, Arc<Unit>>>,
    priority_derived_units: Vec<(Dimensions, Arc<Unit>)>,
    pub max_exponent: f64,
}

impl Default for UnitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            units: Arc::new(UnitMap::default()),
            cache: RwLock::new(FxHashMap::default()),
            priority_derived_units: Vec::new(),
            max_exponent: 1_000.0,
        }
    }

    #[must_use]
    pub fn standard() -> Self {
        let units = Arc::clone(standard_unit_map());
        let priority_derived_units = PRIORITY_DERIVED_SYMBOLS
            .iter()
            .filter_map(|&s| units.get(s).map(|u| (u.dimensions, Arc::clone(u))))
            .collect();
        Self {
            units,
            cache: RwLock::new(FxHashMap::default()),
            priority_derived_units,
            max_exponent: 1_000.0,
        }
    }

    fn clear_cache(&self) {
        self.cache
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// Sets the largest exponent magnitude accepted in `base^exp` unit symbols.
    /// Values beyond the representable range (≈ 77 471) are clamped to it.
    pub fn set_max_exponent(&mut self, max: f64) {
        self.max_exponent = max.min(MAX_REPRESENTABLE_EXPONENT);
        self.clear_cache();
    }

    #[must_use]
    pub fn with_max_exponent(mut self, max: f64) -> Self {
        self.set_max_exponent(max);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn get(&self, symbol: &str) -> Option<Arc<Unit>> {
        if let Some(unit) = self.units.get(symbol) {
            return Some(Arc::clone(unit));
        }
        self.parse_exponent_unit(symbol).ok()
    }

    pub fn contains(&self, symbol: &str) -> bool {
        self.units.contains_key(symbol) || self.parse_exponent_unit(symbol).is_ok()
    }

    pub fn find_unit_by_dimensions(&self, dimensions: &Dimensions) -> Option<Arc<Unit>> {
        self.priority_derived_units
            .iter()
            .find(|(dims, _)| dims == dimensions)
            .map(|(_, u)| Arc::clone(u))
    }

    pub fn unit(&self, symbol: &str) -> Result<Arc<Unit>, AbacusError> {
        if let Some(unit) = self.units.get(symbol) {
            return Ok(Arc::clone(unit));
        }
        self.parse_exponent_unit(symbol)
    }

    /// Resolves `base^exp` symbols (`m^3`, `s^-1`, `m^(1/3)`), caching the result under a
    /// normalized key so `m^1`, `m^1.0` and `m^01` share one entry.
    fn parse_exponent_unit(&self, symbol: &str) -> Result<Arc<Unit>, AbacusError> {
        let unknown = || AbacusError::UnknownUnit(symbol.to_string());
        let (base_sym, exp_str) = symbol.rsplit_once('^').ok_or_else(unknown)?;
        let base_unit = self.units.get(base_sym).ok_or_else(unknown)?;
        let exp = parse_exponent(exp_str).ok_or_else(unknown)?;

        if exp.abs() > self.max_exponent {
            return Err(AbacusError::ExponentLimitExceeded);
        }
        if base_unit.is_affine() {
            if exp == 1.0 {
                return Ok(Arc::clone(base_unit));
            }
            return Err(AbacusError::AffineUnitOperation("exponentiate"));
        }

        // Build the normalized key on the stack; it is only copied to the heap on a cache miss.
        let mut key = crate::units::value::StackStr::new();
        let _ = std::fmt::Write::write_fmt(
            &mut key,
            format_args!("{base_sym}^{}", ExponentDisplay(exp)),
        );
        let key = key.as_str();
        if let Some(cached) = self
            .cache
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
        {
            return Ok(Arc::clone(cached));
        }

        let dimensions = base_unit
            .dimensions
            .checked_mul_f64(exp)
            .ok_or(AbacusError::ExponentLimitExceeded)?;
        let unit = Arc::new(Unit {
            scalar: base_unit.scalar.powf(exp),
            offset: 0.0,
            dimensions,
            display: UnitExpr::single(base_sym).powf(exp),
        });

        let mut guard = self.cache.write().unwrap_or_else(PoisonError::into_inner);
        if guard.len() >= MAX_CACHED_EXPONENT_UNITS {
            guard.clear();
        }
        guard.insert(key.to_string(), Arc::clone(&unit));

        Ok(unit)
    }

    pub fn value(&self, amount: f64, symbol: &str) -> Result<Value, AbacusError> {
        Ok(Value::new(amount, self.unit(symbol)?))
    }

    pub fn insert_unit(&mut self, key: impl Into<String>, unit: Arc<Unit>) {
        let key = key.into();
        if PRIORITY_DERIVED_SYMBOLS.contains(&key.as_str()) {
            if let Some(pos) = self
                .priority_derived_units
                .iter()
                .position(|(_, u)| u.display.is_single(&key))
            {
                self.priority_derived_units[pos] = (unit.dimensions, Arc::clone(&unit));
            } else {
                self.priority_derived_units
                    .push((unit.dimensions, Arc::clone(&unit)));
            }
        }
        Arc::make_mut(&mut self.units).insert(key, unit);
        self.clear_cache();
    }

    #[cfg(feature = "currencies")]
    pub fn update_currency_rates(&mut self, rates: &HashMap<String, f64>) {
        crate::registry::units::currency_units::update_currency_rates_in_map(
            Arc::make_mut(&mut self.units),
            rates,
        );
        self.clear_cache();
    }

    #[cfg(feature = "currencies")]
    pub fn set_currency_rate(&mut self, code: &str, rate_per_usd: f64) {
        let mut map = HashMap::new();
        map.insert(code.to_string(), rate_per_usd);
        self.update_currency_rates(&map);
    }

    #[cfg(feature = "currencies")]
    pub fn set_currencies_enabled(&mut self, enabled: bool) {
        if enabled {
            crate::registry::units::currency_units::register_currency_units(Arc::make_mut(
                &mut self.units,
            ));
        } else {
            Arc::make_mut(&mut self.units).retain(|_, u| u.dimensions != Dimensions::CURRENCY);
            self.priority_derived_units
                .retain(|(dims, _)| *dims != Dimensions::CURRENCY);
        }
        self.clear_cache();
    }

    /// Number of generated exponent units currently cached (exposed for diagnostics).
    #[must_use]
    pub fn cached_exponent_units(&self) -> usize {
        self.cache
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

#[cfg(all(test, feature = "units"))]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_contains_base_metric_units() {
        let registry = UnitRegistry::standard();

        assert!(registry.contains("m"));
        assert!(registry.contains("s"));
        assert!(registry.contains("g"));
        assert!(registry.contains("km"));
    }

    #[test]
    fn standard_registry_contains_derived_si_units() {
        let registry = UnitRegistry::standard();

        assert!(registry.contains("N"));
        assert!(registry.contains("J"));
        assert!(registry.contains("W"));
        assert!(registry.contains("Pa"));
        assert!(registry.contains("Hz"));
        assert!(registry.contains("V"));
        assert!(registry.contains("kHz"));
        assert!(registry.contains("kW"));
    }

    #[test]
    fn standard_registry_contains_volume_and_angle_units() {
        let registry = UnitRegistry::standard();

        assert!(registry.contains("L"));
        assert!(registry.contains("mL"));
        assert!(registry.contains("us_gal"));
        assert!(registry.contains("rad"));
        assert!(registry.contains("deg"));
    }

    #[test]
    fn performs_value_conversions_via_registry() {
        let registry = UnitRegistry::standard();

        let distance = registry.value(5.0, "km").unwrap();
        let meters = distance.to(&registry, "m").unwrap();

        assert_eq!(meters.to_display(), "5000 m");
        assert_eq!(meters.canonical, 5000.0);

        let volume = registry.value(1.0, "L").unwrap();
        let ml = volume.to(&registry, "mL").unwrap();

        assert_eq!(ml.to_display(), "1000 mL");
    }

    #[test]
    fn converts_and_adds_volume_units() {
        let registry = UnitRegistry::standard();

        // 1 oil barrel (bbl) to m^3
        let bbl = registry.value(1.0, "bbl").unwrap();
        let m3 = bbl.to(&registry, "m^3").unwrap();
        assert_eq!(m3.to_display(), "0.158987294928 m^3");

        // 1000 L to m^3
        let liters = registry.value(1000.0, "L").unwrap();
        let liters_m3 = liters.to(&registry, "m^3").unwrap();
        assert_eq!(liters_m3.to_display(), "1 m^3");

        // Addition of bbl and L
        let sum = (bbl + registry.value(100.0, "L").unwrap()).unwrap();
        let sum_m3 = sum.to(&registry, "m^3").unwrap();
        assert_eq!(sum_m3.to_display(), "0.258987294928 m^3");
    }

    #[test]
    fn caches_generated_exponent_units() {
        let registry = UnitRegistry::standard();

        let unit1 = registry.unit("m^3").unwrap();
        let unit2 = registry.unit("m^3").unwrap();

        assert!(Arc::ptr_eq(&unit1, &unit2));
    }
}
