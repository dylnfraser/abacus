use std::{collections::HashMap, sync::Arc};

use crate::{Dimensions, Unit, units::unit::UnitExpr};

const BUSINESS_DAY_UNITS: &[(&str, &str)] = &[
    ("business day", "business_days"),
    ("business days", "business_days"),
    ("business_day", "business_days"),
    ("business_days", "business_days"),
    ("bday", "bdays"),
    ("bdays", "bdays"),
    ("workday", "workdays"),
    ("workdays", "workdays"),
    ("work day", "workdays"),
    ("work days", "workdays"),
    ("work_day", "workdays"),
    ("work_days", "workdays"),
    ("working day", "workdays"),
    ("working days", "workdays"),
    ("working_day", "workdays"),
    ("working_days", "workdays"),
];

const TEMPORAL_UNITS: &[(&str, &str, f64)] = &[
    ("minute", "min", 60.0),
    ("hour", "h", 60.0 * 60.0),
    ("day", "d", 24.0 * 60.0 * 60.0),
    ("week", "wk", 7.0 * 24.0 * 60.0 * 60.0),
];

pub fn register_temporal_units(map: &mut HashMap<String, Arc<Unit>>) {
    for &(alias, display) in BUSINESS_DAY_UNITS {
        let bday_unit = Arc::new(Unit {
            scalar: 86400.0,
            offset: 0.0,
            dimensions: Dimensions::TIME,
            display: UnitExpr::single(display),
        });
        map.insert(alias.to_string(), bday_unit);
    }

    for &(name, alias, scalar) in TEMPORAL_UNITS {
        let unit = Arc::new(Unit {
            scalar,
            offset: 0.0,
            dimensions: Dimensions::TIME,
            display: UnitExpr::single(alias),
        });

        map.insert(name.to_string(), Arc::clone(&unit));
        map.insert(format!("{name}s"), Arc::clone(&unit));
        map.insert(alias.to_string(), unit);
    }
}

/// Whether `symbol` is a business-day unit display symbol. These follow calendar rules
/// rather than a fixed conversion, so they must not be merged with other time units.
pub(crate) fn is_business_day_symbol(symbol: &str) -> bool {
    BUSINESS_DAY_UNITS
        .iter()
        .any(|&(alias, display)| alias == symbol || display == symbol)
}
