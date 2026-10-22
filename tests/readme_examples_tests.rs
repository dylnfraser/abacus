//! Every example output shown in README.md, asserted against the engine.
//!
//! If one of these fails, either the engine regressed or the README needs updating — keep the
//! two in sync. Relative-date examples use a fixed clock (Tuesday 2026-09-01 09:00) and
//! currency examples use the bundled fallback rates, so the suite is deterministic.
#![cfg(all(
    feature = "units",
    feature = "stats",
    feature = "distributions",
    feature = "date",
    feature = "financial",
    feature = "number-scales",
    feature = "currencies"
))]

use abacus::{
    Abacus, AngleMode, Date, DateFormat, IntervalStyle, Notation, TimeZone, WeekendDays, eval,
};

/// Asserts each `(expression, expected display)` pair on `calc`.
fn check_all(calc: &Abacus, cases: &[(&str, &str)]) {
    let mut failures = Vec::new();
    for &(expr, expected) in cases {
        match calc.eval(expr) {
            Ok(res) if res.to_display() == expected => {}
            Ok(res) => failures.push(format!(
                "`{expr}`: expected `{expected}`, got `{}`",
                res.to_display()
            )),
            Err(err) => failures.push(format!("`{expr}`: expected `{expected}`, got {err:?}")),
        }
    }
    assert!(
        failures.is_empty(),
        "README mismatches:\n{}",
        failures.join("\n")
    );
}

fn anchored() -> Abacus {
    Abacus::standard().with_now("2026-09-01 09:00:00".parse::<Date>().unwrap())
}

#[test]
fn quickstart() {
    let show = |e: &str| eval(e).unwrap().to_display();
    assert_eq!(show("10 N * 5 m"), "50 J");
    assert_eq!(show("100 km/h in m/s"), "27.77777777777778 m/s");
    assert_eq!(show("1..10 + 5"), "6..15");
    assert_eq!(
        show("[9.8 m, 10.2 m] / [1.9 s, 2.1 s]"),
        "[4.666666666666666 m/s, 5.36842105263158 m/s]"
    );
    assert_eq!(show("07-08-2026 + 5 business days"), "August 14, 2026");
    assert_eq!(show("$50 + €20 in USD"), "$72.43");
    assert_eq!(show("a thousand dollars per day * 3 days"), "$3000");
    assert_eq!(show("$3 million in EUR"), "2674980 EUR");
    assert_eq!(
        show("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s"),
        "50 m"
    );
    // Float-noise note below the quickstart.
    assert_eq!(show("0.1 + 0.2"), "0.3");
    assert_eq!(show("5 lb to kg"), "2.26796185 kg");
    assert_eq!(
        show("[0.1, 0.1] + [0.2, 0.2]"),
        "[0.3, 0.30000000000000004]"
    );
}

#[test]
fn engine_configuration() {
    let deg = Abacus::standard().with_angle_mode(AngleMode::Degrees);
    check_all(&deg, &[("sin(90)", "1"), ("asin(1)", "90 deg")]);

    let strict = Abacus::standard().with_strict_dimensions(true);
    assert!(strict.eval("5 m + 5").is_err());

    let dec = Abacus::standard().with_decimal_places(2);
    assert_eq!(dec.eval("10 / 3").unwrap().to_display(), "3.33");
    assert_eq!(dec.format_result(&dec.eval("5").unwrap()), "5.00");

    let range = Abacus::standard().with_interval_style(IntervalStyle::Range);
    check_all(&range, &[("[1 m, 5 m] + 2 m", "3 m..7 m")]);

    let eng = Abacus::standard().with_notation(Notation::Engineering);
    assert_eq!(eng.format_result(&eng.eval("45000").unwrap()), "45e3");
    assert_eq!(eng.format_result(&eng.eval("0.045").unwrap()), "45e-3");

    let tz = Abacus::standard().with_default_timezone(TimeZone::parse("EST").unwrap());
    check_all(&tz, &[("07-08-2026 10:00", "August 7, 2026 10:00:00 EST")]);

    let gulf = Abacus::standard().with_weekend(WeekendDays::FridaySaturday);
    check_all(&gulf, &[("06-08-2026 + 1 business day", "August 9, 2026")]);

    let _depth = Abacus::standard().with_max_recursion_depth(32);

    let explicit = Abacus::standard().with_implicit_multiplication(false);
    assert!(explicit.eval("2(3)").is_err());

    let sig = Abacus::standard().with_significant_figures(3);
    check_all(&sig, &[("12.3456 m", "12.3 m")]);
    let follow = Abacus::standard().with_follow_significant_figures(true);
    check_all(&follow, &[("12.3 * 4.567", "56.2")]);

    let raw = Abacus::standard().with_auto_derived_units(false);
    check_all(&raw, &[("10 N * 5 m", "50 N*m")]);

    let curr = Abacus::standard()
        .with_currencies(true)
        .with_currency_rate("EUR", 0.50);
    check_all(&curr, &[("$100 in EUR", "50 EUR")]);

    let scales = Abacus::standard().with_number_scales(true);
    check_all(&scales, &[("$3 million", "$3000000")]);

    let mut vars = Abacus::standard();
    assert_eq!(
        vars.eval("2 * pi * 5").unwrap().to_display(),
        "31.41592653589793"
    );
    let radius = vars.eval("5 m").unwrap();
    vars.set_variable("radius", radius);
    assert_eq!(
        vars.eval("pi * radius^2").unwrap().to_display(),
        "78.53981633974483 m^2"
    );
    assert_eq!(vars.eval_mut("height = 10 m").unwrap().to_display(), "10 m");
    assert_eq!(vars.eval("radius * height").unwrap().to_display(), "50 m^2");

    let speed = Abacus::standard().with_common_speed_overrides();
    check_all(
        &speed,
        &[
            ("60 miles per hour", "60 mph"),
            ("100 km / 1 h", "100 kmph"),
        ],
    );

    let us = Abacus::standard().with_date_format(DateFormat::MMDDYYYY);
    check_all(&us, &[("08-07-2026", "08-07-2026")]);
    // Ambiguous input is month-first under MMDDYYYY: this is August 7.
    assert_eq!(us.eval("month(08-07-2026)").unwrap().to_display(), "8");
}

#[test]
fn physical_units_and_naming() {
    check_all(
        &Abacus::standard(),
        &[
            ("5 m + 3 m", "8 m"),
            ("(5 m + 20 cm) as m", "5.2 m"),
            ("10 N * 5 m", "50 J"),
            ("100 W * 5 s", "500 J"),
            ("12 V * 2 A", "24 W"),
            ("5 km to m", "5000 m"),
            ("1 m in inches", "39.37007874015748 in"),
            ("5 km / m", "5000"),
            ("5 ft 3 in", "5.25 ft"),
            ("100 °C to °F", "212 °F"),
            ("-40 °F in °C", "-40 °C"),
            ("20 °C - 15 °C", "5 K"),
            ("1 kWh in J", "3600000 J"),
            // Naming conventions table.
            ("5 pounds to kg", "2.26796185 kg"),
            ("1 lbs in lb", "1 lb"),
            ("1 gal in L", "3.785411784 L"),
            ("1 imp_gal in L", "4.54609 L"),
            ("1 imperial gallon in L", "4.54609 L"),
            ("1 ton in kg", "907.18474 kg"),
            ("1 long_ton in kg", "1016.0469088 kg"),
            ("1 long ton in kg", "1016.0469088 kg"),
            ("1 t in kg", "1000 kg"),
            ("1 tonne in kg", "1000 kg"),
            ("1 metric ton in kg", "1000 kg"),
            ("1 cal in J", "4.184 J"),
            ("1 kcal in J", "4184 J"),
            ("1 cal_IT in J", "4.1868 J"),
            ("1 hp in W", "745.6998715822702 W"),
            ("1 hp_metric in W", "735.49875 W"),
            ("5 quid", "5 GBP"),
            ("10 V / A", "10 Ω"),
            ("1 atm", "1 atm"),
            ("3 GHz", "3 GHz"),
        ],
    );
    assert!(eval("1 as").is_err());
}

#[test]
fn currencies() {
    check_all(
        &Abacus::standard(),
        &[
            ("$100", "$100"),
            ("-$50.25", "-$50.25"),
            ("$50 + $25", "$75"),
            ("$100 / 3", "$33.33"),
            ("100 USD in EUR", "89.17 EUR"),
            ("$50 + €20 in USD", "$72.43"),
            ("50 EUR in JPY", "8867 JPY"),
            ("20 pounds sterling in USD", "$26.69"),
            ("$100 / 2 hours", "50 $/h"),
            ("(10 EUR / L) * 5 L", "50 EUR"),
        ],
    );
}

#[test]
fn number_scales_articles_and_per() {
    check_all(
        &Abacus::standard(),
        &[
            ("$3 million", "$3000000"),
            ("5 billion USD / 2 million", "$2500"),
            ("a million dollars", "$1000000"),
            ("a dozen", "12"),
            ("a dollar", "$1"),
            ("an hour in minutes", "60 min"),
            ("a thousand dollars per day", "1000 $/d"),
            ("5 usd a second", "5 $/s"),
            ("60 miles an hour", "60 mi/h"),
            ("5 usd per second in 20 days", "$8640000"),
            ("5 usd a second in 20 days", "$8640000"),
            ("$50 an hour in 40 hours", "$2000"),
            ("(a thousand dollars per day) * 3 days", "$3000"),
            ("100 meters per second", "100 m/s"),
            ("10 meters a second in 5 seconds", "50 m"),
            ("60 miles per hour in km/h", "96.56064 km/h"),
            ("$50 per hour * 8 hours", "$400"),
            ("100 per 4", "25"),
        ],
    );
}

#[test]
fn percentages() {
    check_all(
        &Abacus::standard(),
        &[
            ("20% off $120", "$96"),
            ("15% off 200 EUR", "170 EUR"),
            ("$100 - 20% off", "$80"),
            ("$85 + 18% tip", "$100.30"),
            ("$50 after 15% tax", "$57.50"),
            ("$50 after 15%", "$57.50"),
            ("$100 after 20% discount", "$80"),
            ("40 as a % of 200", "20%"),
            ("40 out of 200 as %", "20%"),
            ("40 out of 200", "0.2"),
            ("3 out of 5 in %", "60%"),
            ("% change from 50 to 75", "+50%"),
            ("% change from 100 to 80", "-20%"),
            ("percent change from $80 to $100", "+25%"),
            ("30% more than 50 kg", "65 kg"),
            ("5 kg more than 50 kg", "55 kg"),
            ("15% less than 2 hours in minutes", "102 min"),
            ("15% less than 2 hours", "1.7 h"),
            ("50% + 50%", "100%"),
            ("(% change from 50 to 70) + 50%", "90%"),
            ("50% of 200", "100"),
            ("10 % 3", "1"),
        ],
    );
}

#[test]
fn constants_and_variables() {
    check_all(
        &Abacus::standard(),
        &[
            ("2 * pi * 5", "31.41592653589793"),
            ("e^2", "7.3890560989306495"),
            ("tau", "6.283185307179586"),
            ("phi", "1.618033988749895"),
        ],
    );
    let mut calc = Abacus::standard();
    let r = calc.eval("5 m").unwrap();
    calc.set_variable("r", r);
    assert_eq!(
        calc.eval("pi * r^2").unwrap().to_display(),
        "78.53981633974483 m^2"
    );
    assert_eq!(calc.eval_mut("w = 10 m").unwrap().to_display(), "10 m");
    // Variables shadow units with or without a space; conversion targets are units.
    calc.eval_mut("h = 3").unwrap();
    assert_eq!(calc.eval("2h").unwrap().to_display(), "6");
    assert_eq!(calc.eval("2 h").unwrap().to_display(), "6");
    assert_eq!(calc.eval("7200 s in h").unwrap().to_display(), "2 h");
}

#[test]
fn display_overrides() {
    let std = Abacus::standard();
    let over = Abacus::standard()
        .with_common_speed_overrides()
        .with_unit_display_override("$/d", "$/day");
    let rows = [
        ("60 miles per hour", "60 mi/h", "60 mph"),
        ("100 km / 1 h", "100 km/h", "100 kmph"),
        (
            "60 miles per hour in km/h",
            "96.56064 km/h",
            "96.56064 kmph",
        ),
        (
            "[50 mi/h, 70 mi/h]",
            "[50 mi/h, 70 mi/h]",
            "[50 mph, 70 mph]",
        ),
        ("a thousand dollars per day", "1000 $/d", "1000 $/day"),
    ];
    for (expr, plain, overridden) in rows {
        assert_eq!(std.eval(expr).unwrap().to_display(), plain, "{expr}");
        assert_eq!(over.eval(expr).unwrap().to_display(), overridden, "{expr}");
    }
    let manual = Abacus::standard()
        .with_unit_display_override("mi/h", "mph")
        .with_unit_display_override("km/h", "kmph");
    assert_eq!(
        manual.eval("60 miles per hour").unwrap().to_display(),
        "60 mph"
    );
}

#[test]
fn intervals() {
    check_all(
        &Abacus::standard(),
        &[
            ("[1 m, 2 m] + [3 m, 4 m]", "[4 m, 6 m]"),
            ("1..10 + 5", "6..15"),
            ("[10 m, 20 m] - [2 m, 5 m]", "[5 m, 18 m]"),
            ("[9.8 m, 10.2 m] / 2 s", "[4.9 m/s, 5.1 m/s]"),
            ("[-2 m, 4 m] * [1 s, 3 s]", "[-6 m*s, 12 m*s]"),
            ("[-2, 3]^2", "[0, 9]"),
            ("1 / [-1, 2]", "[-inf, inf]"),
            ("[0.1, 0.1] + [0.2, 0.2]", "[0.3, 0.30000000000000004]"),
            ("[1, 2] - [1, 2]", "[-1, 1]"),
            ("max(1..(2*3))", "6"),
        ],
    );
}

#[test]
fn dates_relative_time_and_timezones() {
    check_all(
        &anchored(),
        &[
            ("07-08-2026 + 3 weeks", "August 28, 2026"),
            ("07-08-2026 + 5 business days", "August 14, 2026"),
            ("10-08-2026 - 1 business day", "August 7, 2026"),
            ("workdays(07-08-2026, 14-08-2026)", "5 workdays"),
            ("2026-01-31 + 1 month", "February 28, 2026"),
            ("2024-02-29 + 1 year", "February 28, 2025"),
            ("17-08-2026 - 07-08-2026", "10 d"),
            ("1 year in days", "365.2425 d"),
            ("now", "September 1, 2026 09:00:00"),
            ("now + 2 hours", "September 1, 2026 11:00:00"),
            ("today", "September 1, 2026"),
            ("today at 12 to 1", "1 h"),
            ("11pm to 1am", "2 h"),
            (
                "last thursday at 3pm + 2 weeks",
                "September 10, 2026 15:00:00",
            ),
            ("3 days ago", "August 29, 2026 09:00:00"),
            ("in 45 minutes", "September 1, 2026 09:45:00"),
            ("next friday", "September 4, 2026"),
            ("15:30:00 EST to PST", "September 1, 2026 12:30:00 PST"),
            ("3:30 PM + 45 minutes", "September 1, 2026 16:15:00"),
            (
                "07-08-2026 10:00 AM EST to PST",
                "August 7, 2026 07:00:00 PST",
            ),
            (
                "2026-08-07 10:00 +0530 in UTC",
                "August 7, 2026 04:30:00 UTC",
            ),
            (
                "2026-08-07 10:00 -0400 in UTC",
                "August 7, 2026 14:00:00 UTC",
            ),
        ],
    );
    assert!(eval("2026-02-30").is_err());
}

#[test]
fn events() {
    check_all(
        &anchored(),
        &[
            ("days until christmas", "115 d"),
            ("business days until end of quarter", "21 business_days"),
            ("days until end of quarter", "29 d"),
            ("third thursday of november 2026", "November 19, 2026"),
            ("last friday of october 2026", "October 30, 2026"),
            ("first monday of january 2027", "January 4, 2027"),
            ("days until third thursday of november 2026", "79 d"),
            (
                "business days until third thursday of november 2026",
                "57 business_days",
            ),
            ("end of quarter", "September 30, 2026"),
            ("start of quarter", "July 1, 2026"),
            ("end of next quarter", "December 31, 2026"),
            ("end of q1 2027", "March 31, 2027"),
            ("end of month", "September 30, 2026"),
            ("end of year", "December 31, 2026"),
            ("thanksgiving 2026", "November 26, 2026"),
            ("black friday 2026", "November 27, 2026"),
            ("cyber monday 2026", "November 30, 2026"),
            ("easter 2026", "April 5, 2026"),
            ("until christmas", "115 d"),
            ("(days until christmas) in weeks", "16.428571428571427 wk"),
        ],
    );
}

#[test]
fn statistics_and_distributions() {
    check_all(
        &Abacus::standard(),
        &[
            ("mean(1..10)", "5.5"),
            ("median(10, 20, 30, 40, 50)", "30"),
            ("std(1..9..2)", "3.1622776601683795"),
            ("std_p(1..9..2)", "2.8284271247461903"),
            ("iqr(1..100)", "49.5"),
            ("skew(1, 2, 3, 10)", "1.7636326148038877"),
            ("mean(10 degC, 20 degC)", "15 °C"),
            ("std(10 degC, 20 degC)", "7.0710678118654755 K"),
            ("normpdf(0, 0, 1)", "0.39894228040143265"),
            ("normcdf(1.96, 0, 1)", "0.9750021048517794"),
            ("invnorm(0.975, 0, 1)", "1.9599639845400538"),
            ("poissonpdf(3, 2)", "0.22404180765538775"),
            ("poissonpdf(200, 180)", "0.010556778614080506"),
            ("binompdf(2000, 0.5, 1000)", "0.017839011145854337"),
            ("chisqcdf(1e6, 1e6)", "0.5001880631966019"),
        ],
    );
    assert!(eval("factorial(2.5)").is_err());
}

#[test]
fn hypothesis_tests_and_intervals() {
    check_all(
        &Abacus::standard(),
        &[
            (
                "ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)",
                "{ mean: 13 m, n: 4, p: 0.0026997960632601905, p_value: 0.0026997960632601905, z: 3 }",
            ),
            (
                "ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m).p_value",
                "0.0026997960632601905",
            ),
            ("TTest(100 m, 105 m, 15 m, 25).p", "0.10858012302472296"),
            (
                "TInterval(10 m, 12 m, 11 m, 14 m)",
                "[9.032469116203984 m, 14.467530883796016 m]",
            ),
            (
                "TInterval(10 m, 12 m, 11 m, 14 m, 90%)",
                "[9.74043339576468 m, 13.75956660423532 m]",
            ),
            (
                "ZInterval(100 m, 15 m, 100)",
                "[97.06005402318992 m, 102.93994597681008 m]",
            ),
            (
                "1-PropZInt(45, 100)",
                "[0.3524930229100607, 0.5475069770899393]",
            ),
        ],
    );
}

#[test]
fn regression_and_financial() {
    check_all(
        &Abacus::standard(),
        &[
            (
                "linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)",
                "{ intercept: 0 m, intercept_se: 0 m, mean_x: 2.5 s, mean_y: 25 m, r: 1, r2: 1, se: 0 m, slope: 10 m/s, slope_se: 0 m/s }",
            ),
            (
                "linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 10 s",
                "100 m",
            ),
            ("predict(5, 1, 2, 3, 2, 4, 6)", "10"),
            ("linreg_predict(1, 2, 3, 2, 4, 6, 5)", "10"),
            ("pmt(0.05/12, 360, 200000)", "-1073.6432460242781"),
            ("npv(0.1, -100, 50, 60)", "-4.507888805409458"),
            ("irr(-100, 60, 60)", "0.13066238629180754"),
        ],
    );
}

#[test]
fn command_line_examples() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let bin = env!("CARGO_BIN_EXE_abacus");

    let out = Command::new(bin)
        .args(["10 N * 5 m", "1 km in m"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "50 J\n1000 m\n");

    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"2+2\n5 ft 3 in\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "4\n5.25 ft\n");

    let out = Command::new(bin).args(["--", "-5 + 2"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "-3\n");

    let out = Command::new(bin).arg("1 m + 1 s").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: cannot perform operation on incompatible dimensions (in `1 m + 1 s`)\n"
    );
}
