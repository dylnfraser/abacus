#![cfg(feature = "units")]
use abacus::Abacus;

#[test]
fn test_auto_derived_units_default_enabled() {
    let calc = Abacus::standard();
    assert!(calc.auto_derived_units());

    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 J");

    let res = calc.eval("100 W * 5 s").unwrap();
    assert_eq!(res.to_display(), "500 J");

    let res = calc.eval("12 V * 2 A").unwrap();
    assert_eq!(res.to_display(), "24 W");
}

#[test]
fn test_auto_derived_units_disabled() {
    let calc = Abacus::standard().with_auto_derived_units(false);
    assert!(!calc.auto_derived_units());

    // Products are not reduced to J.
    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 N*m");

    let res = calc.eval("100 W * 5 s").unwrap();
    assert_eq!(res.to_display(), "500 W*s");

    // An explicit conversion to J still works.
    let explicit = calc.eval("(10 N * 5 m) to J").unwrap();
    assert_eq!(explicit.to_display(), "50 J");
}

#[test]
fn test_fixed_significant_figures() {
    let calc = Abacus::standard().with_significant_figures(3);
    assert_eq!(calc.significant_figures(), Some(3));

    let res = calc.eval("12.3456 m").unwrap();
    assert_eq!(res.to_display(), "12.3 m");

    let res = calc.eval("12345.6 m").unwrap();
    assert_eq!(res.to_display(), "12300 m");

    let calc2 = Abacus::standard().with_significant_figures(2);
    let res2 = calc2.eval("0.004567 s").unwrap();
    assert_eq!(res2.to_display(), "0.0046 s");

    // `format_result` keeps trailing zeros.
    let res3 = calc.eval("1.2 m").unwrap();
    assert_eq!(calc.format_result(&res3), "1.20 m");

    // Interval bounds are rounded too.
    let int_res = calc.eval("[1.2345 m, 2.3456 m]").unwrap();
    assert_eq!(int_res.to_display(), "[1.23 m, 2.35 m]");
}

#[test]
fn test_follow_input_significant_figures() {
    let calc = Abacus::standard().with_follow_significant_figures(true);
    assert!(calc.follow_significant_figures());

    // A product keeps the fewest sig figs among its operands: 12.3 has 3.
    let res = calc.eval("12.3 * 4.567").unwrap();
    assert_eq!(res.to_display(), "56.2");

    let res = calc.eval("2.50 m * 4.0 m").unwrap();
    assert_eq!(res.to_display(), "10 m^2");
}

#[test]
fn test_in_place_configuration_mutators() {
    let mut calc = Abacus::standard();

    calc.set_auto_derived_units(false);
    assert!(!calc.auto_derived_units());

    calc.set_significant_figures(Some(4));
    assert_eq!(calc.significant_figures(), Some(4));

    calc.set_follow_significant_figures(true);
    assert!(calc.follow_significant_figures());

    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 N*m");
}

#[test]
fn test_five_rules_of_significant_figures() {
    use abacus::count_significant_figures;

    // Rule 1: Non-zero digits are always significant (1..9).
    assert_eq!(count_significant_figures("45.2"), Some(3));
    assert_eq!(count_significant_figures("123.45"), Some(5));
    assert_eq!(count_significant_figures("7.89"), Some(3));

    // Rule 2: Captive zeros between non-zero digits are significant.
    assert_eq!(count_significant_figures("5005"), Some(4));
    assert_eq!(count_significant_figures("50.05"), Some(4));
    assert_eq!(count_significant_figures("1.002"), Some(4));
    assert_eq!(count_significant_figures("104"), Some(3));

    // Rule 3: Leading zeros are not significant (placeholders).
    assert_eq!(count_significant_figures("0.0045"), Some(2));
    assert_eq!(count_significant_figures("0.05"), Some(1));
    assert_eq!(count_significant_figures("0045"), Some(2));
    assert_eq!(count_significant_figures("0.000105"), Some(3));

    // Rule 4: Trailing zeros are significant only if a decimal point is present.
    assert_eq!(count_significant_figures("150."), Some(3));
    assert_eq!(count_significant_figures("150"), Some(2));
    assert_eq!(count_significant_figures("150.0"), Some(4));
    assert_eq!(count_significant_figures("12.30"), Some(4));
    assert_eq!(count_significant_figures("0.00450"), Some(3));
    assert_eq!(count_significant_figures("100."), Some(3));
    assert_eq!(count_significant_figures("100"), Some(1));
    assert_eq!(count_significant_figures("1200"), Some(2));
    assert_eq!(count_significant_figures("1200.00"), Some(6));

    // Rule 5: Exact numbers have infinite significant figures.
    let calc = Abacus::standard().with_follow_significant_figures(true);

    let res1 = calc.eval("150. * 2.50").unwrap();
    assert_eq!(res1.to_display(), "375");

    // In an all-integer expression, integers follow rule 4: 150 and 25 have 2 sig figs, so
    // 3750 rounds to 3800.
    let res2 = calc.eval("150 * 25").unwrap();
    assert_eq!(res2.to_display(), "3800");

    // An integer multiplying a measurement is an exact count and doesn't limit precision.
    let res3 = calc.eval("3 * 4.52 g").unwrap();
    assert_eq!(res3.to_display(), "13.6 g");

    // Exponents are exact, so the square keeps 3 sig figs.
    let res4 = calc.eval("(4.52 m) ^ 2").unwrap();
    assert_eq!(res4.to_display(), "20.4 m^2");
}

// Setter conventions

#[cfg(feature = "units")]
#[test]
fn builders_setters_and_getters_agree() {
    use abacus::{AngleMode, IntervalStyle, Notation};
    let built = Abacus::standard()
        .with_angle_mode(AngleMode::Degrees)
        .with_decimal_places(2)
        .with_interval_style(IntervalStyle::Range)
        .with_notation(Notation::Scientific)
        .with_max_recursion_depth(usize::MAX)
        .with_max_exponent(5.0);
    let mut set = Abacus::standard();
    set.set_angle_mode(AngleMode::Degrees)
        .set_decimal_places(2)
        .set_interval_style(IntervalStyle::Range)
        .set_notation(Notation::Scientific)
        .set_max_recursion_depth(usize::MAX)
        .set_max_exponent(5.0);
    for calc in [&built, &set] {
        assert_eq!(calc.angle_mode(), AngleMode::Degrees);
        assert_eq!(calc.decimal_places(), Some(2));
        assert_eq!(calc.interval_style(), Some(IntervalStyle::Range));
        assert_eq!(calc.notation(), Notation::Scientific);
        assert_eq!(calc.max_recursion_depth(), abacus::MAX_RECURSION_DEPTH_CAP);
        // One source of truth for the exponent limit: `^` and unit symbols agree.
        assert_eq!(calc.max_exponent(), calc.units().max_exponent());
        assert!(calc.eval("2^6").is_err());
        assert!(calc.eval("1 m^6").is_err());
    }
    set.set_decimal_places(None);
    assert_eq!(set.decimal_places(), None);
}

#[cfg(feature = "units")]
#[test]
fn default_and_new_are_the_standard_engine() {
    for calc in [Abacus::default(), Abacus::new()] {
        assert_eq!(calc.eval("1 km in m").unwrap().to_display(), "1000 m");
    }
    assert!(Abacus::empty().eval("1 m").is_err());
    assert!(Abacus::empty().eval("1 + 2").is_err());
    assert_eq!(Abacus::empty().eval("(3)").unwrap().to_display(), "3");
}

#[test]
fn eval_to_string_applies_notation() {
    let calc = Abacus::standard().with_notation(abacus::Notation::Scientific);
    assert_eq!(calc.eval_to_string("12345").unwrap(), "1.2345e4");
    assert_eq!(calc.eval("12345").unwrap().to_display(), "12345");
}

#[cfg(feature = "currencies")]
#[test]
fn currency_toggle_reflects_the_registry() {
    let mut calc = Abacus::standard();
    assert!(calc.currencies());
    calc.set_currencies(false);
    assert!(!calc.currencies());
    assert!(calc.eval("$5").is_err());
    calc.set_currencies(true);
    assert!(calc.currencies());
    assert_eq!(calc.eval("$5").unwrap().to_display(), "$5");
}
