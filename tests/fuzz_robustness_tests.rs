use abacus::{AbacusError, eval};

#[test]
fn test_recursion_depth_limit_nested_parentheses() {
    // 1000 nested parentheses should cleanly fail with RecursionLimitExceeded
    let mut expr = String::with_capacity(2001);
    for _ in 0..1000 {
        expr.push('(');
    }
    expr.push('1');
    for _ in 0..1000 {
        expr.push(')');
    }

    let result = eval(&expr);
    assert_eq!(result, Err(AbacusError::RecursionLimitExceeded));
}

#[test]
fn test_recursion_depth_limit_unclosed_parentheses() {
    // 1000 unclosed opening parentheses
    let mut expr = String::with_capacity(1005);
    for _ in 0..1000 {
        expr.push('(');
    }
    expr.push_str("42");

    let result = eval(&expr);
    assert_eq!(result, Err(AbacusError::RecursionLimitExceeded));
}

#[test]
fn test_recursion_depth_limit_nested_unary_operators() {
    // 500 unary minuses: - - - ... 42
    let mut expr = String::with_capacity(1005);
    for _ in 0..500 {
        expr.push('-');
        expr.push(' ');
    }
    expr.push_str("42");

    let result = eval(&expr);
    assert_eq!(result, Err(AbacusError::RecursionLimitExceeded));
}

#[test]
fn test_recursion_depth_limit_nested_functions() {
    // 500 nested function calls: sqrt(sqrt(... 16 ...))
    let mut expr = String::with_capacity(3000);
    for _ in 0..500 {
        expr.push_str("sqrt(");
    }
    expr.push_str("16");
    for _ in 0..500 {
        expr.push(')');
    }

    let result = eval(&expr);
    assert_eq!(result, Err(AbacusError::RecursionLimitExceeded));
}

#[test]
fn test_recursion_depth_within_limit_succeeds() {
    // 50 nested parentheses is well within MAX_RECURSION_DEPTH (256)
    let mut expr = String::with_capacity(105);
    for _ in 0..50 {
        expr.push('(');
    }
    expr.push_str("42");
    for _ in 0..50 {
        expr.push(')');
    }

    let result = eval(&expr);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().to_display(), "42");
}

#[test]
fn test_extreme_arithmetic_overflow_never_panics() {
    let dangerous_inputs = [
        "1e308 ^ 1e308",
        "1e308 * 1e308",
        "(-1e308) - 1e308",
        "1e308 + 1e308",
        "0 / 0",
        "1 / 0",
        "-1 / 0",
        "0 ^ -1",
        "(-1) ^ 0.5",
        "(-5)!",
        "factorial(-10)",
        "factorial(1000)",
        "nCr(1000000, 500000)",
        "nPr(1000000, 500000)",
        "poissonpdf(1e300, 1e300)",
        "hypgeompdf(1e300, 1e300, 1e300, 1e300)",
        "irr(1e300, -1e300, 1e300)",
        "1..10 / 0..0",
        "0..0 / 0..0",
        "[-1e308, 1e308] * [-1e308, 1e308]",
    ];

    for input in dangerous_inputs {
        // Must either return Ok or Err, but NEVER panic or abort
        let _ = eval(input);
    }
}

#[test]
fn test_pseudo_random_string_fuzzing() {
    // Deterministic linear congruential generator (LCG) for reproducible fuzzing
    let mut seed: u64 = 0xDEADBEEFCAFEBABE;
    let mut next_rand = || -> u64 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        seed
    };

    let corpus_tokens = [
        "0",
        "1",
        "-1",
        "3.14",
        "1e308",
        "1e-308",
        "inf",
        "nan",
        "+",
        "-",
        "*",
        "/",
        "^",
        "%",
        "!",
        "++",
        "--",
        "..",
        "(",
        ")",
        "[",
        "]",
        "{",
        "}",
        ",",
        ";",
        ":",
        ".",
        "m",
        "s",
        "kg",
        "J",
        "N",
        "deg",
        "rad",
        "USD",
        "h",
        "hour",
        "in",
        "to",
        "as",
        "of",
        "ago",
        "before",
        "after",
        "sin",
        "cos",
        "tan",
        "sqrt",
        "ln",
        "log",
        "exp",
        "abs",
        "poissonpdf",
        "hypgeompdf",
        "irr",
        "npv",
        "tdy",
        "tmr",
        "yesterday",
        "now",
        " ",
        "\t",
        "\n",
        "\0",
        "@",
        "#",
        "$",
        "&",
        "|",
        "~",
        "`",
        "\\",
    ];

    // Run 5,000 generated randomized inputs through eval
    for _ in 0..5000 {
        let length = (next_rand() % 15) + 1;
        let mut expr = String::new();
        for _ in 0..length {
            let idx = (next_rand() as usize) % corpus_tokens.len();
            expr.push_str(corpus_tokens[idx]);
        }

        // Must never panic
        let _ = eval(&expr);
    }
}
