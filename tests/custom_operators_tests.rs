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
    // Verifies calling calc.tokenize(expr) directly on an Abacus instance
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
    // Tests extending an Abacus calculator instance with a user-defined function ('dbl')
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
    // Tests registering a custom function that returns a Hash and accessing arbitrary dynamic keys via dot property
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

    // Access arbitrary key .my_custom_key without any hardcoding in the parser
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
    // Tests registering a single-character custom binary operator ('@') with custom precedence
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
    // Tests registering a multi-character custom binary operator ('++') and tokenizing/evaluating it
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
    // Tests registering a custom unary operator ('~') with prefix evaluation
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
