//! Regression tests for parser/lexer findings from the codebase audit.
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

use abacus::{
    Abacus, AbacusError, BinaryOp, Date, Token, TokenRegistry, UnitRegistry, Value, eval,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn display(expr: &str) -> String {
    eval(expr)
        .unwrap_or_else(|e| panic!("{expr}: {e:?}"))
        .to_display()
}

fn add_ten(lhs: Value, rhs: Value) -> Result<Value, AbacusError> {
    let sum = (&lhs + &rhs)?;
    Ok(Value::new(sum.canonical + 10.0, sum.unit))
}

// ── Lexer performance: no O(n^2) rescans of the remaining input ──

#[test]
fn long_operator_chains_lex_in_linear_time() {
    let start = Instant::now();
    let expr = format!("{}1", "1+".repeat(100_000));
    assert!(eval(&expr).is_ok());
    let expr = format!("{}1", "-".repeat(100_000));
    assert_eq!(eval(&expr), Err(AbacusError::RecursionLimitExceeded));
    // Previously ~5 s (release) / ~54 s (debug); now milliseconds. Generous bound for slow CI.
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn operator_index_is_invalidated_on_registration() {
    let mut calc = Abacus::standard();
    assert_eq!(calc.eval("1 + 1").unwrap().to_display(), "2");
    calc.register_binop_token(BinaryOp {
        alias: "@@",
        func: add_ten,
        precedence: 0,
        right_associative: false,
    });
    assert_eq!(calc.eval("1 @@ 2").unwrap().to_display(), "13");

    // Direct mutation of the public map is detected as well.
    calc.tokens.binary_operators.insert(
        "##".to_string(),
        Arc::new(BinaryOp {
            alias: "##",
            func: add_ten,
            precedence: 0,
            right_associative: false,
        }),
    );
    assert_eq!(calc.eval("1 ## 2").unwrap().to_display(), "13");
}

#[test]
fn registry_key_overrides_operator_alias() {
    let mut tokens = TokenRegistry::standard();
    tokens.register_binary_operator(
        "plus",
        BinaryOp {
            alias: "myplus",
            func: add_ten,
            precedence: 0,
            right_associative: false,
        },
    );
    let units = UnitRegistry::standard();
    let res = abacus::evaluation::parser::evaluate(&tokens, &units, "2 plus 3").unwrap();
    assert_eq!(res.to_display(), "15");
}

#[test]
fn binary_minus_wins_equal_length_tie_deterministically() {
    let calc = Abacus::standard();
    let toks = calc.tokenize("3-2").unwrap();
    assert_eq!(
        toks,
        vec![Token::Float(3.0), Token::BinaryOp("-"), Token::Float(2.0)]
    );
}

// ── Indefinite articles vs ampere ──

#[cfg(feature = "units")]
#[test]
fn ampere_is_never_swallowed_as_one() {
    assert_eq!(display("10 V / A"), "10 Ω");
    assert_eq!(display("1 kW / 230 V in A"), "4.3478260869565215 A");
    assert_eq!(display("(2+3) A"), "5 A");
    assert_eq!(display("2 * A"), "2 A");
}

#[cfg(all(feature = "units", feature = "number-scales", feature = "currencies"))]
#[test]
fn articles_still_mean_one_or_per() {
    assert_eq!(display("A million dollars"), "$1000000");
    assert_eq!(display("an hour in minutes"), "60 min");
    assert_eq!(display("40 as a % of 200"), "20%");
    assert_eq!(display("5 usd a second"), "5 $/s");
    assert_eq!(display("$50 an hour"), display("$50 / hour"));
}

// ── Conversions: rate x duration only with an explicit duration ──

#[cfg(feature = "units")]
#[test]
fn conversion_to_bare_time_unit_is_not_accumulation() {
    for expr in ["5 m/s in s", "1 kHz in ms", "10 Hz in min"] {
        assert_eq!(
            eval(expr),
            Err(AbacusError::IncompatibleDimensions),
            "{expr}"
        );
    }
}

#[cfg(all(feature = "units", feature = "currencies"))]
#[test]
fn rate_accumulation_over_explicit_duration() {
    assert_eq!(display("5 usd per second in 20 days"), "$8640000");
    assert_eq!(display("10 meters a second in 5 seconds"), "50 m");
    // No time unit to cancel in the denominator: fall back to multiplication with a
    // display that matches the dimensions.
    let res = eval("1 kHz in 2 s").unwrap().into_scalar().unwrap();
    assert!(res.unit.is_dimensionless());
    assert!((res.canonical - 2000.0).abs() < 1e-9);
}

// ── Recursion limits and stack safety ──

#[test]
fn recursion_depth_setter_is_clamped() {
    let calc = Abacus::standard().with_max_recursion_depth(usize::MAX);
    assert_eq!(
        calc.max_recursion_depth,
        abacus::evaluation::parser::pratt::MAX_RECURSION_DEPTH_CAP
    );
}

#[test]
fn deep_nesting_never_overflows_a_small_thread_stack() {
    let calc = Abacus::standard().with_max_recursion_depth(usize::MAX);
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            for n in [255usize, 6_000] {
                let inputs = [
                    format!("{}1{}", "(".repeat(n), ")".repeat(n)),
                    format!("{}1{}", "sqrt(".repeat(n), ")".repeat(n)),
                    format!("{}1{}", "max(".repeat(n), ")".repeat(n)),
                    format!("{}1{}", "[".repeat(n), ", 2]".repeat(n)),
                    format!("{}1", "2^".repeat(n)),
                    format!("{}1", "-".repeat(n)),
                ];
                for input in inputs {
                    assert!(calc.eval(&input).is_err());
                }
            }
        })
        .unwrap();
    handle.join().expect("parser overflowed a 2 MiB stack");
}

#[cfg(feature = "stats")]
#[test]
fn range_element_limit_is_total_not_per_range() {
    let expr = format!("sum({}1)", "1..99999,".repeat(100));
    assert_eq!(eval(&expr), Err(AbacusError::IncompatibleFunctionArguments));
    assert_eq!(display("sum(1..100, 1..100)"), "10100");
}

// ── Juxtaposed numbers and compound measurements ──

#[cfg(feature = "units")]
#[test]
fn adjacent_bare_numbers_are_errors() {
    for expr in ["1 000", "2 3", "1.2.3", "5 m 3", "2 3 m", "1/2 3/4"] {
        assert!(
            matches!(eval(expr), Err(AbacusError::UnexpectedToken(_))),
            "{expr} should be rejected"
        );
    }
}

#[cfg(feature = "units")]
#[test]
fn compound_measurements_are_summed() {
    assert_eq!(display("5 ft 3 in"), "5.25 ft");
    assert_eq!(display("1 h 30 min"), "1.5 h");
    assert_eq!(display("1 h 30 min 15 s in min"), "90.25 min");
    // The compound value binds as a single operand.
    assert_eq!(display("5 ft 3 in * 2"), "10.5 ft");
    // Ascending or different-dimension pairs keep implicit multiplication.
    assert_eq!(display("2 m 3 s"), "6 m*s");
}

// ── Increment / decrement vs binary operators ──

#[test]
fn double_signs_between_operands_are_binary_then_unary() {
    assert_eq!(display("3--2"), "5");
    assert_eq!(display("5--3"), "8");
    assert_eq!(display("2++3"), "5");
    assert_eq!(display("+5"), "5");
}

#[test]
fn increment_binds_like_unary_minus() {
    assert_eq!(display("++2*3"), "9");
    assert_eq!(display("5 ++"), "6");
    assert_eq!(display("(5--)"), "4");
}

// ── Variables vs units ──

#[cfg(feature = "units")]
#[test]
fn variables_shadow_units_regardless_of_spacing() {
    let calc = Abacus::standard()
        .with_variable("h", 3.0)
        .with_variable("x", 4.0);
    assert_eq!(calc.eval("2h").unwrap().to_display(), "6");
    assert_eq!(calc.eval("2 h").unwrap().to_display(), "6");
    // Conversion targets always name units.
    assert_eq!(calc.eval("x in h").unwrap().to_display(), "4 h");
    assert_eq!(calc.eval("2 hours in min").unwrap().to_display(), "120 min");
}

// ── Percent vs modulo ──

#[cfg(feature = "units")]
#[test]
fn percent_and_modulo_ignore_whitespace() {
    for expr in ["10%3", "10 %3", "10% 3", "10 % 3", "10%(3)"] {
        assert_eq!(display(expr), "1", "{expr}");
    }
    assert_eq!(display("20%of 50"), "10");
    assert_eq!(display("50% of 200"), "100");
    assert_eq!(eval("10%").unwrap().to_display(), "10%");
}

// ── sqrt of negatives ──

#[cfg(feature = "units")]
#[test]
fn sqrt_of_negative_is_an_error_on_every_path() {
    for expr in ["sqrt(-9)", "sqrt -9", "sqrt(-9 m^2)"] {
        assert!(
            matches!(eval(expr), Err(AbacusError::EvaluationError(_))),
            "{expr}"
        );
    }
}

// ── Ranges ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn range_endpoints_in_functions_require_parentheses_for_loose_operators() {
    assert_eq!(display("1..2*3"), "3..6");
    assert!(matches!(
        eval("max(1..2*3)"),
        Err(AbacusError::UnexpectedToken(_))
    ));
    assert!(matches!(
        eval("max(1+2..3)"),
        Err(AbacusError::UnexpectedToken(_))
    ));
    assert_eq!(display("max(1..(2*3))"), "6");
    assert_eq!(display("sum(-5..5)"), "0");
    assert_eq!(display("max(1..2^3)"), "8");
    assert_eq!(display("mean(0 m .. 10 m .. 2 m)"), "5 m");
}

// ── Significant figures ──

#[test]
fn sig_fig_scan_skips_identifiers_and_exponents() {
    use abacus::min_significant_figures_in_expr as sf;
    assert_eq!(sf("123 * log10(456)"), Some(3));
    assert_eq!(sf("123 * atan2(456, 789)"), Some(3));
    assert_eq!(sf("2^(3.0) * 1.2345"), Some(5));
    assert_eq!(sf("2^-2 * 1.2345"), Some(5));
    let calc = Abacus::standard().with_follow_significant_figures(true);
    assert_eq!(calc.eval("123 * log10(456)").unwrap().to_display(), "327");
}

// ── Number literals ──

#[test]
fn overflowing_and_truncated_literals_are_errors() {
    assert!(matches!(eval("1e999"), Err(AbacusError::InvalidNumber(_))));
    assert!(matches!(eval("1e"), Err(AbacusError::InvalidNumber(_))));
    assert!(matches!(eval("2E+"), Err(AbacusError::InvalidNumber(_))));
    assert_eq!(display("4e2"), "400");
}

// ── Timezone offset literals ──

#[test]
fn timezone_offsets_only_follow_dates() {
    assert!(!matches!(
        eval("10 -1:30"),
        Err(AbacusError::UnknownUnit(_))
    ));
    assert_eq!(
        display("2026-08-07 10:00:00 -04:00 to +02:00"),
        "August 7, 2026 16:00:00 +02:00"
    );
}

// ── Miscellaneous quirks ──

#[cfg(all(feature = "units", feature = "currencies"))]
#[test]
fn trailing_conversion_keyword_and_stray_tokens_are_errors() {
    assert_eq!(eval("5 in in"), Err(AbacusError::UnexpectedEnd));
    assert_eq!(display("5 in"), "5 in");
    assert_eq!(display("5 in to cm"), "12.7 cm");
    assert!(matches!(eval("$5 $"), Err(AbacusError::UnexpectedToken(_))));
    assert!(matches!(
        eval("5 off"),
        Err(AbacusError::UnexpectedToken(_))
    ));
    assert_eq!(display("$100 - 20% off"), "$80");
}

#[test]
fn dot_property_tokens_borrow_input() {
    let calc = Abacus::standard().with_variable("x", 1.0);
    let toks = calc.tokenize("x.slope").unwrap();
    assert_eq!(toks, vec![Token::Ident("x"), Token::DotProperty("slope")]);
}

#[test]
fn tokenize_respects_anchor_date() {
    let calc = Abacus::standard().with_now(Date::new(2020, 6, 15));
    match calc.tokenize("tomorrow").unwrap().as_slice() {
        [Token::Date(d)] => assert_eq!((d.year, d.month, d.day), (2020, 6, 16)),
        other => panic!("unexpected tokens: {other:?}"),
    }
}

#[test]
fn precedence_unchanged_after_registry_driven_binding_powers() {
    assert_eq!(display("2^3!"), "64");
    assert_eq!(display("-3!"), "-6");
    assert_eq!(display("-2^2"), "-4");
    assert_eq!(display("2^3^2"), "512");
    assert_eq!(display("2 + 3 * 4"), "14");
    assert_eq!(display("10 - 4 - 3"), "3");
}
