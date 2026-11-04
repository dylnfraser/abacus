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
use abacus::{Abacus, AbacusError, AngleMode, IntervalStyle, Notation, TimeZone, WeekendDays};

#[cfg(feature = "units")]
#[test]
fn test_angle_mode_degrees() {
    let calc = Abacus::standard().with_angle_mode(AngleMode::Degrees);

    let sin_90 = calc.eval("sin(90)").unwrap();
    assert!((sin_90.into_scalar().unwrap().canonical - 1.0).abs() < 1e-10);

    let cos_180 = calc.eval("cos(180)").unwrap();
    assert!((cos_180.into_scalar().unwrap().canonical - (-1.0)).abs() < 1e-10);

    let tan_45 = calc.eval("tan(45)").unwrap();
    assert!((tan_45.into_scalar().unwrap().canonical - 1.0).abs() < 1e-10);

    // Inverse trig returns degrees.
    let asin_1 = calc.eval("asin(1)").unwrap();
    assert_eq!(asin_1.to_display(), "90 deg");

    let acos_0 = calc.eval("acos(0)").unwrap();
    assert_eq!(acos_0.to_display(), "90 deg");

    let atan_1 = calc.eval("atan(1)").unwrap();
    assert_eq!(atan_1.to_display(), "45 deg");

    // An explicit angle unit overrides the angle mode.
    let sin_explicit = calc.eval("sin(90 deg)").unwrap();
    assert!((sin_explicit.into_scalar().unwrap().canonical - 1.0).abs() < 1e-10);
}

#[cfg(feature = "units")]
#[test]
fn test_angle_mode_gradians() {
    let calc = Abacus::standard().with_angle_mode(AngleMode::Gradians);

    let sin_100 = calc.eval("sin(100)").unwrap();
    assert!((sin_100.into_scalar().unwrap().canonical - 1.0).abs() < 1e-10);

    let cos_200 = calc.eval("cos(200)").unwrap();
    assert!((cos_200.into_scalar().unwrap().canonical - (-1.0)).abs() < 1e-10);

    let tan_50 = calc.eval("tan(50)").unwrap();
    assert!((tan_50.into_scalar().unwrap().canonical - 1.0).abs() < 1e-10);

    let asin_1 = calc.eval("asin(1)").unwrap();
    assert_eq!(asin_1.to_display(), "100 grad");
}

#[test]
fn test_angle_mode_radians_default() {
    let calc = Abacus::standard();
    assert_eq!(calc.angle_mode(), AngleMode::Radians);

    let asin_1 = calc.eval("asin(1)").unwrap();
    assert!(asin_1.to_display().contains("rad"));
}

#[cfg(feature = "units")]
#[test]
fn test_strict_dimension_checking() {
    let loose = Abacus::standard();
    assert!(!loose.strict_dimensions());

    // Loose mode gives a bare number the other operand's unit.
    let promoted = loose.eval("5 m + 5").unwrap();
    assert_eq!(promoted.to_display(), "10 m");

    let promoted_interval = loose.eval("[1 m, 2]").unwrap();
    assert_eq!(promoted_interval.to_display(), "[1 m, 2 m]");

    // Strict mode rejects mixed dimensions.
    let strict = Abacus::standard().with_strict_dimensions(true);
    assert!(strict.strict_dimensions());

    assert!(matches!(
        strict.eval("5 m + 5"),
        Err(AbacusError::IncompatibleDimensions)
    ));
    assert!(matches!(
        strict.eval("5 + 5 m"),
        Err(AbacusError::IncompatibleDimensions)
    ));
    assert!(matches!(
        strict.eval("5 m - 2"),
        Err(AbacusError::IncompatibleDimensions)
    ));
    assert!(matches!(
        strict.eval("[1 m, 2]"),
        Err(AbacusError::IncompatibleDimensions)
    ));
    assert!(matches!(
        strict.eval("1 m .. 2"),
        Err(AbacusError::IncompatibleDimensions)
    ));

    // `%` is still allowed: it scales the base quantity.
    let pct = strict.eval("100 m + 10%").unwrap();
    assert_eq!(pct.to_display(), "110 m");
}

#[cfg(feature = "units")]
#[test]
fn test_decimal_places_configuration() {
    let calc = Abacus::standard().with_decimal_places(2);
    assert_eq!(calc.decimal_places(), Some(2));

    let res = calc.eval("10 / 3").unwrap();
    assert_eq!(res.to_display(), "3.33");

    let res_m = calc.eval("1.23456 m").unwrap();
    assert_eq!(res_m.to_display(), "1.23 m");

    // `format_result` pads to the configured number of places.
    let res_five = calc.eval("5").unwrap();
    assert_eq!(calc.format_result(&res_five), "5.00");
}

#[cfg(feature = "units")]
#[test]
fn test_interval_style_configuration() {
    let default_calc = Abacus::standard();
    let res = default_calc.eval("[1 m, 5 m] + 2 m").unwrap();
    assert_eq!(res.to_display(), "[3 m, 7 m]");

    let range_calc = Abacus::standard().with_interval_style(IntervalStyle::Range);
    let res_range = range_calc.eval("[1 m, 5 m] + 2 m").unwrap();
    assert_eq!(res_range.to_display(), "3 m..7 m");
}

#[test]
fn test_notation_modes() {
    let calc_eng = Abacus::standard().with_notation(Notation::Engineering);
    assert_eq!(calc_eng.notation(), Notation::Engineering);

    let res1 = calc_eng.eval("45000").unwrap();
    assert_eq!(calc_eng.format_result(&res1), "45e3");

    let res2 = calc_eng.eval("1200000").unwrap();
    assert_eq!(calc_eng.format_result(&res2), "1.2e6");

    let res3 = calc_eng.eval("0.045").unwrap();
    assert_eq!(calc_eng.format_result(&res3), "45e-3");

    let calc_sci = Abacus::standard().with_notation(Notation::Scientific);
    let res_sci = calc_sci.eval("1200000").unwrap();
    assert!(calc_sci.format_result(&res_sci).contains("1.2e6"));
}

#[cfg(feature = "units")]
#[test]
fn test_default_timezone_configuration() {
    let est = TimeZone::parse("EST").unwrap();
    let calc = Abacus::standard().with_default_timezone(est);
    assert_eq!(calc.default_timezone().as_ref().unwrap().name, "EST");

    // A bare date takes the default timezone.
    let res = calc.eval("07-08-2026").unwrap();
    assert_eq!(res.to_display(), "August 7, 2026 EST");

    // EST is UTC-5, so midnight EST is 05:00 UTC.
    let res_utc = calc.eval("07-08-2026 to UTC").unwrap();
    assert_eq!(res_utc.to_display(), "August 7, 2026 05:00:00 UTC");
}

#[cfg(feature = "units")]
#[test]
fn test_custom_weekend_workweek_configuration() {
    let calc = Abacus::standard().with_weekend(WeekendDays::FridaySaturday);
    assert_eq!(calc.weekend(), WeekendDays::FridaySaturday);

    // 07-08-2026 is a Friday, a weekend day under FridaySaturday.
    let res_fri = calc.eval("07-08-2026.is_weekend").unwrap();
    assert_eq!(res_fri.to_display(), "1");
    let res_work = calc.eval("07-08-2026.is_workday").unwrap();
    assert_eq!(res_work.to_display(), "0");

    // 09-08-2026 is a Sunday, a workday under FridaySaturday.
    let res_sun = calc.eval("09-08-2026.is_workday").unwrap();
    assert_eq!(res_sun.to_display(), "1");

    // Thursday 06-08-2026 + 1 business day skips Friday and Saturday, landing on Sunday.
    let res_add = calc.eval("06-08-2026 + 1 business day").unwrap();
    assert_eq!(res_add.to_display(), "August 9, 2026");

    // With the default Saturday-Sunday weekend it lands on Friday.
    let standard_calc = Abacus::standard();
    let std_add = standard_calc.eval("06-08-2026 + 1 business day").unwrap();
    assert_eq!(std_add.to_display(), "August 7, 2026");
}

#[test]
fn test_max_recursion_depth_configuration() {
    let shallow_calc = Abacus::standard().with_max_recursion_depth(3);
    assert_eq!(shallow_calc.max_recursion_depth(), 3);
    assert!(matches!(
        shallow_calc.eval("1 + (2 + (3 + (4 + 5)))"),
        Err(AbacusError::RecursionLimitExceeded)
    ));

    let deep_calc = Abacus::standard().with_max_recursion_depth(64);
    assert!(deep_calc.eval("1 + (2 + (3 + (4 + 5)))").is_ok());
}

#[test]
fn test_implicit_multiplication_configuration() {
    let calc_no_implicit = Abacus::standard().with_implicit_multiplication(false);
    assert!(!calc_no_implicit.implicit_multiplication());
    assert!(matches!(
        calc_no_implicit.eval("2(3)"),
        Err(AbacusError::UnexpectedToken(_))
    ));
    assert_eq!(calc_no_implicit.eval("2 * (3)").unwrap().to_display(), "6");

    let calc_with_implicit = Abacus::standard();
    assert!(calc_with_implicit.implicit_multiplication());
    assert_eq!(calc_with_implicit.eval("2(3)").unwrap().to_display(), "6");
}

#[test]
fn test_in_place_mutator_methods() {
    let mut calc = Abacus::standard();

    calc.set_angle_mode(AngleMode::Degrees);
    assert_eq!(calc.angle_mode(), AngleMode::Degrees);

    calc.set_strict_dimensions(true);
    assert!(calc.strict_dimensions());

    calc.set_decimal_places(Some(3));
    assert_eq!(calc.decimal_places(), Some(3));

    calc.set_interval_style(Some(IntervalStyle::Range));
    assert_eq!(calc.interval_style(), Some(IntervalStyle::Range));

    calc.set_notation(Notation::Engineering);
    assert_eq!(calc.notation(), Notation::Engineering);

    calc.set_default_timezone(Some(TimeZone::parse("EST").unwrap()));
    assert_eq!(calc.default_timezone().as_ref().unwrap().name, "EST");

    calc.set_weekend(WeekendDays::FridaySaturday);
    assert_eq!(calc.weekend(), WeekendDays::FridaySaturday);

    calc.set_max_recursion_depth(10);
    assert_eq!(calc.max_recursion_depth(), 10);

    calc.set_max_exponent(50.0);
    assert_eq!(calc.max_exponent(), 50.0);

    calc.set_implicit_multiplication(false);
    assert!(!calc.implicit_multiplication());
}

#[cfg(feature = "units")]
#[test]
fn test_max_exponent_configuration() {
    // The default limit is 1000.
    let calc = Abacus::standard();
    assert_eq!(calc.max_exponent(), 1000.0);
    assert_eq!(calc.eval("2 ^ 10").unwrap().to_display(), "1024");

    // A limit of 5.
    let low_calc = Abacus::standard().with_max_exponent(5.0);
    assert_eq!(low_calc.max_exponent(), 5.0);
    assert_eq!(low_calc.eval("2 ^ 5").unwrap().to_display(), "32");
    assert!(matches!(
        low_calc.eval("2 ^ 6"),
        Err(AbacusError::ExponentLimitExceeded)
    ));
    assert!(matches!(
        low_calc.eval("2 ^ -6"),
        Err(AbacusError::ExponentLimitExceeded)
    ));
    assert!(matches!(
        low_calc.eval("2 ^ 10000000000"),
        Err(AbacusError::ExponentLimitExceeded)
    ));

    // The limit applies to interval exponents ...
    assert!(matches!(
        low_calc.eval("[1, 2] ^ [6, 7]"),
        Err(AbacusError::ExponentLimitExceeded)
    ));

    // ... and to unit exponents.
    assert!(matches!(
        low_calc.eval("10 m^100000"),
        Err(AbacusError::ExponentLimitExceeded)
    ));
}
