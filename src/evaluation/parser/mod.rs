pub(crate) mod ast;
pub mod config;
pub(crate) mod eval;
pub mod pratt;
pub mod range;

pub use config::EvalConfig;
pub use pratt::MAX_RECURSION_DEPTH;

use std::collections::HashMap;

use crate::{
    TokenRegistry, UnitRegistry,
    evaluation::{
        parser::{
            ast::{Buffers, Located},
            eval::Evaluator,
            pratt::Parser,
        },
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
    /// Names whose reading depended on whether they were variables, and whether they were
    /// (recorded only when compiling with `record_lookups`).
    pub variable_lookups: Box<[(Box<str>, bool)]>,
}

/// Lexes and parses `input`. A "now" needed while lexing date literals is resolved through
/// `lazy_now` (when no anchor is configured), so the caller can evaluate with the same instant.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    input: &str,
    config: &EvalConfig,
    lazy_now: &std::cell::OnceCell<crate::Date>,
    record_lookups: bool,
) -> Result<Compiled, Located> {
    let mut buffers = Buffers::take();
    let compiled = compile_with(
        &mut buffers,
        token_registry,
        unit_registry,
        variables,
        input,
        config,
        lazy_now,
        record_lookups,
    );
    buffers.recycle();
    // The tree outlives this call, so don't keep spare capacity from the buffer pool.
    compiled.map(|mut compiled| {
        compiled.ast.shrink_to_fit();
        compiled
    })
}

/// [`compile`] using (and returning scratch space to) `buffers`.
#[allow(clippy::too_many_arguments)]
fn compile_with(
    buffers: &mut Buffers,
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&HashMap<String, EvalResult>>,
    input: &str,
    config: &EvalConfig,
    lazy_now: &std::cell::OnceCell<crate::Date>,
    record_lookups: bool,
) -> Result<Compiled, Located> {
    let mut lookups = Vec::new();
    let lexed = lex(
        buffers.lexed(),
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
        record_lookups.then_some(&mut lookups),
    )
    .map_err(|(error, span)| Located::new(error, Some(span), None))?;
    let mut parser = Parser::new(
        &lexed.tokens,
        &lexed.spans,
        token_registry,
        unit_registry,
        config.max_recursion_depth,
        buffers,
    );
    let root = parser.parse();
    let explicit_conversion = parser.has_explicit_conversion;
    let has_dates = parser.has_dates;
    let ast = parser.into_ast(buffers);
    let variable_lookups = if lookups.is_empty() {
        Box::default()
    } else {
        lookups.sort_unstable();
        lookups.dedup();
        lookups
            .into_iter()
            .map(|(name, found)| (name.into(), found))
            .collect()
    };
    buffers.keep_lexed(lexed);
    Ok(Compiled {
        ast,
        root: root?,
        explicit_conversion,
        has_dates,
        variable_lookups,
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
    let mut buffers = Buffers::take();
    let compiled = match compile_with(
        &mut buffers,
        token_registry,
        unit_registry,
        variables,
        input,
        &config,
        &lazy_now,
        false,
    ) {
        Ok(compiled) => compiled,
        Err(located) => {
            buffers.recycle();
            return Err(located);
        }
    };
    // Share a "now" resolved while lexing with the evaluator, so both see the same instant.
    if config.anchor_date.is_none() {
        config.anchor_date = lazy_now.into_inner();
    }
    let result = run(&compiled, unit_registry, variables, &config);
    buffers.ast = compiled.ast;
    buffers.recycle();
    result
}
