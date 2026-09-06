use abacus::{Abacus, AbacusError, eval};

#[test]
fn test_fractional_exponent_dimensions() {
    // Square root of m^4 / s^2 is m^2/s
    let res = eval("sqrt(9 m^4 / s^2)").unwrap();
    assert_eq!(res.to_display(), "3 m^2/s");

    // Power with fractional exponents
    let res2 = Abacus::standard().eval_scalar("(16 m^2)^0.5").unwrap();
    assert_eq!(res2.canonical, 4.0);

    // Negative exponent
    let res3 = Abacus::standard().eval_scalar("1 / (4 s^2)^0.5").unwrap();
    assert_eq!(res3.canonical, 0.5);
}

#[test]
fn test_mixed_prefix_and_unit_conversion_chains() {
    // Mixed metric prefix addition: km + m + cm -> m
    let res = eval("(1 km + 500 m + 2500 cm) as m").unwrap();
    assert_eq!(res.to_display(), "1525 m");

    // Storage unit conversions & binary prefixes
    let res2 = eval("1 TiB to GiB").unwrap();
    assert_eq!(res2.to_display(), "1024 GiB");

    let res3 = eval("(1 MB + 500 kB) as kB").unwrap();
    assert_eq!(res3.to_display(), "1500 kB");

    let res4 = Abacus::standard().eval_scalar("1 GiB / 1 MiB").unwrap();
    assert_eq!(res4.canonical, 1024.0);
}

#[test]
fn test_affine_temperature_conversions_and_protections() {
    // Valid conversions
    assert_eq!(eval("0 °C as °F").unwrap().to_display(), "32 °F");
    assert_eq!(eval("100 °C as K").unwrap().to_display(), "373.15 K");
    assert_eq!(eval("98.6 °F as °C").unwrap().to_display(), "37 °C");

    // Disallowed affine arithmetic (e.g. adding two affine temperatures directly)
    assert!(matches!(
        eval("20 °C + 10 °C"),
        Err(AbacusError::AffineUnitOperation(_))
    ));
    assert!(matches!(
        eval("++ 20 °C"),
        Err(AbacusError::AffineUnitOperation(_))
    ));
}

#[test]
fn test_unusual_niche_and_humorous_units() {
    // Smoot
    let smoot = eval("1 smoot to inches").unwrap();
    assert_eq!(smoot.to_display(), "67 in");

    // Fortnight
    let fortnight = eval("1 fortnight to h").unwrap();
    assert_eq!(fortnight.to_display(), "336 h");

    // Typography
    let pica = eval("1 pica to point").unwrap();
    assert_eq!(pica.to_display(), "12 pt_type");
}

#[test]
fn test_cgs_and_astronomical_physics_conversions() {
    // Astronomical: pc to ly
    let pc = Abacus::standard().eval_scalar("1 pc to ly").unwrap();
    let ly_value = pc.canonical / 9_460_730_472_580_800.0;
    assert!((ly_value - 3.2615637).abs() < 1e-4);

    // AU to km
    let au = eval("1 au to km").unwrap();
    assert_eq!(au.to_display(), "149597870.7 km");

    // CGS Physics: bar to Pa
    let bar = eval("1 bar to Pa").unwrap();
    assert_eq!(bar.to_display(), "100000 Pa");
}

#[test]
fn test_complex_implicit_multiplication() {
    // 2(3 + 4)(5 + 6) = 2 * 7 * 11 = 154
    assert_eq!(eval("2(3 + 4)(5 + 6)").unwrap().to_display(), "154");

    // Juxtaposition with functions: 2 sqrt(16 m^2) 3 = 2 * 4 m * 3 = 24 m
    assert_eq!(eval("2 sqrt(16 m^2) 3").unwrap().to_display(), "24 m");

    // Double parentheses juxtaposition
    assert_eq!(eval("2(3)4").unwrap().to_display(), "24");

    // Complex nested parentheses
    assert_eq!(eval("((2 + 3) * (4 + 5))^2").unwrap().to_display(), "2025");
}

#[test]
fn test_statistical_dispersion_and_multi_unit_ranges() {
    // Mean of mixed compatible units: 1 m, 100 cm, 2000 mm -> 4/3 m = 1.3333333333333333 m
    let m = Abacus::standard()
        .eval_scalar("mean(1 m, 100 cm, 2000 mm)")
        .unwrap();
    assert!((m.canonical - 1.3333333333333333).abs() < 1e-6);

    // Quantile
    let q = eval("quantile(1 m .. 9 m, 0.5)").unwrap();
    assert_eq!(q.to_display(), "5 m");

    // Pearson Correlation
    let corr_pos = Abacus::standard()
        .eval_scalar("corr(1..5, 10..14)")
        .unwrap()
        .canonical;
    assert!((corr_pos - 1.0).abs() < 1e-6);

    let corr_neg = Abacus::standard()
        .eval_scalar("corr(1..5, 5..1)")
        .unwrap()
        .canonical;
    assert!((corr_neg - (-1.0)).abs() < 1e-6);
}

#[test]
fn test_custom_range_step_expansion_edge_cases() {
    // Step expansion with units: 0 m .. 10 m .. 2.5 m -> sum = 0 + 2.5 + 5 + 7.5 + 10 = 25 m
    assert_eq!(
        eval("sum(0 m .. 10 m .. 2.5 m)").unwrap().to_display(),
        "25 m"
    );

    // Range with step in reverse direction
    assert_eq!(eval("sum(10 .. 0 .. -2)").unwrap().to_display(), "30");
}

#[test]
fn test_advanced_trigonometric_angle_units() {
    assert_eq!(eval("sin(90 deg)").unwrap().to_display(), "1");
    assert_eq!(eval("cos(180 deg)").unwrap().to_display(), "-1");
    assert_eq!(eval("tan(45 deg)").unwrap().to_display(), "1");
    assert_eq!(eval("asin(1) to deg").unwrap().to_display(), "90 deg");

    let atan_res = eval("atan2(1 m, 1 m) to deg").unwrap();
    assert_eq!(atan_res.to_display(), "45 deg");
}

#[test]
fn test_financial_npv_irr_and_tvm() {
    // Net Present Value (NPV): rate = 10%, CFs = [-1000, 300, 400, 500] -> -19.124434
    let npv_val = Abacus::standard()
        .eval_scalar("npv(0.1, -1000, 300, 400, 500)")
        .unwrap()
        .canonical;
    assert!((npv_val - (-19.124434)).abs() < 1e-3);

    // Internal Rate of Return (IRR): CFs = [-100, 60, 60] -> ~13.066%
    let irr_val = Abacus::standard()
        .eval_scalar("irr(-100, 60, 60)")
        .unwrap()
        .canonical;
    assert!((irr_val - 0.13066).abs() < 1e-3);

    // Future Value (FV)
    let fv_val = Abacus::standard()
        .eval_scalar("fv(0.05, 10, -1000, -10000)")
        .unwrap()
        .canonical;
    assert!((fv_val - 28866.83).abs() < 1e-1);
}

#[test]
fn test_statistical_distributions_and_inverse_cdfs() {
    // Normal CDF round-trip with inverse
    let p = Abacus::standard()
        .eval_scalar("normcdf(1.95996)")
        .unwrap()
        .canonical;
    assert!((p - 0.975).abs() < 1e-4);

    let x = Abacus::standard()
        .eval_scalar("invnorm(0.975)")
        .unwrap()
        .canonical;
    assert!((x - 1.95996).abs() < 1e-3);

    // Student's t distribution
    let t_inv = Abacus::standard()
        .eval_scalar("invt(0.975, 10)")
        .unwrap()
        .canonical;
    assert!((t_inv - 2.2281388).abs() < 1e-4);

    // Binomial CDF
    let b_cdf = Abacus::standard()
        .eval_scalar("binomcdf(10, 0.5, 5)")
        .unwrap()
        .canonical;
    assert!((b_cdf - 0.623046875).abs() < 1e-5);
}

#[test]
fn test_chained_unary_incrementers_and_factorials() {
    // Chained prefix increments: ++ ++5 = 7
    assert_eq!(eval("++ ++5").unwrap().to_display(), "7");

    // Double negation: - - 5 = 5
    assert_eq!(eval("- - 5").unwrap().to_display(), "5");

    // Factorial of 5 = 120
    assert_eq!(eval("5!").unwrap().to_display(), "120");
}

#[test]
fn test_niche_error_handling_and_boundary_conditions() {
    // Empty input
    assert!(matches!(eval(""), Err(AbacusError::UnexpectedEnd)));

    // Unclosed parenthesis
    assert!(matches!(eval("(5 + 3"), Err(AbacusError::UnclosedParen)));

    // Invalid inverse norm probability (> 1)
    assert!(matches!(
        eval("invnorm(1.5)"),
        Err(AbacusError::IncompatibleFunctionArguments)
    ));

    // Division by zero gives infinity
    assert_eq!(
        Abacus::standard().eval_scalar("5 / 0").unwrap().canonical,
        f64::INFINITY
    );

    // Negative factorial error
    assert!(matches!(
        eval("factorial(-3)"),
        Err(AbacusError::IncompatibleFunctionArguments)
    ));
}

#[test]
fn test_niche_relative_date_and_time_edge_cases() {
    let abacus = Abacus::standard();
    let today = abacus::Date::today();
    let tomorrow = abacus::Date::tomorrow();

    // Crossing midnight backwards: 3 hours before tmr at 1am -> today at 22:00
    let d_cross_back = abacus.eval_date("3 hours before tmr at 1am").unwrap();
    assert_eq!(d_cross_back.day, today.day);
    assert_eq!(d_cross_back.time.hour, 22);

    // Crossing midnight forwards: 25 hours after tdy at 11pm -> day after tomorrow at 00:00
    let d_cross_fw = abacus.eval_date("25 hours after tdy at 11pm").unwrap();
    let day_after_tomorrow = tomorrow.add_days(1);
    assert_eq!(d_cross_fw.day, day_after_tomorrow.day);
    assert_eq!(d_cross_fw.time.hour, 0);

    // Exact difference between tmr and tdy in hours
    let diff_hrs = abacus.eval("tdy at 3pm to tmr at 3pm in hours").unwrap();
    assert_eq!(diff_hrs.to_display(), "24 h");

    // Exact difference in days
    let diff_days = abacus.eval("tdy to tmr in days").unwrap();
    assert_eq!(diff_days.to_display(), "1 d");

    // Interval from 3 hours ago to in 3 hours -> 6 hours
    let interval_diff = abacus
        .eval("((3 hours ago) to (in 3 hours)) in hours")
        .unwrap();
    assert_eq!(interval_diff.to_display(), "6 h");

    // Property access on tdy and tmr
    let dow_tdy = abacus.eval_scalar("tdy.day_of_week").unwrap().canonical;
    assert_eq!(dow_tdy, today.day_of_week() as u32 as f64);

    let dow_tmr = abacus.eval_scalar("tmr.day_of_week").unwrap().canonical;
    assert_eq!(dow_tmr, tomorrow.day_of_week() as u32 as f64);
}

#[test]
fn test_niche_unparenthesized_function_chaining() {
    let abacus = Abacus::standard();

    // Nested unparenthesized calls: ln exp 5 -> 5
    let ln_exp = abacus.eval_scalar("ln exp 5").unwrap().canonical;
    assert!((ln_exp - 5.0).abs() < 1e-5);

    // Double unparenthesized sqrt: sqrt sqrt 81 -> 3
    assert_eq!(abacus.eval("sqrt sqrt 81").unwrap().to_display(), "3");

    // Trigonometric unparenthesized with unit conversion: acos -1 in deg -> 180 deg
    let acos_deg = abacus.eval("acos -1 in deg").unwrap();
    assert_eq!(acos_deg.to_display(), "180 deg");

    // Floor and ceil unparenthesized with physical units
    assert_eq!(abacus.eval("floor 5.9 m").unwrap().to_display(), "5 m");
    assert_eq!(abacus.eval("ceil 3.1 s").unwrap().to_display(), "4 s");

    // Unparenthesized abs with energy reduction: abs -10 N * 2 m -> 20 J
    assert_eq!(abacus.eval("abs -10 N * 2 m").unwrap().to_display(), "20 J");
}

#[test]
fn test_numerical_stability_edge_cases() {
    let abacus = Abacus::standard();

    // 1. Poisson PMF and CDF with k > 170
    let p_pdf = abacus
        .eval_scalar("poissonpdf(200, 200)")
        .unwrap()
        .canonical;
    assert!(p_pdf.is_finite() && p_pdf > 0.0);
    assert!((p_pdf - 0.0282).abs() < 1e-3);

    let p_cdf = abacus
        .eval_scalar("poissoncdf(200, 200)")
        .unwrap()
        .canonical;
    assert!(p_cdf.is_finite() && (p_cdf - 0.5188).abs() < 1e-2);

    // 2. Hypergeometric rejection of negative / invalid arguments
    assert!(abacus.eval("hypgeompdf(-10, -5, -2, -1)").is_err());
    assert!(abacus.eval("hypgeomcdf(-10, -5, -2, -1)").is_err());

    // 3. IRR divergence handling when no rate exists
    assert!(abacus.eval("irr(100, 200, 300)").is_err());
}

#[test]
fn test_code_review_findings_cr_001_through_cr_006() {
    let abacus = Abacus::standard();

    // CR-001: Range overflow in RangeSeq::new on massive bounds
    assert!(abacus.eval("mean(0..1e308)").is_err());
    assert!(abacus.eval("sum(1..1e300)").is_err());

    // CR-002: Interval division singularity with non-SI units preserves correct canonical scaling
    let int_div = abacus.eval("[10 km, 20 km] / [0 h, 2 h]").unwrap();
    assert_eq!(int_div.to_display(), "[5 km/h, inf km/h]");

    // CR-003: Affine units rejected by abs and sign
    assert!(matches!(
        abacus.eval("abs(-20 °C)"),
        Err(AbacusError::AffineUnitOperation(_))
    ));
    assert!(matches!(
        abacus.eval("sign(-20 °C)"),
        Err(AbacusError::AffineUnitOperation(_))
    ));

    // CR-004: Unary sqrt unit simplification doesn't drop units in compound products
    let sqrt_res = abacus.eval("sqrt(4 m * s)").unwrap();
    assert_eq!(sqrt_res.to_display(), "2 (m*s)^0.5");

    // CR-005: Significant figure rounding when decade shifts
    let calc = Abacus::standard().with_significant_figures(2);
    let sf2_a = calc.eval("0.0999").unwrap();
    assert_eq!(calc.format_result(&sf2_a), "0.10");
    let sf2_b = calc.eval("9.99").unwrap();
    assert_eq!(calc.format_result(&sf2_b), "10");

    // CR-006: Clamp with NaN bounds returns Err instead of panicking
    assert!(abacus.eval("clamp(5, 0/0, 10)").is_err());
    assert!(abacus.eval("clamp(5, 0, 0/0)").is_err());
}

#[test]
fn test_code_review_findings_cr_007_through_cr_009() {
    let abacus = Abacus::standard();

    // CR-007: round_to_decimals with decimals >= 308 does not produce NaN
    let val = abacus::units::value::Value::dimensionless(123.456);
    let rounded = val.round_to_decimals(400);
    assert_eq!(rounded.canonical, 123.456);
    assert!(!rounded.canonical.is_nan());

    let calc_dec = Abacus::standard().with_decimal_places(400);
    let res = calc_dec.eval("123.456").unwrap();
    if let abacus::EvalResult::Scalar(v) = res {
        assert_eq!(v.canonical, 123.456);
        assert!(!v.canonical.is_nan());
    }

    // CR-008: UnitRegistry inherits and enforces configured max_exponent
    let low_exp_calc = Abacus::standard().with_max_exponent(5.0);
    assert_eq!(low_exp_calc.units.max_exponent, 5.0);
    assert!(low_exp_calc.eval("5 m^5").is_ok());
    assert!(matches!(
        low_exp_calc.eval("5 m^6"),
        Err(AbacusError::ExponentLimitExceeded)
    ));
    assert!(matches!(
        low_exp_calc.eval("5 m^-6"),
        Err(AbacusError::ExponentLimitExceeded)
    ));

    // CR-009: Date arithmetic does not overflow or panic on extreme bounds
    let d = abacus::units::date::Date::new(2025, 1, 1);
    let _ = d.add_days(i64::MAX);
    let _ = d.add_days(i64::MIN);
    let _ = d.add_seconds(i64::MAX);
    let _ = d.add_minutes(i64::MAX);
    let _ = d.add_hours(i64::MAX);
    let _ = d.add_milliseconds(i64::MAX);
    let _ = d.add_years(i32::MAX);
    let _ = d.sub_days(i64::MAX);
    let _ = abacus.eval("2025-01-01 + 100000000000000 days");
}
