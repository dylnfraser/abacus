//! Regression tests for the unit-data / currency / tooling audit fixes.
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

use abacus::{Abacus, AbacusError, UnitRegistry, eval};

/// Evaluates `expr` to a scalar and returns `(amount in its display unit, display unit)`.
fn amount(expr: &str) -> (f64, String) {
    let v = eval(expr)
        .unwrap_or_else(|e| panic!("`{expr}` failed: {e:?}"))
        .into_scalar()
        .unwrap();
    (v.amount(), v.unit.display.render())
}

#[track_caller]
fn assert_amount(expr: &str, expected: f64, unit: &str) {
    let (got, got_unit) = amount(expr);
    assert_eq!(got_unit, unit, "unit of `{expr}`");
    let tol = 1e-12 * expected.abs().max(1e-300);
    assert!(
        (got - expected).abs() <= tol,
        "`{expr}`: got {got}, expected {expected}"
    );
}

// ---------------------------------------------------------------------------------------------
// pound means mass, not GBP
// ---------------------------------------------------------------------------------------------

#[cfg(all(feature = "units", feature = "currencies"))]
#[test]
fn pound_is_mass() {
    assert_amount("1 pound", 1.0, "lb");
    assert_amount("5 pounds to kg", 5.0 * 0.453_592_37, "kg");
    assert_amount("1 pound in g", 453.592_37, "g");
    assert_amount("1 lb + 1 pound", 2.0, "lb");
    assert_amount("10 lbs in kg", 4.535_923_7, "kg");
    // GBP remains reachable.
    assert_eq!(eval("£5").unwrap().to_display(), "5 GBP");
    assert_eq!(eval("5 quid").unwrap().to_display(), "5 GBP");
    assert_eq!(eval("5 pounds sterling").unwrap().to_display(), "5 GBP");
    assert!(matches!(
        eval("1 pound + 1 GBP"),
        Err(AbacusError::IncompatibleDimensions)
    ));
}

// ---------------------------------------------------------------------------------------------
// US customary vs imperial volumes, tons
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "units")]
#[test]
fn volumes_default_to_us_customary() {
    assert_amount("1 gal in L", 3.785_411_784, "L");
    assert_amount("1 gallon in mL", 3_785.411_784, "mL");
    assert_amount("1 qt in mL", 946.352_946, "mL");
    assert_amount("1 pt in mL", 473.176_473, "mL");
    assert_amount("1 gi in mL", 118.294_118_25, "mL");
    assert_amount("1 fl oz in mL", 29.573_529_562_5, "mL");
    assert_amount("2 fluid ounces in mL", 59.147_059_125, "mL");
    assert_amount("1 cup in fl oz", 8.0, "fl oz");
    assert_amount("1 gal in qt", 4.0, "qt");
    assert_amount("1 gal in floz", 128.0, "fl oz");
    // us_* aliases are kept.
    assert_amount("1 us_gal in gal", 1.0, "gal");
}

#[cfg(feature = "units")]
#[test]
fn imperial_volumes_need_explicit_names() {
    assert_amount("1 imp_gal in L", 4.546_09, "L");
    assert_amount("1 imperial gallon in L", 4.546_09, "L");
    assert_amount("1 imp_pt in mL", 568.261_25, "mL");
    assert_amount("1 imp_qt in imp_pt", 2.0, "imp_pt");
    assert_amount("1 imp_fl_oz in mL", 28.413_062_5, "mL");
    assert_amount("1 imp_gal in imp_fl_oz", 160.0, "imp_fl_oz");
}

#[cfg(feature = "units")]
#[test]
fn ton_variants() {
    assert_amount("1 ton to lb", 2_000.0, "lb");
    assert_amount("1 short ton to lb", 2_000.0, "lb");
    assert_amount("1 long ton to lb", 2_240.0, "lb");
    assert_amount("1 long_ton to lb", 2_240.0, "lb");
    assert_amount("1 imperial ton to lb", 2_240.0, "lb");
    assert_amount("1 t in kg", 1_000.0, "kg");
    assert_amount("2 tonnes in kg", 2_000.0, "kg");
    assert_amount("1 metric ton in kg", 1_000.0, "kg");
    // `kt` stays the knot and `ft` the foot (tonnes are not prefixable).
    assert_amount("1 kt in m/s", 1_852.0 / 3_600.0, "m/s");
    assert_amount("1 ft in m", 0.3048, "m");
}

// ---------------------------------------------------------------------------------------------
// Newly added units
// ---------------------------------------------------------------------------------------------

#[cfg(feature = "units")]
#[test]
fn energy_power_pressure_force_units() {
    assert_amount("1 cal in J", 4.184, "J");
    assert_amount("1 kcal in J", 4_184.0, "J");
    assert_amount("1 Cal in kcal", 1.0, "kcal");
    assert_amount("1 cal_IT in J", 4.1868, "J");
    assert_amount("1 BTU in J", 1_055.055_852_62, "J");
    assert_amount("1 hp in W", 745.699_871_582_270_2, "W");
    assert_amount("1 hp_metric in W", 735.498_75, "W");
    assert_amount("1 psi in Pa", 6_894.757_293_168, "Pa");
    assert_amount("1 lbf in N", 4.448_221_615_260_5, "N");
    assert_amount("1 Wh in J", 3_600.0, "J");
    assert_amount("1 kWh in J", 3.6e6, "J");
    assert_amount("1 MWh in kWh", 1_000.0, "kWh");
    assert_amount("2 kilowatthours in Wh", 2_000.0, "Wh");
    assert_amount("1 cc in mL", 1.0, "mL");
}

#[cfg(feature = "units")]
#[test]
fn micro_sign_alias() {
    // U+00B5 MICRO SIGN (keyboard) and U+03BC GREEK SMALL LETTER MU are both accepted.
    assert_amount("1 \u{b5}m in m", 1e-6, "m");
    assert_amount("1000 \u{3bc}m in mm", 1.0, "mm");
    assert_amount("2 \u{b5}s in \u{3bc}s", 2.0, "\u{3bc}s");
}

#[cfg(feature = "units")]
#[test]
fn uppercase_kb_alias_is_decimal() {
    assert_amount("1 KB in B", 1_000.0, "B");
    assert_amount("1 KiB in B", 1_024.0, "B");
}

#[cfg(feature = "units")]
#[test]
fn as_is_always_the_conversion_keyword() {
    // `as` is not registered as the attosecond.
    assert!(!UnitRegistry::standard().contains("as"));
    assert_amount("5 km as m", 5_000.0, "m");
    let attos = eval("1 attosecond").unwrap().into_scalar().unwrap();
    assert_eq!(attos.canonical, 1e-18);
}

#[cfg(feature = "units")]
#[test]
fn mmhg_is_distinct_from_torr() {
    assert_amount("1 mmHg in Pa", 133.322_387_415, "Pa");
    assert_amount("760 torr in Pa", 101_325.0, "Pa");
    let (ratio, _) = amount("1 mmHg in torr");
    assert!((ratio - 1.000_000_142_466_321).abs() < 1e-12);
}

#[cfg(feature = "units")]
#[test]
fn dalton_codata_2022() {
    let da = eval("1 Da").unwrap().into_scalar().unwrap();
    assert_eq!(da.canonical, 1.660_539_068_92e-24);
}

#[cfg(feature = "units")]
#[test]
fn typographic_point_display() {
    assert_eq!(eval("1 pica to point").unwrap().to_display(), "12 point");
    assert_amount("72 points in in", 1.0, "in");
    assert_amount("1 typ_pt in point", 1.0, "point");
}

#[cfg(feature = "units")]
#[test]
fn multi_word_units_are_reachable() {
    assert_amount("3 fl oz in fl oz", 3.0, "fl oz");
    assert_amount("1 fluid ounce in mL", 29.573_529_562_5, "mL");
    assert_amount("1 long tons in lb", 2_240.0, "lb");
    assert_amount("1 Long Ton in lb", 2_240.0, "lb");
    assert_eq!(
        eval("5 business days").unwrap().to_display(),
        "5 business_days"
    );
    assert_eq!(
        eval("5 Business Days").unwrap().to_display(),
        "5 business_days"
    );
}

#[cfg(all(feature = "units", feature = "currencies"))]
#[test]
fn brazilian_real_symbol() {
    assert_eq!(eval("R$5").unwrap().to_display(), "5 BRL");
    assert_eq!(eval("R$ 5").unwrap().to_display(), "5 BRL");
    assert_eq!(eval("5 R$").unwrap().to_display(), "5 BRL");
}

#[cfg(all(feature = "units", feature = "currencies"))]
mod currency {
    use super::*;
    use abacus::registry::units::currency_units::{
        FALLBACK_RATES_DATE, FIXED_EXCHANGE_RATES, is_using_fallback_rates, load_rates_from_cache,
        parse_frankfurter_json, save_rates_to_cache,
    };

    // ---------------------------------------------------------------------------------------------
    // Currency: parsing, validation, caching
    // ---------------------------------------------------------------------------------------------

    struct TempPath(std::path::PathBuf);

    impl TempPath {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            Self(std::env::temp_dir().join(format!(
                "abacus_audit_{tag}_{}_{nanos}.json",
                std::process::id()
            )))
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn standard_engine_never_writes_a_cache_implicitly() {
        let mut calc = Abacus::standard();
        assert!(calc.currency_cache_path.is_none());
        calc.update_rates_from_json(r#"{"base":"USD","rates":{"EUR":0.5}}"#)
            .unwrap();
        assert_eq!(calc.eval("100 USD in EUR").unwrap().to_display(), "50 EUR");
    }

    #[test]
    fn fallback_rates_are_detectable() {
        assert_eq!(FALLBACK_RATES_DATE.len(), 10);
        let mut calc = Abacus::standard();
        assert!(is_using_fallback_rates(&calc.units));
        calc.set_currency_rate("EUR", 0.5);
        assert!(!is_using_fallback_rates(&calc.units));
    }

    #[test]
    fn fallback_rates_are_plausible() {
        for def in FIXED_EXCHANGE_RATES {
            assert!(
                def.rate_per_usd.is_finite() && def.rate_per_usd > 0.0,
                "{}",
                def.code
            );
        }
        assert!(
            FIXED_EXCHANGE_RATES
                .iter()
                .all(|d| !d.aliases.contains(&"pound") && !d.aliases.contains(&"pounds"))
        );
    }

    #[test]
    fn rejects_invalid_rate_responses() {
        for bad in [
            r#"{"base":"EUR","rates":{"USD":1.1}}"#,
            r#"{"rates":{"EUR":-0.9}}"#,
            r#"{"rates":{"EUR":0}}"#,
            r#"{"rates":{"EUR":1e999}}"#,
            r#"{"rates":{"EUR":"x"}}"#,
            "not json",
            r#"{"rates":{"EUR":0.9}"#,
        ] {
            assert!(parse_frankfurter_json(bad).is_err(), "accepted {bad}");
            let mut calc = Abacus::standard();
            assert!(calc.update_rates_from_json(bad).is_err(), "accepted {bad}");
            assert!(is_using_fallback_rates(&calc.units));
        }
    }

    #[test]
    fn stale_dated_cache_is_not_fresh() {
        // A cache written just now but containing an old ECB date (e.g. leftover test data)
        // must not be treated as fresh.
        let tmp = TempPath::new("stale");
        save_rates_to_cache(
            r#"{"amount":1.0,"base":"USD","date":"2026-09-01","rates":{"EUR":0.5}}"#,
            &tmp.0,
        )
        .unwrap();
        assert!(tmp.0.exists());
        assert!(!abacus::is_currency_cache_fresh(&tmp.0));
        // It is still loadable as a stale fallback.
        assert_eq!(load_rates_from_cache(&tmp.0).unwrap()["EUR"], 0.5);
    }

    #[test]
    fn cache_writes_are_validated_and_atomic() {
        let tmp = TempPath::new("atomic");
        assert!(save_rates_to_cache("garbage", &tmp.0).is_err());
        assert!(!tmp.0.exists());

        save_rates_to_cache(r#"{"rates":{"EUR":0.5}}"#, &tmp.0).unwrap();
        save_rates_to_cache(r#"{"rates":{"EUR":0.6}}"#, &tmp.0).unwrap();
        assert_eq!(load_rates_from_cache(&tmp.0).unwrap()["EUR"], 0.6);

        // No temporary files are left behind next to the cache.
        let dir = tmp.0.parent().unwrap();
        let stem = tmp.0.file_name().unwrap().to_string_lossy().to_string();
        let leftovers = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(&format!(".{stem}")) && name.ends_with(".tmp")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cache_write_replaces_symlink_instead_of_following_it() {
        let target = TempPath::new("symlink_target");
        let link = TempPath::new("symlink_link");
        std::fs::write(&target.0, "precious").unwrap();
        std::os::unix::fs::symlink(&target.0, &link.0).unwrap();

        save_rates_to_cache(r#"{"rates":{"EUR":0.5}}"#, &link.0).unwrap();

        assert_eq!(std::fs::read_to_string(&target.0).unwrap(), "precious");
        assert!(
            !std::fs::symlink_metadata(&link.0)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}

#[cfg(feature = "units")]
#[test]
fn informal_time_abbreviations() {
    assert_eq!(eval("2 hr in min").unwrap().to_display(), "120 min");
    assert_eq!(eval("3 hrs").unwrap().to_display(), "3 h");
    assert_eq!(eval("90 sec in min").unwrap().to_display(), "1.5 min");
    assert_eq!(eval("5 secs").unwrap().to_display(), "5 s");
}
