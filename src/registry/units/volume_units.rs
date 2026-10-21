use std::{collections::HashMap, sync::Arc};

use crate::{
    registry::{
        helpers::{UnitDefinition, register_unit_definitions},
        units::metric_units::register_metric_prefixed_units,
    },
    units::{dimensions::Dimensions, unit::Unit},
};

const VOLUME_AND_AREA_UNITS: &[UnitDefinition] = &[
    // Metric Liter (1 L = 0.001 m^3)
    UnitDefinition {
        keys: &["liter", "liters", "litre", "litres", "L"],
        display: "L",
        scalar: 1e-3,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    // Land Area: Hectare (1 ha = 10,000 m^2)
    UnitDefinition {
        keys: &["hectare", "ha"],
        display: "ha",
        scalar: 10_000.0,
        offset: 0.0,
        dimensions: Dimensions::AREA,
    },
    // Are (1 are = 100 m^2)
    UnitDefinition {
        keys: &["are", "ares"],
        display: "a",
        scalar: 100.0,
        offset: 0.0,
        dimensions: Dimensions::AREA,
    },
    // Cubic centimetre (1 cc = 1 mL)
    UnitDefinition {
        keys: &["cc"],
        display: "cc",
        scalar: 1e-6,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    // US Customary liquid measures (the default meaning of gal/qt/pt/gi/fl oz).
    // Imperial measures are available as imp_gal, imp_qt, imp_pt, imp_gi, imp_fl_oz.
    UnitDefinition {
        keys: &[
            "gallon",
            "gallons",
            "gal",
            "us_gallon",
            "us_gallons",
            "us_gal",
            "us gallon",
            "us gallons",
        ],
        display: "gal",
        scalar: 0.003_785_411_784,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["quart", "quarts", "qt", "us_quart", "us_quarts", "us_qt"],
        display: "qt",
        scalar: 0.000_946_352_946,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["pint", "pints", "pt", "us_pint", "us_pints", "us_pt"],
        display: "pt",
        scalar: 0.000_473_176_473,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["gill", "gills", "gi", "us_gill", "us_gills", "us_gi"],
        display: "gi",
        scalar: 0.000_118_294_118_25,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &[
            "fluid ounce",
            "fluid ounces",
            "fl oz",
            "floz",
            "us_fluid_ounce",
            "us_fluid_ounces",
            "us_fl_oz",
        ],
        display: "fl oz",
        scalar: 0.000_029_573_529_562_5,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["cup", "cups"],
        display: "cup",
        scalar: 0.000_236_588_236_5,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["tablespoon", "tablespoons", "tbsp"],
        display: "tbsp",
        scalar: 0.000_014_786_764_781_25,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
    UnitDefinition {
        keys: &["teaspoon", "teaspoons", "tsp"],
        display: "tsp",
        scalar: 0.000_004_928_921_593_75,
        offset: 0.0,
        dimensions: Dimensions::VOLUME,
    },
];

pub fn register_volume_and_area_units(map: &mut HashMap<String, Arc<Unit>>) {
    register_unit_definitions(map, VOLUME_AND_AREA_UNITS);

    // Prefixed liters (mL, cL, dL, kL, etc.)
    register_metric_prefixed_units(
        map,
        Some("liter"),
        "L",
        1e-3,
        Dimensions::VOLUME,
        true,
        &["litre"],
    );
}
