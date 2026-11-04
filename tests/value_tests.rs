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
    let registry = UnitRegistry::standard();
    let meter = registry.unit("m").unwrap();

    let val = Value::new(10.5, meter);
    assert_eq!(val.canonical, 10.5);
    assert_eq!(val.to_display(), "10.5 m");
}

#[test]
fn test_dimensionless_value() {
    let val = Value::new(42.0, Arc::new(Unit::dimensionless()));
    assert_eq!(val.canonical, 42.0);
    assert_eq!(val.to_display(), "42");
    assert!(val.unit.dimensions.is_dimensionless());
}

#[cfg(feature = "units")]
#[test]
fn test_value_addition_subtraction() {
    // Operands in different units of the same dimension.
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
    // Length / time gives a speed in km/h.
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
    let registry = UnitRegistry::standard();
    let km = registry.unit("km").unwrap();
    let m = registry.unit("m").unwrap();

    let dist = Value::new(3.5, km);
    let converted = dist.convert_to(m).unwrap();

    assert_eq!(converted.to_display(), "3500 m");
    assert_eq!(converted.canonical, 3500.0);
}

// Exact arithmetic and equality

#[cfg(feature = "units")]
#[test]
fn value_arithmetic_is_exact_and_the_engine_snaps_by_default() {
    use abacus::Abacus;
    let calc = Abacus::standard();
    let a = calc.eval_scalar("1.0000000000000002").unwrap();
    let b = calc.eval_scalar("1").unwrap();
    // `Value` subtraction is plain IEEE arithmetic ...
    assert_eq!((&a - &b).unwrap().canonical, f64::EPSILON);
    // ... while the engine reports cancellation noise as 0 unless asked not to.
    assert_eq!(
        calc.eval("1.0000000000000002 - 1").unwrap().to_display(),
        "0"
    );
    assert_eq!(calc.eval("0.1 + 0.2 - 0.3").unwrap().to_display(), "0");
    let exact = Abacus::standard().with_snap_cancellation(false);
    assert_eq!(
        exact
            .eval_scalar("1.0000000000000002 - 1")
            .unwrap()
            .canonical,
        f64::EPSILON
    );
    assert_eq!(
        exact.eval_scalar("0.1 + 0.2 - 0.3").unwrap().canonical,
        0.1 + 0.2 - 0.3
    );
}

#[cfg(feature = "units")]
#[test]
fn value_equality_is_exact_and_transitive() {
    use abacus::Abacus;
    let calc = Abacus::standard();
    let v = |s: &str| calc.eval_scalar(s).unwrap();
    // Same quantity in different units.
    assert_eq!(v("5 km"), v("5000 m"));
    // No tolerance: these differ in the last bit.
    let x = v("0.30000000000000004 m");
    let y = v("0.3 m");
    assert_ne!(x, y);
    assert!(x.approx_eq(&y, 1e-12));
    assert!(!x.approx_eq(&v("0.3 s"), 1e-12));
}
