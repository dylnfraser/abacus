use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::registry::helpers::fx_hash::FxHashMap;

use crate::evaluation::tokenizer::registry::{
    binary::{arithmetic::standard_binary_ops, operators::BinaryOp},
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
    /// Lock-free copy of the index for the common case where the maps are only changed
    /// through the `register_*` methods (which reset it).
    index: std::sync::OnceLock<Arc<OperatorIndex>>,
}

/// Operator patterns grouped by first character, longest pattern first.
#[derive(Debug)]
pub struct OperatorIndex {
    by_first_char: HashMap<char, Vec<(Box<str>, MatchedOpKind)>>,
    /// Patterns that are not plain words (contain a symbol or `-`), by first character,
    /// longest first. Plain-word patterns live in `words` instead.
    symbolic_by_first_char: FxHashMap<char, Vec<(Box<str>, MatchedOpKind)>>,
    /// Plain-word patterns (see [`is_word_pattern`]) mapped to their kinds in tie-break order.
    words: FxHashMap<Box<str>, Vec<MatchedOpKind>>,
    signature: (usize, usize, usize),
}

/// Characters that continue an identifier for operator word-boundary purposes.
#[inline]
#[must_use]
pub(crate) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '°' | 'Å' | 'Ω')
}

/// A pattern is a plain word when every character continues an identifier and it ends in an
/// alphanumeric or `_` (so the lexer's word-boundary rule applies). Such a pattern can only
/// match when it equals the input's whole leading word, which allows an exact map lookup.
pub(crate) fn is_word_pattern(pattern: &str) -> bool {
    pattern.chars().all(is_word_char)
        && pattern
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// A borrowed or shared [`OperatorIndex`] (see [`TokenRegistry::operator_index_ref`]).
pub(crate) enum IndexRef<'r> {
    Borrowed(&'r OperatorIndex),
    Shared(Arc<OperatorIndex>),
}

impl std::ops::Deref for IndexRef<'_> {
    type Target = OperatorIndex;
    fn deref(&self) -> &OperatorIndex {
        match self {
            Self::Borrowed(index) => index,
            Self::Shared(index) => index,
        }
    }
}

impl OperatorIndex {
    /// Candidate patterns starting with `c`, sorted by descending length.
    #[must_use]
    pub fn candidates(&self, c: char) -> Option<&[(Box<str>, MatchedOpKind)]> {
        self.by_first_char.get(&c).map(Vec::as_slice)
    }

    /// Non-word candidate patterns starting with `c`, sorted by descending length.
    #[must_use]
    pub(crate) fn symbolic_candidates(&self, c: char) -> &[(Box<str>, MatchedOpKind)] {
        self.symbolic_by_first_char
            .get(&c)
            .map_or(&[], Vec::as_slice)
    }

    /// Kinds registered for the plain-word pattern `word`, in tie-break order.
    #[must_use]
    pub(crate) fn word_kinds(&self, word: &str) -> &[MatchedOpKind] {
        self.words.get(word).map_or(&[], Vec::as_slice)
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
            index: std::sync::OnceLock::new(),
            // Shared built-in instances: their identity is how built-ins are recognised
            // (see `arithmetic::standard_binary_ops`).
            binary_operators: standard_binary_ops()
                .iter()
                .map(|(op, _)| (op.alias.to_string(), Arc::clone(op)))
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
        self.index = std::sync::OnceLock::new();
        *self
            .index_cache
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Borrows the operator index without locking (the lexer calls this once per input).
    ///
    /// The index is built once and reset by the `register_*` methods. If the public maps were
    /// mutated directly (detected by a size change), falls back to [`Self::operator_index`].
    pub(crate) fn operator_index_ref(&self) -> IndexRef<'_> {
        let index = self.index.get_or_init(|| Arc::new(self.build_index()));
        if index.signature == self.signature() {
            IndexRef::Borrowed(index)
        } else {
            IndexRef::Shared(self.operator_index())
        }
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
        if let Some(index) = self.index.get()
            && index.signature == signature
        {
            return Arc::clone(index);
        }
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
        let mut symbolic_by_first_char: FxHashMap<char, Vec<(Box<str>, MatchedOpKind)>> =
            FxHashMap::default();
        let mut words: FxHashMap<Box<str>, Vec<MatchedOpKind>> = FxHashMap::default();
        for (&c, candidates) in &map {
            for (pattern, kind) in candidates {
                if is_word_pattern(pattern) {
                    // Candidates are already in tie-break order, so pushing preserves it.
                    words.entry(pattern.clone()).or_default().push(*kind);
                } else {
                    symbolic_by_first_char
                        .entry(c)
                        .or_default()
                        .push((pattern.clone(), *kind));
                }
            }
        }
        OperatorIndex {
            by_first_char: map,
            symbolic_by_first_char,
            words,
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
