use std::{collections::HashMap, sync::Arc};

use crate::{
    registry::helpers::{UnitDefinition, register_unit_definitions},
    units::{dimensions::Dimensions, unit::Unit},
};

const NUMBER_SCALE_UNITS: &[UnitDefinition] = &[
    // Hundred (10^2)
    UnitDefinition {
        keys: &["hundred", "hundreds"],
        display: "hundred",
        scalar: 100.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Thousand (10^3)
    UnitDefinition {
        keys: &["thousand", "thousands"],
        display: "thousand",
        scalar: 1_000.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Million (10^6)
    UnitDefinition {
        keys: &["million", "millions"],
        display: "million",
        scalar: 1_000_000.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Billion (10^9 - short scale)
    UnitDefinition {
        keys: &["billion", "billions"],
        display: "billion",
        scalar: 1_000_000_000.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Trillion (10^12)
    UnitDefinition {
        keys: &["trillion", "trillions"],
        display: "trillion",
        scalar: 1_000_000_000_000.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Quadrillion (10^15)
    UnitDefinition {
        keys: &["quadrillion", "quadrillions"],
        display: "quadrillion",
        scalar: 1e15,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Quintillion (10^18)
    UnitDefinition {
        keys: &["quintillion", "quintillions"],
        display: "quintillion",
        scalar: 1e18,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Sextillion (10^21)
    UnitDefinition {
        keys: &["sextillion", "sextillions"],
        display: "sextillion",
        scalar: 1e21,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Septillion (10^24)
    UnitDefinition {
        keys: &["septillion", "septillions"],
        display: "septillion",
        scalar: 1e24,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Googol (10^100)
    UnitDefinition {
        keys: &["googol", "googols"],
        display: "googol",
        scalar: 1e100,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Dozen (12)
    UnitDefinition {
        keys: &["dozen", "dozens"],
        display: "dozen",
        scalar: 12.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Baker's Dozen (13)
    UnitDefinition {
        keys: &["bakers_dozen", "bakers_dozens", "baker_dozen"],
        display: "bakers_dozen",
        scalar: 13.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Gross (144)
    UnitDefinition {
        keys: &["gross", "grosses"],
        display: "gross",
        scalar: 144.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
    // Myriad (10,000)
    UnitDefinition {
        keys: &["myriad", "myriads"],
        display: "myriad",
        scalar: 10_000.0,
        offset: 0.0,
        dimensions: Dimensions::DIMENSIONLESS,
    },
];

pub fn register_number_scale_units(map: &mut HashMap<String, Arc<Unit>>) {
    register_unit_definitions(map, NUMBER_SCALE_UNITS);
}
