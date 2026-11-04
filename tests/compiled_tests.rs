//! Compiled expressions (`Abacus::compile` / `eval_compiled`) and span diagnostics.
#![cfg(all(feature = "units", feature = "date", feature = "stats"))]

use abacus::{Abacus, AbacusError, Date};

fn anchored() -> Abacus {
    Abacus::standard().with_now("2026-10-21 09:30:00".parse::<Date>().unwrap())
}

#[test]
fn compiled_results_match_eval() {
    let calc = anchored().with_variable("x", 4.0);
    for src in [
        "1 + 2 * 3",
        "10 N * 5 m",
        "100 km/h in m/s",
        "[9.8 m, 10.2 m] / [1.9 s, 2.1 s]",
        "sqrt(2) * sin(30 deg)",
        "mean(1..10)",
        "x^2 + 2x",
        "$50 + €20 in USD",
        "5 ft 3 in to cm",
        "linreg(1, 2, 3, 4, 10, 20, 30, 40).slope",
        "$50 after 15% tax",
        "3 days ago",
        "today + 5 business days",
        "07-08-2026 to 08-08-2026",
        "20% off $120",
    ] {
        let expr = calc.compile(src).unwrap();
        assert_eq!(
            calc.eval_compiled(&expr).map(|r| r.to_display()),
            calc.eval(src).map(|r| r.to_display()),
            "{src}"
        );
    }
}

#[test]
fn compiled_expressions_see_new_variable_values() {
    let mut calc = Abacus::standard().with_variable("x", 1.0);
    let expr = calc.compile("x * 10 m").unwrap();
    calc.set_variable("x", 2.0);
    assert_eq!(calc.eval_compiled(&expr).unwrap().to_display(), "20 m");
    calc.remove_variable("x");
    assert_eq!(
        calc.eval_compiled(&expr),
        Err(AbacusError::UnknownUnit("x".to_string()))
    );
}

#[test]
fn stale_compiled_expressions_are_reread() {
    let mut calc = Abacus::standard();
    // `h` is the hour when compiled ...
    let expr = calc.compile("2 h").unwrap();
    assert_eq!(calc.eval_compiled(&expr).unwrap().to_display(), "2 h");
    // ... and a variable shadowing it afterwards is honoured, as `eval` would.
    calc.set_variable("h", 3.0);
    assert_eq!(calc.eval_compiled(&expr).unwrap().to_display(), "6");

    // Settings that change how input is read are honoured too.
    let implicit = calc.compile("2(3)").unwrap();
    calc.set_implicit_multiplication(false);
    assert!(calc.eval_compiled(&implicit).is_err());

    // An expression compiled by another engine is evaluated by this one's rules.
    let other = Abacus::standard().with_variable("y", 5.0);
    let foreign = other.compile("y + 1").unwrap();
    assert_eq!(
        calc.eval_compiled(&foreign),
        Err(AbacusError::UnknownUnit("y".to_string()))
    );
    assert_eq!(other.eval_compiled(&foreign).unwrap().to_display(), "6");
}

#[test]
fn compiled_dates_follow_the_reference_time() {
    let mut calc = anchored();
    let expr = calc.compile("today + 1 day").unwrap();
    assert_eq!(
        calc.eval_compiled(&expr).unwrap().to_display(),
        "October 22, 2026"
    );
    calc.set_now("2027-01-01 00:00:00".parse::<Date>().unwrap());
    assert_eq!(
        calc.eval_compiled(&expr).unwrap().to_display(),
        "January 2, 2027"
    );
}

#[test]
fn compiled_expressions_apply_output_settings() {
    let mut calc = Abacus::standard();
    let expr = calc.compile("10 / 3").unwrap();
    calc.set_decimal_places(2);
    assert_eq!(calc.eval_compiled(&expr).unwrap().to_display(), "3.33");
    assert_eq!(expr.source(), "10 / 3");
}

#[test]
fn long_flat_expressions_do_not_deepen_the_tree() {
    let expr = vec!["1"; 50_000].join(" + ");
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || Abacus::standard().eval(&expr).map(|r| r.to_display()))
        .unwrap();
    assert_eq!(handle.join().unwrap(), Ok("50000".to_string()));
}

// ── Diagnostics ──

/// The error, the source text its span covers, and the note.
fn diagnose(src: &str) -> (AbacusError, String, Option<String>) {
    let d = Abacus::standard().eval_detailed(src).unwrap_err();
    let text = d
        .span()
        .map(|s| s.text(src).to_string())
        .unwrap_or_default();
    (d.error().clone(), text, d.note().map(str::to_string))
}

#[test]
fn diagnostics_point_at_the_problem() {
    assert_eq!(
        diagnose("5 m + 3 s"),
        (
            AbacusError::IncompatibleDimensions,
            "+".into(),
            Some("m + s".into())
        )
    );
    assert_eq!(
        diagnose("2 * foo(3)"),
        (
            AbacusError::UnknownFunction("foo".into()),
            "foo".into(),
            None
        )
    );
    assert_eq!(
        diagnose("1 +* 2"),
        (AbacusError::UnexpectedToken("*".into()), "*".into(), None)
    );
    assert_eq!(
        diagnose("sqrt(4, 5)"),
        (AbacusError::UnexpectedToken(",".into()), ",".into(), None)
    );
    assert_eq!(
        diagnose("3 # 4"),
        (AbacusError::UnexpectedCharacter('#'), "#".into(), None)
    );
    assert_eq!(
        diagnose("sqrt(-4) + 1").1,
        "sqrt(-4)",
        "evaluation errors point at the failing subexpression"
    );
    let (error, text, _) = diagnose("2 * (3");
    assert_eq!((error, text), (AbacusError::UnclosedParen, String::new()));
}

#[test]
fn diagnostics_render_a_caret() {
    let d = Abacus::standard().eval_detailed("5 m + 3 s").unwrap_err();
    assert_eq!(
        d.to_string(),
        "cannot perform operation on incompatible dimensions\n  5 m + 3 s\n      ^\n  note: m + s"
    );
    // The plain error is unchanged.
    assert_eq!(
        Abacus::standard().eval("5 m + 3 s"),
        Err(AbacusError::IncompatibleDimensions)
    );
}
