pub mod config;
pub mod conversion;
pub mod function;
pub mod pratt;
pub mod prefix;
pub mod range;

pub use config::EvalConfig;
pub use pratt::{MAX_RECURSION_DEPTH, Parser};
pub use range::RangeSeq;

use crate::{AbacusError, TokenRegistry, UnitRegistry, units::eval_result::EvalResult};

/// Convenience function: tokenize and parse an expression string into an `EvalResult`.
pub fn evaluate(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input: &str,
) -> Result<EvalResult, AbacusError> {
    evaluate_with_config(token_registry, unit_registry, input, EvalConfig::default())
}

/// Tokenize and parse an expression string with explicit auto_derived setting.
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

/// Tokenize and parse an expression string with full evaluation configuration.
pub fn evaluate_with_config(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input: &str,
    config: EvalConfig,
) -> Result<EvalResult, AbacusError> {
    evaluate_with_variables(token_registry, unit_registry, None, input, config)
}

/// Tokenize and parse an expression string with variable definitions and full evaluation configuration.
pub fn evaluate_with_variables(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, EvalResult>>,
    input: &str,
    config: EvalConfig,
) -> Result<EvalResult, AbacusError> {
    let tokens = crate::evaluation::tokenizer::lexer::tokenize_string_with_anchor(
        token_registry,
        unit_registry,
        variables,
        input,
        config.implicit_multiplication,
        config.number_scales,
        config.anchor_date.as_ref(),
    )?;
    let mut parser =
        Parser::new_with_variables(&tokens, token_registry, unit_registry, variables, config);
    let result = parser.parse()?;
    if parser.has_explicit_conversion || !parser.config.auto_derived {
        let mut result = result;
        result.simplify_unit_display(unit_registry);
        Ok(result)
    } else {
        let mut result = result.to_derived(unit_registry)?;
        result.simplify_unit_display(unit_registry);
        Ok(result)
    }
}
