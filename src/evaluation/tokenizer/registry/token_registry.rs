use std::{collections::HashMap, sync::Arc};

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
}

impl TokenRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            binary_operators: HashMap::new(),
            unary_operators: HashMap::new(),
            function_operators: HashMap::new(),
        }
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

    pub fn register_binary_operator(&mut self, alias: &str, op: BinaryOp) {
        self.binary_operators
            .insert(alias.to_string(), Arc::new(op));
    }

    pub fn register_unary_operator(&mut self, alias: &str, op: UnaryOp) {
        self.unary_operators.insert(alias.to_string(), Arc::new(op));
    }

    pub fn register_function_operator(&mut self, alias: &str, op: FunctionOp) {
        self.function_operators
            .insert(alias.to_string(), Arc::new(op));
    }

    /// Indexes all registered operators by their first character, sorted by descending pattern length.
    #[must_use]
    pub fn operators_by_first_char(&self) -> HashMap<char, Vec<(&str, MatchedOpKind)>> {
        let mut map: HashMap<char, Vec<(&str, MatchedOpKind)>> = HashMap::new();
        for (alias, op) in &self.binary_operators {
            if let Some(c) = alias.chars().next() {
                map.entry(c)
                    .or_default()
                    .push((alias.as_str(), MatchedOpKind::Binary(op.alias)));
            }
        }
        for (alias, op) in &self.unary_operators {
            if let Some(c) = alias.chars().next() {
                map.entry(c)
                    .or_default()
                    .push((alias.as_str(), MatchedOpKind::Unary(op.alias)));
            }
        }
        for (name, op) in &self.function_operators {
            if let Some(c) = name.chars().next() {
                map.entry(c)
                    .or_default()
                    .push((name.as_str(), MatchedOpKind::Func(op.name)));
            }
        }
        for candidates in map.values_mut() {
            candidates.sort_unstable_by_key(|b| std::cmp::Reverse(b.0.len()));
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
