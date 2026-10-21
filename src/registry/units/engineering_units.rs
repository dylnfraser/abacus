use std::{collections::HashMap, sync::Arc};

use crate::{
    registry::{
        helpers::{UnitDefinition, register_unit_definitions},
        units::metric_units::register_metric_prefixed_units,
    },
    units::{dimensions::Dimensions, unit::Unit},
};

// Canonical mass is the gram, so 1 J = 1000 g·m²/s², 1 W = 1000 g·m²/s³, 1 Pa = 1000 g/(m·s²).
const ENERGY: Dimensions = Dimensions::from_f64([2.0, 1.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
const POWER: Dimensions = Dimensions::from_f64([2.0, 1.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
const PRESSURE: Dimensions = Dimensions::from_f64([-1.0, 1.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
const FORCE: Dimensions = Dimensions::from_f64([1.0, 1.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

/// Watt-hour: 3600 J. Prefixed forms (kWh, MWh, GWh, ...) are generated.
const WATT_HOUR_SCALAR: f64 = 3_600_000.0;

const ENGINEERING_UNITS: &[UnitDefinition] = &[
    // Thermochemical calorie: exactly 4.184 J.
    UnitDefinition {
        keys: &["calorie", "calories", "cal", "cal_th"],
        display: "cal",
        scalar: 4_184.0,
        offset: 0.0,
        dimensions: ENERGY,
    },
    // Kilocalorie / food Calorie (thermochemical): 4184 J.
    UnitDefinition {
        keys: &["kilocalorie", "kilocalories", "kcal", "Cal"],
        display: "kcal",
        scalar: 4_184_000.0,
        offset: 0.0,
        dimensions: ENERGY,
    },
    // International Steam Table calorie: exactly 4.1868 J.
    UnitDefinition {
        keys: &["cal_IT"],
        display: "cal_IT",
        scalar: 4_186.8,
        offset: 0.0,
        dimensions: ENERGY,
    },
    // British thermal unit (International Table): 1055.05585262 J.
    UnitDefinition {
        keys: &["BTU", "Btu", "btu"],
        display: "BTU",
        scalar: 1_055_055.852_62,
        offset: 0.0,
        dimensions: ENERGY,
    },
    // Mechanical (imperial) horsepower: 550 ft·lbf/s = 745.69987158227022 W.
    UnitDefinition {
        keys: &["horsepower", "hp"],
        display: "hp",
        scalar: 745_699.871_582_270_2,
        offset: 0.0,
        dimensions: POWER,
    },
    // Metric horsepower (PS): 75 kgf·m/s = 735.49875 W.
    UnitDefinition {
        // (No `PS` alias: that is peta-siemens.)
        keys: &["hp_metric", "metric_horsepower"],
        display: "hp_metric",
        scalar: 735_498.75,
        offset: 0.0,
        dimensions: POWER,
    },
    // Pound-force per square inch: 6894.757293168 Pa.
    UnitDefinition {
        keys: &["psi"],
        display: "psi",
        scalar: 6_894_757.293_168,
        offset: 0.0,
        dimensions: PRESSURE,
    },
    // Pound-force: 4.4482216152605 N.
    UnitDefinition {
        keys: &["lbf", "poundforce", "pound_force"],
        display: "lbf",
        scalar: 4_448.221_615_260_5,
        offset: 0.0,
        dimensions: FORCE,
    },
    // Watt-hour: 3600 J.
    UnitDefinition {
        keys: &["Wh", "watthour", "watthours", "watt_hour", "watt_hours"],
        display: "Wh",
        scalar: WATT_HOUR_SCALAR,
        offset: 0.0,
        dimensions: ENERGY,
    },
];

pub fn register_engineering_units(map: &mut HashMap<String, Arc<Unit>>) {
    register_unit_definitions(map, ENGINEERING_UNITS);

    // kWh, MWh, GWh, mWh, ... and kilowatthour(s), megawatthour(s), ...
    register_metric_prefixed_units(
        map,
        Some("watthour"),
        "Wh",
        WATT_HOUR_SCALAR,
        ENERGY,
        true,
        &[],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_engineering_units() {
        let mut units = HashMap::new();
        register_engineering_units(&mut units);

        assert_eq!(units.get("cal").unwrap().scalar, 4_184.0);
        assert_eq!(units.get("Cal").unwrap().scalar, 4_184_000.0);
        assert_eq!(units.get("kWh").unwrap().scalar, 3.6e9);
        assert_eq!(units.get("kilowatthours").unwrap().scalar, 3.6e9);
        assert_eq!(units.get("MWh").unwrap().dimensions, ENERGY);
        assert_eq!(units.get("psi").unwrap().dimensions, PRESSURE);
    }
}
