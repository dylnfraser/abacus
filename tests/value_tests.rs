// Helpers and imports are only used by tests gated on default features.
#![cfg_attr(
    not(all(
        feature = "units",
        feature = "stats",
        feature = "distributions",
        feature = "date",
        feature = "financial",
        feature = "number-scales",
        feature = "currencies"
    )),
    allow(unused)
)]
use abacus::{Dimensions, Unit, UnitRegistry, Value};
use std::sync::Arc;

#[cfg(feature = "units")]
#[test]
fn test_value_creation_and_display() {
    // Tests manual construction of Value structs from numerical amounts and Unit Arcs
    let registry = UnitRegistry::standard();
    let meter = registry.unit("m").unwrap();

    let val = Value::new(10.5, meter);
    assert_eq!(val.canonical, 10.5);
    assert_eq!(val.to_display(), "10.5 m");
}

#[test]
fn test_dimensionless_value() {
    // Tests constructing dimensionless Value structs with Unit::dimensionless()
    let val = Value::new(42.0, Arc::new(Unit::dimensionless()));
    assert_eq!(val.canonical, 42.0);
    assert_eq!(val.to_display(), "42");
    assert!(val.unit.dimensions.is_dimensionless());
}

#[cfg(feature = "units")]
#[test]
fn test_value_addition_subtraction() {
    // Tests direct Value arithmetic (+ and -) across compatible unit scales (e.g. km and m)
    let registry = UnitRegistry::standard();
    let km = registry.unit("km").unwrap();
    let m = registry.unit("m").unwrap();

    let v1 = Value::new(2.0, km); // 2000 m
    let v2 = Value::new(500.0, m); // 500 m

    let sum = (&v1 + &v2).unwrap();
    assert_eq!(sum.canonical, 2500.0);
    assert_eq!(sum.to_display(), "2.5 km");

    let diff = (&v1 - &v2).unwrap();
    assert_eq!(diff.canonical, 1500.0);
    assert_eq!(diff.to_display(), "1.5 km");
}

#[cfg(feature = "units")]
#[test]
fn test_value_multiplication_division() {
    // Tests Value division across different physical dimensions (Length / Time -> Speed in km/h)
    let registry = UnitRegistry::standard();
    let km = registry.unit("km").unwrap();
    let h = registry.unit("h").unwrap();

    let distance = Value::new(100.0, km);
    let time = Value::new(2.0, h);

    let speed = (&distance / &time).unwrap();
    assert_eq!(speed.to_display(), "50 km/h");
    assert_eq!(speed.unit.dimensions, Dimensions::LENGTH - Dimensions::TIME);
}

#[cfg(feature = "units")]
#[test]
fn test_value_unit_conversion() {
    // Tests programmatic unit conversion via value.convert_to(target_unit)
    let registry = UnitRegistry::standard();
    let km = registry.unit("km").unwrap();
    let m = registry.unit("m").unwrap();

    let dist = Value::new(3.5, km);
    let converted = dist.convert_to(m).unwrap();

    assert_eq!(converted.to_display(), "3500 m");
    assert_eq!(converted.canonical, 3500.0);
}
