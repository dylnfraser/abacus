#![cfg(feature = "units")]
use abacus::Abacus;

#[test]
fn test_auto_derived_units_default_enabled() {
    let calc = Abacus::standard();
    assert!(calc.auto_derived_units);

    // 10 N * 5 m -> 50 J
    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 J");

    // 100 W * 5 s -> 500 J
    let res = calc.eval("100 W * 5 s").unwrap();
    assert_eq!(res.to_display(), "500 J");

    // 12 V * 2 A -> 24 W
    let res = calc.eval("12 V * 2 A").unwrap();
    assert_eq!(res.to_display(), "24 W");
}

#[test]
fn test_auto_derived_units_disabled() {
    let calc = Abacus::standard().with_auto_derived_units(false);
    assert!(!calc.auto_derived_units);

    // 10 N * 5 m -> 50 N*m (not reduced to J)
    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 N*m");

    // 100 W * 5 s -> 500 W*s (not reduced to J)
    let res = calc.eval("100 W * 5 s").unwrap();
    assert_eq!(res.to_display(), "500 W*s");

    // Explicit conversion overrides the disabled auto-reduction
    let explicit = calc.eval("(10 N * 5 m) to J").unwrap();
    assert_eq!(explicit.to_display(), "50 J");
}

#[test]
fn test_fixed_significant_figures() {
    let calc = Abacus::standard().with_significant_figures(3);
    assert_eq!(calc.significant_figures, Some(3));

    // 12.3456 m -> 12.3 m
    let res = calc.eval("12.3456 m").unwrap();
    assert_eq!(res.to_display(), "12.3 m");

    // 12345.6 m -> 12300 m
    let res = calc.eval("12345.6 m").unwrap();
    assert_eq!(res.to_display(), "12300 m");

    // 0.004567 s with 2 sig figs
    let calc2 = Abacus::standard().with_significant_figures(2);
    let res2 = calc2.eval("0.004567 s").unwrap();
    assert_eq!(res2.to_display(), "0.0046 s");

    // Formatted trailing zeros via format_result
    let res3 = calc.eval("1.2 m").unwrap();
    assert_eq!(calc.format_result(&res3), "1.20 m");

    // Degenerate/Interval rounding
    let int_res = calc.eval("[1.2345 m, 2.3456 m]").unwrap();
    assert_eq!(int_res.to_display(), "[1.23 m, 2.35 m]");
}

#[test]
fn test_follow_input_significant_figures() {
    let calc = Abacus::standard().with_follow_significant_figures(true);
    assert!(calc.follow_significant_figures);

    // 12.3 (3 sig figs) * 4.567 (4 sig figs) -> minimum is 3 sig figs
    // 12.3 * 4.567 = 56.1741 -> rounded to 3 sig figs = 56.2
    let res = calc.eval("12.3 * 4.567").unwrap();
    assert_eq!(res.to_display(), "56.2");

    // 2.50 m (3 sig figs) * 4.0 m (2 sig figs) -> min is 2 sig figs = 10 m^2
    let res = calc.eval("2.50 m * 4.0 m").unwrap();
    assert_eq!(res.to_display(), "10 m^2");
}

#[test]
fn test_in_place_configuration_mutators() {
    let mut calc = Abacus::standard();

    calc.set_auto_derive(false);
    assert!(!calc.auto_derived_units);

    calc.set_sig_figs(Some(4));
    assert_eq!(calc.significant_figures, Some(4));

    calc.set_follow_sig_figs(true);
    assert!(calc.follow_significant_figures);

    let res = calc.eval("10 N * 5 m").unwrap();
    assert_eq!(res.to_display(), "50 N*m");
}

#[test]
fn test_five_rules_of_significant_figures() {
    use abacus::count_significant_figures;

    // Rule 1: Non-zero digits are always significant (1..9).
    // e.g. 45.2 has 3 sig figs
    assert_eq!(count_significant_figures("45.2"), Some(3));
    assert_eq!(count_significant_figures("123.45"), Some(5));
    assert_eq!(count_significant_figures("7.89"), Some(3));

    // Rule 2: Captive zeros between non-zero digits are significant.
    // e.g. 5005 has 4 sig figs
    assert_eq!(count_significant_figures("5005"), Some(4));
    assert_eq!(count_significant_figures("50.05"), Some(4));
    assert_eq!(count_significant_figures("1.002"), Some(4));
    assert_eq!(count_significant_figures("104"), Some(3));

    // Rule 3: Leading zeros are not significant (placeholders).
    // e.g. 0.0045 has 2 sig figs
    assert_eq!(count_significant_figures("0.0045"), Some(2));
    assert_eq!(count_significant_figures("0.05"), Some(1));
    assert_eq!(count_significant_figures("0045"), Some(2));
    assert_eq!(count_significant_figures("0.000105"), Some(3));

    // Rule 4: Trailing zeros are significant only if a decimal point is present.
    // e.g. 150. has 3 sig figs, while 150 has only 2 sig figs.
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

    // 150. (3 sig figs) * 2.50 (3 sig figs) -> 375
    let res1 = calc.eval("150. * 2.50").unwrap();
    assert_eq!(res1.to_display(), "375");

    // Pure integers in pure integer expressions follow Rule 4:
    // 150 (2 sig figs) * 25 (2 sig figs) = 3750 -> rounded to 2 sig figs -> 3800
    let res2 = calc.eval("150 * 25").unwrap();
    assert_eq!(res2.to_display(), "3800");

    // Exact count multiplier with measurement:
    // 3 * 4.52 g (3 sig figs) -> 13.6 g (3 sig figs, not limited by integer 3)
    let res3 = calc.eval("3 * 4.52 g").unwrap();
    assert_eq!(res3.to_display(), "13.6 g");

    // Exact mathematical powers: (4.52 m) ^ 2 -> 20.4 m^2 (3 sig figs, not 1)
    let res4 = calc.eval("(4.52 m) ^ 2").unwrap();
    assert_eq!(res4.to_display(), "20.4 m^2");
}
