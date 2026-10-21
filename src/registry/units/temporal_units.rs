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

/// Calendar durations. Added to or subtracted from a date they move by calendar months/years
/// (clamping to the month's last day); converted to fixed units they use the average Gregorian
/// lengths (year = 365.2425 d, month = year / 12).
const CALENDAR_UNITS: &[(&[&str], &str, f64)] = &[
    (
        &["month", "months", "mo", "mos"],
        "mo",
        crate::units::date::SECONDS_PER_AVERAGE_MONTH,
    ),
    (
        &["year", "years", "yr", "yrs"],
        "yr",
        crate::units::date::SECONDS_PER_AVERAGE_YEAR,
    ),
];

pub fn register_temporal_units(map: &mut HashMap<String, Arc<Unit>>) {
    for &(aliases, display, scalar) in CALENDAR_UNITS {
        let unit = Arc::new(Unit {
            scalar,
            offset: 0.0,
            dimensions: Dimensions::TIME,
            display: UnitExpr::single(display),
        });
        for &alias in aliases {
            map.insert(alias.to_string(), Arc::clone(&unit));
        }
    }

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
