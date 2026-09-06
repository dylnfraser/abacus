use std::{collections::HashMap, sync::Arc};

use crate::{
    gen_prefixes,
    registry::{
        helpers::scalar_prefixes::ScalarPrefix,
        units::{
            angle_units::register_angle_units, astronomical_units::register_astronomical_units,
            cgs_physics_units::register_cgs_physics_units,
            computing_niche_units::register_computing_niche_units,
            derived_units::register_derived_units, humorous_units::register_humorous_units,
            imperial_units::register_imperial_units, nautical_units::register_nautical_units,
            speed_units::register_speed_units, storage_units::register_storage_units,
            temporal_units::register_temporal_units,
            trade_historical_units::register_trade_historical_units,
            typography_units::register_typography_units,
            volume_units::register_volume_and_area_units,
        },
    },
    units::{
        dimensions::Dimensions,
        unit::{Unit, UnitExpr},
    },
};

#[cfg(feature = "currencies")]
use crate::registry::units::currency_units::register_currency_units;
#[cfg(feature = "number-scales")]
use crate::registry::units::number_scale_units::register_number_scale_units;

pub static METRIC_PREFIXES: &[ScalarPrefix] = gen_prefixes! {
    "quetta",  "Q",  1e30;
    "ronna",   "R",  1e27;
    "yotta",   "Y",  1e24;
    "zetta",   "Z",  1e21;
    "exa",     "E",  1e18;
    "peta",    "P",  1e15;
    "tera",    "T",  1e12;
    "giga",    "G",   1e9;
    "mega",    "M",   1e6;
    "kilo",    "k",   1e3;
    "hecto",   "h",   1e2;
    "deca",    "da",  1e1;
    "deci",    "d",  1e-1;
    "centi",   "c",  1e-2;
    "milli",   "m",  1e-3;
    "micro",   "μ",  1e-6;
    "nano",    "n",  1e-9;
    "pico",    "p",  1e-12;
    "femto",   "f",  1e-15;
    "atto",    "a",  1e-18;
    "zepto",   "z",  1e-21;
    "yocto",   "y",  1e-24;
    "ronto",   "r",  1e-27;
    "quecto" , "q",  1e-30;
};

macro_rules! generate_base_units {
    ( $( $name:expr, $alias:expr, $quant:expr, $base:expr );* $(;)? ) => {
        &[
            $(
                MetricBaseUnit {
                    name: $name,
                    alias: $alias,
                    dim: $quant,
                    base: $base,
                }
            ),*
        ]
    };
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct MetricBaseUnit {
    pub name: &'static str,
    pub alias: &'static str,
    pub dim: Dimensions,
    pub base: f64,
}

pub static BASE_METRIC_UNITS: &[MetricBaseUnit] = generate_base_units! {
    // SI Base Units
    "second",  "s",    Dimensions::TIME,                 1f64;
    "meter",   "m",    Dimensions::LENGTH,               1f64;
    "gram",    "g",    Dimensions::MASS,                 1f64;
    "ampere",  "A",    Dimensions::CURRENT,              1f64;
    "kelvin",  "K",    Dimensions::TEMPERATURE,          1f64;
    "mole",    "mol",  Dimensions::AMOUNT,               1f64;
    "candela", "cd",   Dimensions::LUMINOUS_INTENSITY,   1f64;
};

const CELSIUS_ALIASES: &[&str] = &["celsius", "degC", "°C"];

// Declares a public function that returns a map of base and prefixed metric units.
#[must_use]
pub fn register_metric_units() -> HashMap<String, Arc<Unit>> {
    let mut map: HashMap<String, Arc<Unit>> = HashMap::new();

    let celsius = Arc::new(Unit {
        scalar: 1.0,
        offset: 273.15,
        dimensions: Dimensions::TEMPERATURE,
        display: UnitExpr::single("°C"),
    });

    for &alias in CELSIUS_ALIASES {
        map.insert(alias.to_string(), Arc::clone(&celsius));
    }

    let percent = Arc::new(Unit {
        scalar: 0.01,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
        display: UnitExpr::single("%"),
    });

    map.insert("%".to_string(), Arc::clone(&percent));
    map.insert("percent".to_string(), Arc::clone(&percent));
    map.insert("pct".to_string(), Arc::clone(&percent));

    for &(tz_name, _) in crate::TimeZone::SUPPORTED_TIMEZONES {
        let tz_unit = Arc::new(Unit {
            scalar: 1.0,
            offset: 0.0,
            dimensions: Dimensions::TIME,
            display: UnitExpr::single(tz_name),
        });
        map.insert(tz_name.to_string(), tz_unit);
    }

    register_temporal_units(&mut map);
    register_storage_units(&mut map);
    register_imperial_units(&mut map);
    register_derived_units(&mut map);
    register_speed_units(&mut map);
    register_volume_and_area_units(&mut map);
    register_angle_units(&mut map);
    register_astronomical_units(&mut map);
    register_nautical_units(&mut map);
    register_cgs_physics_units(&mut map);
    register_typography_units(&mut map);
    register_computing_niche_units(&mut map);
    register_trade_historical_units(&mut map);
    register_humorous_units(&mut map);
    #[cfg(feature = "number-scales")]
    register_number_scale_units(&mut map);
    #[cfg(feature = "currencies")]
    register_currency_units(&mut map);

    for base in BASE_METRIC_UNITS {
        let base_unit = Arc::new(Unit {
            scalar: 1.0f64,
            offset: 0.0,
            dimensions: base.dim,
            display: UnitExpr::single(base.alias),
        });

        map.insert(base.alias.to_string(), Arc::clone(&base_unit));
        map.insert(base.name.to_string(), Arc::clone(&base_unit));
        map.insert(format!("{}s", base.name), Arc::clone(&base_unit));
        if base.name == "meter" {
            map.insert("metre".to_string(), Arc::clone(&base_unit));
            map.insert("metres".to_string(), Arc::clone(&base_unit));
        }

        let extra_names = if base.name == "meter" {
            &["metre"][..]
        } else {
            &[][..]
        };
        register_metric_prefixed_units(
            &mut map,
            Some(base.name),
            base.alias,
            1.0,
            base.dim,
            true,
            extra_names,
        );
    }

    map
}

/// Helper function to register metric prefixed units across modules.
pub fn register_metric_prefixed_units(
    map: &mut HashMap<String, Arc<Unit>>,
    base_name: Option<&str>,
    base_alias: &str,
    base_scalar: f64,
    dimensions: Dimensions,
    add_plural: bool,
    extra_names: &[&str],
) {
    for pref in METRIC_PREFIXES {
        let pref_unit = Arc::new(Unit {
            scalar: base_scalar * pref.scalar,
            offset: 0.0,
            dimensions,
            display: UnitExpr::single(format!("{}{}", pref.alias, base_alias)),
        });

        map.insert(
            format!("{}{}", pref.alias, base_alias),
            Arc::clone(&pref_unit),
        );

        if let Some(name) = base_name {
            map.insert(format!("{}{}", pref.name, name), Arc::clone(&pref_unit));
            if add_plural && !name.ends_with('s') {
                map.insert(format!("{}{}s", pref.name, name), Arc::clone(&pref_unit));
            }
        }

        for extra in extra_names {
            map.insert(format!("{}{}", pref.name, extra), Arc::clone(&pref_unit));
            if add_plural && !extra.ends_with('s') {
                map.insert(format!("{}{}s", pref.name, extra), Arc::clone(&pref_unit));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_fixed_duration_temporal_units() {
        let units = register_metric_units();
        let expected = [
            ("min", "minute", 60.0),
            ("h", "hour", 3_600.0),
            ("d", "day", 86_400.0),
            ("wk", "week", 604_800.0),
        ];

        for (alias, name, scalar) in expected {
            let unit = units.get(alias).unwrap();

            assert_eq!(unit.scalar, scalar);
            assert_eq!(unit.dimensions, Dimensions::TIME);
            assert_eq!(unit.display.render(), alias);
            assert!(Arc::ptr_eq(unit, units.get(name).unwrap()));
        }
    }

    #[test]
    fn registers_seconds_and_prefixed_seconds() {
        let units = register_metric_units();

        assert_eq!(units.get("s").unwrap().scalar, 1.0);
        assert_eq!(units.get("ms").unwrap().scalar, 1e-3);
        assert_eq!(units.get("μs").unwrap().scalar, 1e-6);
    }

    #[test]
    fn includes_storage_units() {
        let units = register_metric_units();

        assert_eq!(units.get("B").unwrap().scalar, 8.0);
        assert_eq!(units.get("GB").unwrap().scalar, 8e9);
        assert_eq!(units.get("GiB").unwrap().scalar, 8_589_934_592.0);
    }

    #[test]
    fn registers_metric_full_name_and_plural_aliases() {
        let units = register_metric_units();

        assert_eq!(units.get("meter").unwrap().scalar, 1.0);
        assert_eq!(units.get("meters").unwrap().scalar, 1.0);
        assert_eq!(units.get("kilometer").unwrap().scalar, 1000.0);
        assert_eq!(units.get("kilometers").unwrap().scalar, 1000.0);
        assert_eq!(units.get("centimeter").unwrap().scalar, 0.01);
        assert_eq!(units.get("centimeters").unwrap().scalar, 0.01);
        assert_eq!(units.get("millimeter").unwrap().scalar, 0.001);
        assert_eq!(units.get("millimeters").unwrap().scalar, 0.001);
        assert_eq!(units.get("gram").unwrap().scalar, 1.0);
        assert_eq!(units.get("grams").unwrap().scalar, 1.0);
        assert_eq!(units.get("kilogram").unwrap().scalar, 1000.0);
        assert_eq!(units.get("kilograms").unwrap().scalar, 1000.0);
        assert_eq!(units.get("liter").unwrap().scalar, 0.001);
        assert_eq!(units.get("liters").unwrap().scalar, 0.001);
        assert_eq!(units.get("milliliter").unwrap().scalar, 1e-6);
        assert_eq!(units.get("milliliters").unwrap().scalar, 1e-6);
    }

    #[test]
    fn registers_celsius() {
        let units = register_metric_units();
        let celsius = units.get("°C").unwrap();

        assert_eq!(celsius.scalar, 1.0);
        assert_eq!(celsius.offset, 273.15);
        assert_eq!(celsius.dimensions, Dimensions::TEMPERATURE);
        assert_eq!(celsius.display.render(), "°C");
        assert!(Arc::ptr_eq(celsius, units.get("celsius").unwrap()));
        assert!(Arc::ptr_eq(celsius, units.get("degC").unwrap()));
    }

    #[test]
    fn converts_celsius_to_kelvin_and_fahrenheit() {
        use crate::units::value::Value;

        let units = register_metric_units();
        let celsius = Value::new(100.0, Arc::clone(units.get("°C").unwrap()));
        let kelvin = celsius
            .convert_to(Arc::clone(units.get("K").unwrap()))
            .unwrap();
        let fahrenheit = celsius
            .convert_to(Arc::clone(units.get("°F").unwrap()))
            .unwrap();

        assert_eq!(celsius.canonical, 373.15);
        assert_eq!(kelvin.to_display(), "373.15 K");
        assert_eq!(fahrenheit.to_display(), "212 °F");
    }

    #[test]
    fn rejects_unsupported_celsius_arithmetic() {
        use crate::units::value::Value;

        let units = register_metric_units();
        let lhs = Value::new(10.0, Arc::clone(units.get("°C").unwrap()));
        let rhs = Value::new(5.0, Arc::clone(units.get("°C").unwrap()));

        assert!((lhs - rhs).is_err());
    }

    #[test]
    fn storage_units_work_with_value_arithmetic() {
        use crate::units::value::Value;

        let units = register_metric_units();
        let decimal = Value::new(1.0, Arc::clone(units.get("MB").unwrap()));
        let binary = Value::new(1.0, Arc::clone(units.get("MiB").unwrap()));
        let total = (decimal + binary).unwrap();

        assert_eq!(total.to_display(), "2.048576 MB");

        let data = Value::new(1.0, Arc::clone(units.get("MiB").unwrap()));
        let duration = Value::new(1.0, Arc::clone(units.get("s").unwrap()));
        let rate = (data / duration).unwrap();

        assert_eq!(rate.to_display(), "1 MiB/s");
        assert_eq!(
            rate.unit.dimensions,
            Dimensions::INFORMATION - Dimensions::TIME
        );
    }
}
