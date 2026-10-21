use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::evaluation::tokenizer::registry::{
    binary::{arithmetic::register_arithmetic, operators::BinaryOp},
    function::{
        combinatorics::register_combinatorics, math::register_math,
        math_helpers::register_math_helpers, operators::FunctionOp, trig::register_trig,
    },
    unary::{general::register_general, operators::UnaryOp},
};

#[cfg(feature = "date")]
use crate::evaluation::tokenizer::registry::function::date::register_date_functions;
#[cfg(feature = "distributions")]
use crate::evaluation::tokenizer::registry::function::distributions::register_distributions;
#[cfg(feature = "financial")]
use crate::evaluation::tokenizer::registry::function::financial::register_financial;
#[cfg(feature = "stats")]
use crate::evaluation::tokenizer::registry::function::{
    ci::register_ci, hypothesis::register_hypothesis, regression::register_regression,
    stats::register_stats,
};

#[derive(Debug, Default)]
pub struct TokenRegistry {
    pub binary_operators: HashMap<String, Arc<BinaryOp>>,
    pub unary_operators: HashMap<String, Arc<UnaryOp>>,
    pub function_operators: HashMap<String, Arc<FunctionOp>>,
    /// Lazily-built operator index used by the lexer. Cleared by the `register_*` methods and
    /// rebuilt automatically if the operator maps are mutated directly (detected by size change).
    index_cache: RwLock<Option<Arc<OperatorIndex>>>,
}

/// Operator patterns grouped by first character, longest pattern first.
#[derive(Debug)]
pub struct OperatorIndex {
    by_first_char: HashMap<char, Vec<(Box<str>, MatchedOpKind)>>,
    signature: (usize, usize, usize),
}

impl OperatorIndex {
    /// Candidate patterns starting with `c`, sorted by descending length.
    #[must_use]
    pub fn candidates(&self, c: char) -> Option<&[(Box<str>, MatchedOpKind)]> {
        self.by_first_char.get(&c).map(Vec::as_slice)
    }
}

/// Leaks `alias` as a `'static` string only when it differs from the operator's own alias.
/// Registration is a one-off setup step, so the leak is bounded by the number of registrations.
fn static_alias(alias: &str, current: &'static str) -> &'static str {
    if alias == current {
        current
    } else {
        Box::leak(alias.to_string().into_boxed_str())
    }
}

impl TokenRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    #[allow(unused_mut)]
    pub fn standard() -> Self {
        let binary = register_arithmetic();

        let mut unary: Vec<UnaryOp> = Vec::new();
        unary.append(&mut register_general());

        let mut functions = [
            register_trig(),
            register_math(),
            register_combinatorics(),
            register_math_helpers(),
        ]
        .concat();

        #[cfg(feature = "financial")]
        functions.extend(register_financial());

        #[cfg(feature = "stats")]
        {
            functions.extend(register_stats());
            functions.extend(register_ci());
            functions.extend(register_regression());
            functions.extend(register_hypothesis());
        }

        #[cfg(feature = "distributions")]
        functions.extend(register_distributions());

        #[cfg(feature = "date")]
        functions.extend(register_date_functions());

        Self {
            index_cache: RwLock::new(None),
            binary_operators: binary
                .into_iter()
                .map(|op| (op.alias.to_string(), Arc::new(op)))
                .collect(),
            unary_operators: unary
                .into_iter()
                .map(|op| (op.alias.to_string(), Arc::new(op)))
                .collect(),
            function_operators: functions
                .into_iter()
                .map(|op| (op.name.to_string(), Arc::new(op)))
                .collect(),
        }
    }

    /// Registers a binary operator under `alias`. The lexer emits the registered alias, so the
    /// operator's own `alias` field is rewritten to match the key it was registered under.
    pub fn register_binary_operator(&mut self, alias: &str, mut op: BinaryOp) {
        op.alias = static_alias(alias, op.alias);
        self.binary_operators
            .insert(alias.to_string(), Arc::new(op));
        self.invalidate_index();
    }

    /// Registers a unary operator under `alias` (see [`Self::register_binary_operator`]).
    pub fn register_unary_operator(&mut self, alias: &str, mut op: UnaryOp) {
        op.alias = static_alias(alias, op.alias);
        self.unary_operators.insert(alias.to_string(), Arc::new(op));
        self.invalidate_index();
    }

    /// Registers a function under `alias` (see [`Self::register_binary_operator`]).
    pub fn register_function_operator(&mut self, alias: &str, mut op: FunctionOp) {
        op.name = static_alias(alias, op.name);
        self.function_operators
            .insert(alias.to_string(), Arc::new(op));
        self.invalidate_index();
    }

    fn invalidate_index(&mut self) {
        *self
            .index_cache
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    fn signature(&self) -> (usize, usize, usize) {
        (
            self.binary_operators.len(),
            self.unary_operators.len(),
            self.function_operators.len(),
        )
    }

    /// Returns the cached operator index, building it on first use.
    #[must_use]
    pub fn operator_index(&self) -> Arc<OperatorIndex> {
        let signature = self.signature();
        if let Some(index) = self
            .index_cache
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            && index.signature == signature
        {
            return Arc::clone(index);
        }
        let index = Arc::new(self.build_index());
        *self
            .index_cache
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&index));
        index
    }

    fn build_index(&self) -> OperatorIndex {
        let mut map: HashMap<char, Vec<(Box<str>, MatchedOpKind)>> = HashMap::new();
        let entries = self
            .binary_operators
            .iter()
            .map(|(k, op)| (k, MatchedOpKind::Binary(op.alias)))
            .chain(
                self.unary_operators
                    .iter()
                    .map(|(k, op)| (k, MatchedOpKind::Unary(op.alias))),
            )
            .chain(
                self.function_operators
                    .iter()
                    .map(|(k, op)| (k, MatchedOpKind::Func(op.name))),
            );
        for (pattern, kind) in entries {
            if let Some(c) = pattern.chars().next() {
                map.entry(c)
                    .or_default()
                    .push((pattern.as_str().into(), kind));
            }
        }
        for candidates in map.values_mut() {
            // Longest match first; equal-length ties resolve Binary < Unary < Func, then by pattern,
            // so the result never depends on HashMap iteration order.
            candidates.sort_unstable_by(|a, b| {
                b.0.len()
                    .cmp(&a.0.len())
                    .then_with(|| a.1.rank().cmp(&b.1.rank()))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        OperatorIndex {
            by_first_char: map,
            signature: self.signature(),
        }
    }

    /// Indexes all registered operators by their first character, sorted by descending pattern length.
    #[must_use]
    #[deprecated(note = "use `operator_index()`, which is cached")]
    pub fn operators_by_first_char(&self) -> HashMap<char, Vec<(&str, MatchedOpKind)>> {
        let mut map: HashMap<char, Vec<(&str, MatchedOpKind)>> = HashMap::new();
        let index = self.build_index();
        for (c, candidates) in index.by_first_char {
            for (pattern, kind) in candidates {
                let key = self
                    .binary_operators
                    .get_key_value(&*pattern)
                    .map(|(k, _)| k.as_str())
                    .or_else(|| {
                        self.unary_operators
                            .get_key_value(&*pattern)
                            .map(|(k, _)| k.as_str())
                    })
                    .or_else(|| {
                        self.function_operators
                            .get_key_value(&*pattern)
                            .map(|(k, _)| k.as_str())
                    });
                if let Some(key) = key {
                    map.entry(c).or_default().push((key, kind));
                }
            }
        }
        map
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedOpKind {
    Binary(&'static str),
    Unary(&'static str),
    Func(&'static str),
}

impl MatchedOpKind {
    const fn rank(self) -> u8 {
        match self {
            Self::Binary(_) => 0,
            Self::Unary(_) => 1,
            Self::Func(_) => 2,
        }
    }
}
