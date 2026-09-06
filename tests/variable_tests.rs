use abacus::{Abacus, eval};

#[test]
fn test_standard_constant_pi() {
    let pi_res = eval("pi").unwrap();
    assert!((pi_res.into_scalar().unwrap().canonical - std::f64::consts::PI).abs() < 1e-12);

    let upper_pi = eval("PI").unwrap();
    assert!((upper_pi.into_scalar().unwrap().canonical - std::f64::consts::PI).abs() < 1e-12);

    // sin(pi / 2) == 1
    let sin_pi_half = eval("sin(pi / 2)").unwrap();
    assert!((sin_pi_half.into_scalar().unwrap().canonical - 1.0).abs() < 1e-12);

    // cos(pi) == -1
    let cos_pi = eval("cos(pi)").unwrap();
    assert!((cos_pi.into_scalar().unwrap().canonical - (-1.0)).abs() < 1e-12);

    // 2 * pi
    let two_pi = eval("2 * pi").unwrap();
    assert!((two_pi.into_scalar().unwrap().canonical - 2.0 * std::f64::consts::PI).abs() < 1e-12);
}

#[test]
fn test_standard_constant_e() {
    let e_res = eval("e").unwrap();
    assert!((e_res.into_scalar().unwrap().canonical - std::f64::consts::E).abs() < 1e-12);

    let upper_e = eval("E").unwrap();
    assert!((upper_e.into_scalar().unwrap().canonical - std::f64::consts::E).abs() < 1e-12);

    // ln(e) == 1
    let ln_e = eval("ln(e)").unwrap();
    assert!((ln_e.into_scalar().unwrap().canonical - 1.0).abs() < 1e-12);

    // e^2
    let e_squared = eval("e^2").unwrap();
    assert!(
        (e_squared.into_scalar().unwrap().canonical - std::f64::consts::E.powi(2)).abs() < 1e-12
    );
}

#[test]
fn test_standard_constants_tau_and_phi() {
    let tau_res = eval("tau").unwrap();
    assert!((tau_res.into_scalar().unwrap().canonical - std::f64::consts::TAU).abs() < 1e-12);

    let tau_over_pi = eval("tau / pi").unwrap();
    assert!((tau_over_pi.into_scalar().unwrap().canonical - 2.0).abs() < 1e-12);

    let phi_res = eval("phi").unwrap();
    let expected_phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    assert!((phi_res.into_scalar().unwrap().canonical - expected_phi).abs() < 1e-12);

    // Golden ratio identity: phi^2 - phi - 1 == 0
    let phi_identity = eval("phi^2 - phi - 1").unwrap();
    assert!(phi_identity.into_scalar().unwrap().canonical.abs() < 1e-12);
}

#[test]
fn test_implicit_multiplication_with_constants() {
    // 2pi == 2 * pi
    let res1 = eval("2pi").unwrap();
    assert!((res1.into_scalar().unwrap().canonical - 2.0 * std::f64::consts::PI).abs() < 1e-12);

    // 2 pi
    let res2 = eval("2 pi").unwrap();
    assert!((res2.into_scalar().unwrap().canonical - 2.0 * std::f64::consts::PI).abs() < 1e-12);

    // 4e == 4 * e
    let res3 = eval("4e").unwrap();
    assert!((res3.into_scalar().unwrap().canonical - 4.0 * std::f64::consts::E).abs() < 1e-12);

    // Scientific notation 4e2 must still be 400.0, NOT 4 * e^2
    let res4 = eval("4e2").unwrap();
    assert_eq!(res4.into_scalar().unwrap().canonical, 400.0);
}

#[test]
fn test_programmatic_variable_definition() {
    let calc = Abacus::standard()
        .with_variable("x", 42.0)
        .with_variable("y", 8.0);

    assert_eq!(calc.eval("x + y").unwrap().to_display(), "50");
    assert_eq!(calc.eval("x * 2").unwrap().to_display(), "84");
    assert_eq!(calc.eval("x - y").unwrap().to_display(), "34");

    // Implicit multiplication with variables: 2x, x y
    assert_eq!(calc.eval("2x").unwrap().to_display(), "84");
    assert_eq!(calc.eval("x y").unwrap().to_display(), "336");
}

#[test]
fn test_variables_with_physical_units() {
    let mut calc = Abacus::standard();
    let mass = calc.eval("10 kg").unwrap();
    let acceleration = calc.eval("9.8 m/s^2").unwrap();

    calc.set_variable("mass", mass);
    calc.set_variable("a", acceleration);

    let force = calc.eval("mass * a").unwrap();
    assert_eq!(force.to_display(), "98 N");

    let speed = calc.eval("100 km/h").unwrap();
    calc.set_variable("v", speed);
    let converted = calc.eval("v in m/s").unwrap();
    assert_eq!(converted.to_display(), "27.77777777777778 m/s");
}

#[test]
fn test_variables_with_intervals() {
    let mut calc = Abacus::standard();
    calc.set_variable_expr("tolerance", "[95 ohm, 105 ohm]")
        .unwrap();

    let doubled = calc.eval("tolerance * 2").unwrap();
    assert_eq!(doubled.to_display(), "[190 Ω, 210 Ω]");
}

#[test]
fn test_variables_with_dates() {
    let mut calc = Abacus::standard();
    calc.set_variable_expr("start_date", "2026-08-01").unwrap();

    let end_date = calc.eval("start_date + 10 days").unwrap();
    assert_eq!(end_date.to_display(), "August 11, 2026");
}

#[test]
fn test_eval_mut_assignment_syntax() {
    let mut calc = Abacus::standard();

    // Assignment creates variable
    let assign_res = calc.eval_mut("r = 5 m").unwrap();
    assert_eq!(assign_res.to_display(), "5 m");
    assert!(calc.has_variable("r"));

    // Variable can be referenced in subsequent evals
    let area = calc.eval("pi * r^2").unwrap();
    assert_eq!(area.to_display(), "78.53981633974483 (m)^2");

    // Multiple chained assignments
    calc.eval_mut("w = 10 m").unwrap();
    calc.eval_mut("h = 2 m").unwrap();
    calc.eval_mut("volume = r * w * h").unwrap();
    assert_eq!(calc.eval("volume").unwrap().to_display(), "100 m^3");
}

#[test]
fn test_variable_management_methods() {
    let mut calc = Abacus::standard();
    calc.set_variable("temp_var", 100.0);
    assert!(calc.has_variable("temp_var"));
    assert_eq!(calc.get_variable("temp_var").unwrap().to_display(), "100");

    let removed = calc.remove_variable("temp_var");
    assert!(removed.is_some());
    assert!(!calc.has_variable("temp_var"));

    // Clear and reset standard variables
    calc.set_variable("custom", 123.0);
    calc.clear_variables();
    assert!(!calc.has_variable("custom"));
    assert!(!calc.has_variable("pi"));

    calc.reset_standard_variables();
    assert!(calc.has_variable("pi"));
    assert!(calc.has_variable("e"));
}

#[test]
fn test_undefined_variable_error() {
    let calc = Abacus::standard();
    let result = calc.eval("unknown_var_name + 1");
    assert!(result.is_err());
}
