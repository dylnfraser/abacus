use std::{collections::HashMap, sync::Arc};

use crate::{
    registry::{
        helpers::storage_prefixes::BINARY_STORAGE_PREFIXES, units::metric_units::METRIC_PREFIXES,
    },
    units::{
        dimensions::Dimensions,
        unit::{Unit, UnitExpr},
    },
};

const STORAGE_BASE_UNITS: &[(&str, &str, f64)] = &[("bit", "b", 1.0), ("byte", "B", 8.0)];

fn insert_unit(map: &mut HashMap<String, Arc<Unit>>, name: String, alias: String, scalar: f64) {
    let unit = Arc::new(Unit {
        scalar,
        offset: 0.0,
        dimensions: Dimensions::INFORMATION,
        display: UnitExpr::single(alias.clone()),
    });

    map.insert(name, Arc::clone(&unit));
    map.insert(alias, unit);
}

pub fn register_storage_units(map: &mut HashMap<String, Arc<Unit>>) {
    let decimal_storage_prefixes = &METRIC_PREFIXES[0..10];

    for &(name, alias, base_scalar) in STORAGE_BASE_UNITS {
        insert_unit(map, name.to_string(), alias.to_string(), base_scalar);

        for prefix in decimal_storage_prefixes
            .iter()
            .chain(BINARY_STORAGE_PREFIXES)
        {
            insert_unit(
                map,
                format!("{}{}", prefix.name, name),
                format!("{}{}", prefix.alias, alias),
                prefix.scalar * base_scalar,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_bits_and_bytes() {
        let mut units = HashMap::new();
        register_storage_units(&mut units);

        assert_eq!(units.get("b").unwrap().scalar, 1.0);
        assert_eq!(units.get("B").unwrap().scalar, 8.0);
        assert_eq!(units.get("B").unwrap().dimensions, Dimensions::INFORMATION);
        assert!(Arc::ptr_eq(
            units.get("byte").unwrap(),
            units.get("B").unwrap()
        ));
    }

    #[test]
    fn registers_decimal_storage_units() {
        let mut units = HashMap::new();
        register_storage_units(&mut units);

        assert_eq!(units.get("Mb").unwrap().scalar, 1_000_000.0);
        assert_eq!(units.get("MB").unwrap().scalar, 8_000_000.0);
        assert!(Arc::ptr_eq(
            units.get("megabyte").unwrap(),
            units.get("MB").unwrap()
        ));
    }

    #[test]
    fn registers_binary_storage_units() {
        let mut units = HashMap::new();
        register_storage_units(&mut units);

        assert_eq!(units.get("Mib").unwrap().scalar, 1_048_576.0);
        assert_eq!(units.get("MiB").unwrap().scalar, 8_388_608.0);
        assert!(Arc::ptr_eq(
            units.get("mebibyte").unwrap(),
            units.get("MiB").unwrap()
        ));
    }
}
