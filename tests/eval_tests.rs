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
use abacus::{Abacus, AbacusError, eval};

#[test]
fn test_basic_arithmetic() {
    // Verifies basic scalar operations (+, -, *, /, ^, %) using Pratt parser precedence
    let calc = Abacus::standard();

    assert_eq!(calc.eval("2 + 3").unwrap().to_display(), "5");
    assert_eq!(calc.eval("10 - 4").unwrap().to_display(), "6");
    assert_eq!(calc.eval("6 * 7").unwrap().to_display(), "42");
    assert_eq!(calc.eval("20 / 5").unwrap().to_display(), "4");
    assert_eq!(calc.eval("2 ^ 3").unwrap().to_display(), "8");
    assert_eq!(calc.eval("10 % 3").unwrap().to_display(), "1");
}

#[cfg(feature = "units")]
#[test]
fn test_top_level_eval_convenience_function() {
    // Tests top-level exported helper function abacus::eval(...)
    assert_eq!(eval("5 m + 3 m").unwrap().to_display(), "8 m");
    assert_eq!(eval("10 / 2").unwrap().to_display(), "5");
}

#[cfg(feature = "units")]
#[test]
fn test_section1_physical_unit_arithmetic_expanded() {
    let calc = Abacus::standard();

    // Power & Energy: 500 W * 2 h -> 3600000 J
    assert_eq!(calc.eval("500 W * 2 h").unwrap().to_display(), "3600000 J");

    // Speed & Acceleration: (100 km / 2 h) / 5 s in m/s^2
    let acc = calc.eval("(100 km / 2 h) / 5 s in m/s^2").unwrap();
    assert_eq!(acc.to_display(), "2.7777777777777777 m/s^2");

    // Data Storage: 500 MB + 1.5 GB in GB -> 2 GB
    assert_eq!(
        calc.eval("500 MB + 1.5 GB in GB").unwrap().to_display(),
        "2 GB"
    );

    // Force * Distance -> Energy: 10 N * 5 m -> 50 J
    assert_eq!(calc.eval("10 N * 5 m").unwrap().to_display(), "50 J");
}

#[cfg(feature = "units")]
#[test]
fn test_unit_arithmetic() {
    // Verifies unit-aware addition, subtraction, multiplication, and division
    assert_eq!(eval("5 m + 3 m").unwrap().to_display(), "8 m");
    assert_eq!(eval("10 km - 2 km").unwrap().to_display(), "8 km");
    assert_eq!(eval("5 m * 2 m").unwrap().to_display(), "10 m^2");
    assert_eq!(eval("10 m / 2 s").unwrap().to_display(), "5 m/s");
}

#[cfg(feature = "units")]
#[test]
fn test_implicit_multiplication() {
    // Tests juxtaposition implicit multiplication without explicit '*' (e.g., 5(2+3) -> 25)
    assert_eq!(eval("5(2 + 3)").unwrap().to_display(), "25");
    assert_eq!(eval("(2 + 3)(4 + 5)").unwrap().to_display(), "45");
    assert_eq!(eval("2 sqrt(9 m^2)").unwrap().to_display(), "6 m");
}

#[cfg(feature = "units")]
#[test]
fn test_dimensionless_promotion() {
    // Tests automatic unit promotion when adding/subtracting dimensionless scalars to physical units
    assert_eq!(eval("5 m + 5").unwrap().to_display(), "10 m");
    assert_eq!(eval("5 cm + 5").unwrap().to_display(), "10 cm");
    assert_eq!(eval("10 cm - 3").unwrap().to_display(), "7 cm");
    assert_eq!(eval("1 as inches").unwrap().to_display(), "1 in");
}

#[cfg(feature = "units")]
#[test]
fn test_unit_conversions() {
    // Tests explicit user conversion syntax ('as', 'to', 'in')
    assert_eq!(eval("(5 m + 20 cm) as m").unwrap().to_display(), "5.2 m");
    assert_eq!(eval("5 km to m").unwrap().to_display(), "5000 m");
    assert_eq!(eval("100 cm in m").unwrap().to_display(), "1 m");
    assert_eq!(eval("1m/m").unwrap().to_display(), "1");
    assert_eq!(eval("5 km / m").unwrap().to_display(), "5000");
}

#[cfg(feature = "units")]
#[test]
fn test_automatic_derived_si_unit_reduction() {
    // Tests automatic reduction of compound units into SI derived units (e.g. N*m -> J, W*s -> J, V*A -> W)
    assert_eq!(eval("10 N * 5 m").unwrap().to_display(), "50 J");
    assert_eq!(eval("100 W * 5 s").unwrap().to_display(), "500 J");
    assert_eq!(eval("12 V * 2 A").unwrap().to_display(), "24 W");
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_statistical_functions() {
    // Verifies built-in statistical functions operating on physical unit ranges and discrete arguments
    assert_eq!(eval("sum(1 m .. 5 m)").unwrap().to_display(), "15 m");
    assert_eq!(eval("mean(10 m .. 30 m)").unwrap().to_display(), "20 m");
    assert_eq!(
        eval("median(1 m, 10 m, 5 m, 20 m)").unwrap().to_display(),
        "7.5 m"
    );
    assert_eq!(
        eval("mode(2 m, 5 m, 2 m, 8 m)").unwrap().to_display(),
        "2 m"
    );
    assert_eq!(eval("min(3 m, 1 m, 5 m)").unwrap().to_display(), "1 m");
    assert_eq!(eval("max(3 m, 1 m, 5 m)").unwrap().to_display(), "5 m");
    assert_eq!(eval("range(1 m .. 10 m)").unwrap().to_display(), "9 m");
    assert_eq!(eval("iqr(1 m .. 5 m)").unwrap().to_display(), "2 m");
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_range_step_expansion() {
    // Tests explicit step expansion syntax in ranges (start..end..step)
    assert_eq!(eval("sum(1..9..2)").unwrap().to_display(), "25");
    assert_eq!(
        eval("mean(0 m .. 10 m .. 2 m)").unwrap().to_display(),
        "5 m"
    );
}

#[cfg(feature = "units")]
#[test]
fn test_trigonometric_functions() {
    // Tests standard trigonometric functions with radian inputs
    assert_eq!(eval("sin(0 rad)").unwrap().to_display(), "0");
    assert_eq!(eval("cos(0 rad)").unwrap().to_display(), "1");
    assert_eq!(eval("tan(0 rad)").unwrap().to_display(), "0");
}

#[cfg(feature = "units")]
#[test]
fn test_unparenthesized_single_parameter_functions() {
    let calc = Abacus::standard();

    // sin 13deg
    let sin_val = calc.eval_scalar("sin 13deg").unwrap().canonical;
    assert!((sin_val - 0.224951).abs() < 1e-5);

    // sin 13 deg
    let sin_val2 = calc.eval_scalar("sin 13 deg").unwrap().canonical;
    assert!((sin_val2 - 0.224951).abs() < 1e-5);

    // cos 0 rad
    assert_eq!(calc.eval("cos 0 rad").unwrap().to_display(), "1");

    // tan 0 rad
    assert_eq!(calc.eval("tan 0 rad").unwrap().to_display(), "0");

    // sqrt 16 m^2 -> 4 m
    assert_eq!(calc.eval("sqrt 16 m^2").unwrap().to_display(), "4 m");

    // ln 10
    let ln_val = calc.eval_scalar("ln 10").unwrap().canonical;
    assert!((ln_val - std::f64::consts::LN_10).abs() < 1e-5);

    // log10 100
    assert_eq!(calc.eval("log10 100").unwrap().to_display(), "2");

    // abs -5 m
    assert_eq!(calc.eval("abs -5 m").unwrap().to_display(), "5 m");

    // unparenthesized function in arithmetic: sin 13deg + 5
    let sin_plus = calc.eval_scalar("sin 13deg + 5").unwrap().canonical;
    assert!((sin_plus - 5.224951).abs() < 1e-5);
}

#[test]
fn test_logarithmic_and_exponential_functions() {
    // Tests natural log, log10, log2, base-N log, and exp
    assert_eq!(eval("ln(exp(1))").unwrap().to_display(), "1");
    assert_eq!(eval("log10(100)").unwrap().to_display(), "2");
    assert_eq!(eval("log2(8)").unwrap().to_display(), "3");
    assert_eq!(eval("log(81, 3)").unwrap().to_display(), "4");
}

#[test]
fn test_combinatorics() {
    // Tests factorial (!), combinations (nCr), and permutations (nPr)
    assert_eq!(eval("5!").unwrap().to_display(), "120");
    assert_eq!(eval("factorial(5)").unwrap().to_display(), "120");
    assert_eq!(eval("nCr(5, 2)").unwrap().to_display(), "10");
    assert_eq!(eval("nPr(5, 2)").unwrap().to_display(), "20");
}

#[cfg(all(feature = "units", feature = "distributions"))]
#[test]
fn test_probability_distributions() {
    // Tests continuous normal CDF and inverse cumulative distribution functions
    let result = Abacus::standard()
        .eval_scalar("normcdf(0)")
        .unwrap()
        .canonical;
    assert_eq!(result, 0.5);

    let norm_val = Abacus::standard()
        .eval_scalar("normcdf(70 kg, 65 kg, 5 kg)")
        .unwrap()
        .canonical;
    assert!((norm_val - 0.841_344_746_068_542_9).abs() < 1e-15);

    let invnorm_val = Abacus::standard()
        .eval_scalar("invnorm(0.975)")
        .unwrap()
        .canonical;
    assert!((invnorm_val - 1.959_963_984_540_054).abs() < 1e-14);
}

#[cfg(feature = "financial")]
#[test]
fn test_financial_functions() {
    // Tests loan payment PMT function
    let pmt_val = Abacus::standard()
        .eval_scalar("pmt(0.05 / 12, 360, 200000)")
        .unwrap()
        .canonical;
    assert!((pmt_val - (-1_073.643_246_024_279_5)).abs() < 1e-9);
}

#[cfg(feature = "units")]
#[test]
fn test_unary_increment_decrement() {
    // Tests prefix (++x, --x) and postfix (x++, x--) incrementers and decrementers

    // Dimensionless numbers
    assert_eq!(eval("++5").unwrap().to_display(), "6");
    assert_eq!(eval("5++").unwrap().to_display(), "6");
    assert_eq!(eval("--5").unwrap().to_display(), "4");
    assert_eq!(eval("5--").unwrap().to_display(), "4");

    // Physical values with units
    assert_eq!(eval("++5 m").unwrap().to_display(), "6 m");
    assert_eq!(eval("5 m++").unwrap().to_display(), "6 m");
    assert_eq!(eval("--5 m").unwrap().to_display(), "4 m");
    assert_eq!(eval("5 m--").unwrap().to_display(), "4 m");
    assert_eq!(eval("500 cm++").unwrap().to_display(), "501 cm");
}

#[cfg(feature = "units")]
#[test]
fn test_error_handling() {
    // Verifies incompatible dimension addition and unknown unit errors
    assert!(matches!(
        eval("5 m + 3 s"),
        Err(AbacusError::IncompatibleDimensions)
    ));

    assert!(matches!(
        eval("5 unknown_unit_xyz"),
        Err(AbacusError::UnknownUnit(_))
    ));
}

#[cfg(feature = "units")]
#[test]
fn test_inches_and_conversion_keyword_disambiguation() {
    let calc = Abacus::standard();

    // "14m to in" -> 551.1811 in
    let res1 = calc.eval("14m to in").unwrap();
    assert_eq!(res1.as_scalar().unwrap().unit.display.render(), "in");
    assert!((res1.as_scalar().unwrap().canonical - 14.0).abs() < 1e-6);

    // "14m in in" -> converts 14m to inches
    let res2 = calc.eval("14m in in").unwrap();
    assert_eq!(res2.as_scalar().unwrap().unit.display.render(), "in");

    // "14m as in" -> converts 14m to inches
    let res3 = calc.eval("14m as in").unwrap();
    assert_eq!(res3.as_scalar().unwrap().unit.display.render(), "in");

    // "10 in" -> 10 inches
    let res4 = calc.eval("10 in").unwrap();
    assert_eq!(res4.to_display(), "10 in");

    // "10 in to cm" -> 25.4 cm
    let res5 = calc.eval("10 in to cm").unwrap();
    assert_eq!(res5.to_display(), "25.4 cm");

    // "10 in in cm" -> 25.4 cm
    let res6 = calc.eval("10 in in cm").unwrap();
    assert_eq!(res6.to_display(), "25.4 cm");

    // "5 in + 2 in" -> 7 in
    let res7 = calc.eval("5 in + 2 in").unwrap();
    assert_eq!(res7.to_display(), "7 in");

    // "10 in / 2 s" -> 5 in/s
    let res8 = calc.eval("10 in / 2 s").unwrap();
    assert_eq!(res8.to_display(), "5 in/s");
}
