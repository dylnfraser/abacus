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
    Abacus, AbacusError, BinaryOp, EvalResult, FunctionOp, FunctionTarget, Hash, Token, UnaryOp,
    Value,
};

#[cfg(feature = "units")]
#[test]
fn test_tokenize_method() {
    let calc = Abacus::standard();
    let tokens = calc.tokenize("5 m + 3 m").unwrap();

    assert_eq!(
        tokens,
        vec![
            Token::Val(calc.eval_scalar("5 m").unwrap()),
            Token::BinaryOp("+"),
            Token::Val(calc.eval_scalar("3 m").unwrap()),
        ]
    );
}

#[cfg(feature = "units")]
#[test]
fn test_register_custom_function() {
    let mut calc = Abacus::standard();

    fn double_val(args: &[Value]) -> Result<Value, AbacusError> {
        let val = &args[0];
        Ok(Value::new(val.canonical * 2.0, val.unit.clone()))
    }

    calc.register_function_token(FunctionOp {
        name: "dbl",
        min_args: 1,
        max_args: 1,
        func: FunctionTarget::Scalar(double_val),
    });

    assert_eq!(calc.eval("dbl(10 m)").unwrap().to_display(), "20 m");
}

#[cfg(feature = "units")]
#[test]
fn test_register_custom_hash_function_with_dynamic_dot_property() {
    let mut calc = Abacus::standard();

    fn custom_stats(args: &[Value]) -> Result<EvalResult, AbacusError> {
        let val = &args[0];
        let mut hash = Hash::new();
        hash.insert("original", val.clone());
        hash.insert(
            "my_custom_key",
            Value::new(val.canonical * 10.0, val.unit.clone()),
        );
        Ok(EvalResult::Hash(hash))
    }

    calc.register_function_token(FunctionOp {
        name: "my_stats",
        min_args: 1,
        max_args: 1,
        func: FunctionTarget::EvalResult(custom_stats),
    });

    // The parser accepts any key after `.`; the hash decides what exists.
    assert_eq!(
        calc.eval("my_stats(5 m).my_custom_key")
            .unwrap()
            .to_display(),
        "50 m"
    );
    assert_eq!(
        calc.eval("my_stats(5 m).original").unwrap().to_display(),
        "5 m"
    );
}

#[test]
fn test_register_custom_binary_operator() {
    let mut calc = Abacus::standard();

    fn add_ten(lhs: Value, rhs: Value) -> Result<Value, AbacusError> {
        let sum = (&lhs + &rhs)?;
        Ok(Value::new(sum.canonical + 10.0, sum.unit))
    }

    calc.register_binop_token(BinaryOp {
        alias: "@",
        func: add_ten,
        precedence: 10,
        right_associative: false,
    });

    assert_eq!(calc.eval("5 @ 3").unwrap().to_display(), "18");
}

#[test]
fn test_register_multi_char_binary_operator_plus_plus() {
    // `++` must tokenize as one operator, not two `+`.
    let mut calc = Abacus::standard();

    fn add_twenty(lhs: Value, rhs: Value) -> Result<Value, AbacusError> {
        let sum = (&lhs + &rhs)?;
        Ok(Value::new(sum.canonical + 20.0, sum.unit))
    }

    calc.register_binop_token(BinaryOp {
        alias: "++",
        func: add_twenty,
        precedence: 10,
        right_associative: false,
    });

    let tokens = calc.tokenize("5 ++ 3").unwrap();
    assert_eq!(
        tokens,
        vec![Token::Float(5.0), Token::BinaryOp("++"), Token::Float(3.0),]
    );

    assert_eq!(calc.eval("5 ++ 3").unwrap().to_display(), "28");
}

#[test]
fn test_register_custom_unary_operator() {
    let mut calc = Abacus::standard();

    fn triple(val: Value) -> Result<Value, AbacusError> {
        Ok(Value::new(val.canonical * 3.0, val.unit))
    }

    calc.register_unop_token(UnaryOp {
        alias: "~",
        func: triple,
        precedence: 20,
        prefix: true,
    });

    assert_eq!(calc.eval("~ 5").unwrap().to_display(), "15");
}

// Function context

#[cfg(feature = "units")]
#[test]
fn overriding_a_builtin_function_gets_raw_arguments() {
    // Angle-mode handling belongs to the built-in `sin`, not to the name.
    fn identity(args: &[Value]) -> Result<Value, AbacusError> {
        Ok(Value::dimensionless(args[0].canonical))
    }
    let mut calc = Abacus::standard().with_angle_mode(abacus::AngleMode::Degrees);
    assert_eq!(calc.eval("sin(90)").unwrap().to_display(), "1");
    calc.register_function_token(FunctionOp::scalar("sin", 1, 1, identity));
    assert_eq!(calc.eval("sin(90)").unwrap().to_display(), "90");
}

#[test]
fn context_functions_see_engine_settings_and_capture_state() {
    let offset = 10.0;
    let mut calc = Abacus::standard().with_angle_mode(abacus::AngleMode::Gradians);
    calc.register_function_token(FunctionOp::with_context("probe", 1, 1, move |ctx, args| {
        let mode = match ctx.angle_mode {
            abacus::AngleMode::Gradians => 1.0,
            _ => 0.0,
        };
        let x = args[0].as_scalar()?.canonical;
        Ok(EvalResult::Scalar(Value::dimensionless(x + offset + mode)))
    }));
    assert_eq!(calc.eval("probe(1)").unwrap().to_display(), "12");
}

#[cfg(feature = "date")]
#[test]
fn business_day_functions_use_the_configured_weekend() {
    let gulf = Abacus::standard().with_weekend(abacus::WeekendDays::FridaySaturday);
    // 2026-08-07 is a Friday, 2026-08-09 a Sunday.
    assert_eq!(
        gulf.eval("is_workday(2026-08-07)").unwrap().to_display(),
        "0"
    );
    assert_eq!(
        gulf.eval("is_weekend(2026-08-09)").unwrap().to_display(),
        "0"
    );
    assert_eq!(
        gulf.eval("workdays(2026-08-06, 2026-08-09)")
            .unwrap()
            .to_display(),
        "1 workdays"
    );
    // The same functions applied without an engine use the default Saturday/Sunday weekend.
    let calc = Abacus::standard();
    let friday = calc.eval("2026-08-07").unwrap();
    let op = calc.tokens().function("is_workday").unwrap();
    assert_eq!(op.apply(&[friday]).unwrap().to_display(), "1");
}

#[test]
fn overriding_builtin_arithmetic_is_honoured() {
    fn concat_digits(a: Value, b: Value) -> Result<Value, AbacusError> {
        Ok(Value::dimensionless(a.canonical * 10.0 + b.canonical))
    }
    let mut calc = Abacus::standard();
    calc.register_binop_token(BinaryOp {
        alias: "+",
        func: concat_digits,
        precedence: 0,
        right_associative: false,
    });
    assert_eq!(calc.eval("1 + 2").unwrap().to_display(), "12");
}
