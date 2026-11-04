pub(crate) mod ast;
pub mod config;
pub(crate) mod eval;
pub mod pratt;
pub mod range;

pub use config::EvalConfig;
pub use pratt::MAX_RECURSION_DEPTH;
pub use range::RangeSeq;

use std::collections::HashMap;

use crate::{
    AbacusError, TokenRegistry, UnitRegistry,
    evaluation::{
        parser::{ast::Located, eval::Evaluator, pratt::Parser},
        tokenizer::lexer::{LexOptions, lex},
    },
    units::eval_result::EvalResult,
};

/// A parsed expression, ready to be evaluated (possibly many times).
#[derive(Debug, Clone)]
pub(crate) struct Compiled {
    ast: ast::Ast,
    root: ast::NodeId,
    /// The expression contains an explicit `in`/`to`/`as` conversion.
    explicit_conversion: bool,
    /// The expression contains date literals, which are resolved while lexing.
    pub has_dates: bool,
}

/// Lexes and parses `input`. A "now" needed while lexing date literals is resolved through
/// `lazy_now` (when no anchor is configured), so the caller can evaluate with the same instant.
pub(crate) fn compile(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    input: &str,
    config: &EvalConfig,
    lazy_now: &std::cell::OnceCell<crate::Date>,
) -> Result<Compiled, Located> {
    let lexed = lex(
        token_registry,
        unit_registry,
        variables,
        input,
        LexOptions {
            implicit_multiplication: config.implicit_multiplication,
            number_scales: config.number_scales,
            anchor_date: config.anchor_date.as_ref(),
            lazy_now: Some((lazy_now, config.default_timezone.as_ref())),
            date_month_first: config.date_format == crate::DateFormat::MMDDYYYY,
        },
    )
    .map_err(|(error, span)| Located::new(error, Some(span), None))?;
    let mut parser = Parser::new(
        &lexed.tokens,
        &lexed.spans,
        token_registry,
        unit_registry,
        config.max_recursion_depth,
    );
    let root = parser.parse()?;
    let explicit_conversion = parser.has_explicit_conversion;
    let has_dates = parser.has_dates;
    Ok(Compiled {
        ast: parser.into_ast(),
        root,
        explicit_conversion,
        has_dates,
    })
}

/// Evaluates a compiled expression and tidies the result's unit.
pub(crate) fn run(
    compiled: &Compiled,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    config: &EvalConfig,
) -> Result<EvalResult, Located> {
    let mut result =
        Evaluator::new(&compiled.ast, unit_registry, variables, config).eval(compiled.root)?;
    if compiled.explicit_conversion {
        // Keep the unit the user asked for instead of merging it into another.
        result.simplify_unit_display_with_options(unit_registry, false);
    } else if config.auto_derived {
        result = result.to_derived(unit_registry)?;
        result.simplify_unit_display(unit_registry);
    } else {
        result.simplify_unit_display(unit_registry);
    }
    Ok(result)
}

/// Lexes, parses and evaluates `input` in one go.
pub(crate) fn evaluate_located(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    input: &str,
    mut config: EvalConfig,
) -> Result<EvalResult, Located> {
    // The reference "now" is resolved at most once per evaluation: from the anchor if one is
    // set, otherwise lazily (reading the clock) only when something actually needs it.
    if config.anchor_date.is_some() {
        config.anchor_date = Some(crate::Date::reference_now(
            config.anchor_date.as_ref(),
            config.default_timezone.as_ref(),
        ));
    }
    let lazy_now = std::cell::OnceCell::new();
    let compiled = compile(
        token_registry,
        unit_registry,
        variables,
        input,
        &config,
        &lazy_now,
    )?;
    // Share a "now" resolved while lexing with the evaluator, so both see the same instant.
    if config.anchor_date.is_none() {
        config.anchor_date = lazy_now.into_inner();
    }
    let result = run(&compiled, unit_registry, variables, &config);
    compiled.ast.recycle();
    result
}

/// Convenience function: tokenize and evaluate an expression string.
pub fn evaluate(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input: &str,
) -> Result<EvalResult, AbacusError> {
    evaluate_with_config(token_registry, unit_registry, input, EvalConfig::default())
}

/// Tokenize and evaluate an expression string with explicit auto_derived setting.
pub fn evaluate_with_options(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input: &str,
    auto_derived: bool,
) -> Result<EvalResult, AbacusError> {
    let config = EvalConfig {
        auto_derived,
        ..Default::default()
    };
    evaluate_with_config(token_registry, unit_registry, input, config)
}

/// Tokenize and evaluate an expression string with full evaluation configuration.
pub fn evaluate_with_config(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input: &str,
    config: EvalConfig,
) -> Result<EvalResult, AbacusError> {
    evaluate_with_variables(token_registry, unit_registry, None, input, config)
}

/// Tokenize and evaluate an expression string with variable definitions and full evaluation
/// configuration.
pub fn evaluate_with_variables(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    input: &str,
    config: EvalConfig,
) -> Result<EvalResult, AbacusError> {
    evaluate_located(token_registry, unit_registry, variables, input, config)
        .map_err(|located| located.into_inner().error)
}
