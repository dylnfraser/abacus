use abacus::{Abacus, IntervalStyle};

#[test]
fn test_single_unit_display_override_mph() {
    let calc = Abacus::standard().with_unit_display_override("mi/h", "mph");

    // Division with 'per'
    let speed1 = calc.eval("60 miles per hour").unwrap();
    assert_eq!(speed1.to_display(), "60 mph");

    // Explicit division
    let speed2 = calc.eval("60 mi / 1 h").unwrap();
    assert_eq!(speed2.to_display(), "60 mph");

    // Raw slash
    let speed3 = calc.eval("60 mi/h").unwrap();
    assert_eq!(speed3.to_display(), "60 mph");
}

#[test]
fn test_single_unit_display_override_kmph() {
    let calc = Abacus::standard().with_unit_display_override("km/h", "kmph");

    let speed1 = calc.eval("100 km / 1 h").unwrap();
    assert_eq!(speed1.to_display(), "100 kmph");

    let speed2 = calc.eval("100 km/h").unwrap();
    assert_eq!(speed2.to_display(), "100 kmph");

    let speed3 = calc.eval("100 kilometers per hour").unwrap();
    assert_eq!(speed3.to_display(), "100 kmph");
}

#[test]
fn test_common_speed_overrides() {
    let calc = Abacus::standard().with_common_speed_overrides();

    assert_eq!(
        calc.eval("60 miles per hour").unwrap().to_display(),
        "60 mph"
    );
    assert_eq!(calc.eval("100 km / 1 h").unwrap().to_display(), "100 kmph");
}

#[test]
fn test_interval_unit_display_override() {
    let calc = Abacus::standard()
        .with_common_speed_overrides()
        .with_interval_style(IntervalStyle::Bracket);

    let inv = calc.eval("[50 mi/h, 70 mi/h]").unwrap();
    assert_eq!(inv.to_display(), "[50 mph, 70 mph]");

    let calc_range = Abacus::standard()
        .with_common_speed_overrides()
        .with_interval_style(IntervalStyle::Range);

    let inv_range = calc_range.eval("(50 mi/h) .. (70 mi/h)").unwrap();
    assert_eq!(inv_range.to_display(), "50 mph..70 mph");
}

#[test]
fn test_hash_regression_unit_display_override() {
    let calc = Abacus::standard().with_common_speed_overrides();

    let reg = calc.eval("linreg(1 h, 2 h, 60 mi, 120 mi)").unwrap();
    let slope = reg
        .clone()
        .into_hash()
        .unwrap()
        .get("slope")
        .unwrap()
        .clone();
    assert_eq!(slope.to_display(), "60 mph");
}

#[test]
fn test_currency_rate_display_override() {
    let calc = Abacus::standard().with_unit_display_override("$/d", "$/day");

    let rate = calc.eval("a thousand dollars per day").unwrap();
    assert_eq!(rate.to_display(), "1000 $/day");

    let total = calc.eval("(a thousand dollars per day) * 3 days").unwrap();
    assert_eq!(total.to_display(), "$3000");
}

#[test]
fn test_dimensional_preservation_with_override() {
    let mut calc = Abacus::standard().with_common_speed_overrides();

    let speed = calc.eval("60 miles per hour").unwrap();
    assert_eq!(speed.to_display(), "60 mph");

    calc.set_variable("v", speed);

    // Multiplying speed by time and converting to miles
    let dist = calc.eval("v * 2 h in mi").unwrap();
    assert_eq!(dist.to_display(), "120 mi");

    // Converting speed to m/s should work
    let ms = calc.eval("v in m/s").unwrap();
    assert_eq!(ms.to_display(), "26.8224 m/s");
}

#[test]
fn test_conversion_target_with_override() {
    let calc = Abacus::standard()
        .with_common_speed_overrides()
        .with_decimal_places(2);

    let speed_km = calc.eval("60 miles per hour in km/h").unwrap();
    assert_eq!(speed_km.to_display(), "96.56 kmph");
}

#[test]
fn test_override_management_lifecycle() {
    let mut calc = Abacus::standard();
    assert!(!calc.has_unit_display_override("mi/h"));

    calc.set_unit_display_override("mi/h", "mph");
    assert!(calc.has_unit_display_override("mi/h"));
    assert_eq!(calc.eval("60 mi/h").unwrap().to_display(), "60 mph");

    let removed = calc.remove_unit_display_override("mi/h");
    assert_eq!(removed, Some("mph".to_string()));
    assert!(!calc.has_unit_display_override("mi/h"));
    assert_eq!(calc.eval("60 mi/h").unwrap().to_display(), "60 mi/h");

    calc.enable_common_speed_overrides();
    assert!(calc.has_unit_display_override("km/h"));
    assert_eq!(calc.eval("100 km/h").unwrap().to_display(), "100 kmph");

    calc.clear_unit_display_overrides();
    assert!(!calc.has_unit_display_override("km/h"));
    assert_eq!(calc.eval("100 km/h").unwrap().to_display(), "100 km/h");
}

#[test]
fn test_mph_and_kmph_registered_as_units() {
    let calc = Abacus::standard();

    let v1 = calc.eval("60 mph in m/s").unwrap();
    assert_eq!(v1.to_display(), "26.8224 m/s");

    let v2 = calc.eval("100 kmph in m/s").unwrap();
    assert_eq!(v2.to_display(), "27.77777777777778 m/s");

    let v3 = calc.with_decimal_places(2).eval("60 mph in kmph").unwrap();
    assert_eq!(v3.to_display(), "96.56 kmph");
}
