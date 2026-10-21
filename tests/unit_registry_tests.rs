use abacus::{AbacusError, UnitRegistry};
use std::sync::Arc;

#[test]
fn test_standard_unit_registry_lookups() {
    // Verifies that the standard UnitRegistry contains base metric, imperial, and derived units
    let registry = UnitRegistry::standard();

    assert!(registry.contains("m"));
    assert!(registry.contains("km"));
    assert!(registry.contains("h"));
    assert!(registry.contains("hour"));
    assert!(registry.contains("kg"));
    assert!(registry.contains("N"));
    assert!(registry.contains("J"));
    assert!(registry.contains("W"));

    // Scalars are relative to canonical SI units (with the gram as the mass base).
    assert_eq!(registry.unit("km").unwrap().scalar, 1000.0);
    assert_eq!(registry.unit("hour").unwrap().scalar, 3600.0);
    assert_eq!(registry.unit("kg").unwrap().scalar, 1000.0);
    assert_eq!(registry.unit("N").unwrap().scalar, 1000.0);
    assert!(Arc::ptr_eq(
        &registry.unit("h").unwrap(),
        &registry.unit("hour").unwrap()
    ));
}

#[test]
fn test_unit_lookup_returns_arc() {
    // Verifies memory efficiency: repeated unit lookups return shared Arc pointers
    let registry = UnitRegistry::standard();
    let unit1 = registry.unit("km").unwrap();
    let unit2 = registry.unit("km").unwrap();

    assert!(Arc::ptr_eq(&unit1, &unit2));
    assert_eq!(unit1.display.render(), "km");
}

#[test]
fn test_unknown_unit_lookup_fails() {
    // Verifies returning AbacusError::UnknownUnit for unregistered unit symbols
    let registry = UnitRegistry::standard();
    let res = registry.unit("invalid_unit");

    assert_eq!(
        res.unwrap_err(),
        AbacusError::UnknownUnit("invalid_unit".to_string())
    );
}

#[test]
fn test_custom_unit_insertion() {
    // Tests registering new custom units into a fresh or existing UnitRegistry
    let mut registry = UnitRegistry::new();
    assert!(!registry.contains("custom_meter"));

    let base_meter = UnitRegistry::standard().unit("m").unwrap();
    registry.insert_unit("custom_meter", base_meter.clone());

    assert!(registry.contains("custom_meter"));
    let retrieved = registry.unit("custom_meter").unwrap();
    assert_eq!(retrieved.display.render(), "m");
}

#[test]
fn test_exponent_units_resolution() {
    // Tests dynamic on-the-fly parsing and caching of exponent units (e.g. m^2, m^3)
    let registry = UnitRegistry::standard();

    let m2 = registry.unit("m^2").unwrap();
    assert_eq!(m2.display.render(), "m^2");
    assert_eq!(m2.dimensions, abacus::Dimensions::AREA);
    assert_eq!(m2.scalar, 1.0);

    let m3 = registry.unit("m^3").unwrap();
    assert_eq!(m3.display.render(), "m^3");
    assert_eq!(m3.dimensions, abacus::Dimensions::VOLUME);

    let km2 = registry.unit("km^2").unwrap();
    assert_eq!(km2.scalar, 1e6);
}
