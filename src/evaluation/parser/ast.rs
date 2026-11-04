//! Syntax tree produced by [`super::Parser`] and walked by [`super::eval::Evaluator`].
//!
//! Nodes live in one arena ([`Ast`]) and refer to each other by index, so building a tree
//! costs a handful of amortised vector pushes rather than an allocation per node. Operator
//! sequences at one precedence level are stored flat ([`Kind::Chain`]), so the tree's depth is
//! bounded by the parser's recursion depth even for long inputs like `1 + 1 + … + 1`.

use std::sync::Arc;

use crate::{
    AbacusError, BinaryOp, Date, FunctionOp, TimeZone, UnaryOp, Unit, Value,
    evaluation::tokenizer::{
        lexer::Lexed,
        tokens::{Span, Token},
    },
};

/// Index of a node in [`Ast::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeId(u32);

/// A contiguous range of [`Ast::leds`] or [`Ast::args`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slice {
    start: u32,
    len: u32,
}

impl Slice {
    fn range(self) -> std::ops::Range<usize> {
        self.start as usize..(self.start + self.len) as usize
    }
}

/// A parsed expression.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ast {
    nodes: Vec<Node>,
    leds: Vec<Led>,
    args: Vec<Arg>,
}

/// Buffers kept between evaluations on one thread, so that evaluating an expression doesn't
/// allocate for its tokens and tree once a few expressions have been seen. Taken whole at the
/// start of an evaluation and put back at the end (one thread-local access each).
#[derive(Default)]
pub(crate) struct Buffers {
    pub ast: Ast,
    pub leds: Vec<Led>,
    pub args: Vec<Arg>,
    /// Always empty; `'static` only so it can be stored (see [`Buffers::lexed`]).
    tokens: Vec<Token<'static>>,
    spans: Vec<Span>,
}

/// Buffers larger than this (in elements) are freed rather than kept.
const MAX_SPARE_CAPACITY: usize = 4096;

thread_local! {
    static SPARE: std::cell::Cell<Buffers> = std::cell::Cell::new(Buffers::default());
}

impl Buffers {
    /// This thread's spare buffers (empty ones if none are available).
    pub(crate) fn take() -> Self {
        SPARE.try_with(std::cell::Cell::take).unwrap_or_default()
    }

    /// Empties the buffers and keeps them for the next [`Buffers::take`] on this thread.
    pub(crate) fn recycle(mut self) {
        let fits = |len: usize| len <= MAX_SPARE_CAPACITY;
        if !(fits(self.ast.nodes.capacity())
            && fits(self.ast.leds.capacity())
            && fits(self.ast.args.capacity())
            && fits(self.leds.capacity())
            && fits(self.args.capacity())
            && fits(self.tokens.capacity())
            && fits(self.spans.capacity()))
        {
            return;
        }
        self.ast.nodes.clear();
        self.ast.leds.clear();
        self.ast.args.clear();
        self.leds.clear();
        self.args.clear();
        self.spans.clear();
        let _ = SPARE.try_with(|spare| spare.set(self));
    }

    /// Empty token and span buffers for the lexer.
    pub(crate) fn lexed<'a>(&mut self) -> Lexed<'a> {
        Lexed {
            tokens: std::mem::take(&mut self.tokens),
            spans: std::mem::take(&mut self.spans),
        }
    }

    /// Keeps the lexer's buffers once their tokens are no longer needed.
    pub(crate) fn keep_lexed(&mut self, lexed: Lexed<'_>) {
        let Lexed { mut tokens, spans } = lexed;
        tokens.clear();
        // Re-typing an empty vector to `'static` reuses its allocation (same layout).
        self.tokens = tokens.into_iter().map(|_| unreachable!()).collect();
        self.spans = spans;
    }
}

impl Ast {
    /// Makes room for an expression of `tokens` tokens (every node and operation consumes at
    /// least one token, so this avoids regrowing).
    pub(crate) fn reserve_for(&mut self, tokens: usize) {
        self.nodes.reserve(tokens);
        self.leds.reserve(tokens / 2 + 1);
    }

    pub(crate) fn shrink_to_fit(&mut self) {
        self.nodes.shrink_to_fit();
        self.leds.shrink_to_fit();
        self.args.shrink_to_fit();
    }

    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    pub(crate) fn leds(&self, slice: Slice) -> &[Led] {
        &self.leds[slice.range()]
    }

    pub(crate) fn args(&self, slice: Slice) -> &[Arg] {
        &self.args[slice.range()]
    }

    pub(crate) fn push(&mut self, kind: Kind, span: Span) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("expression too large"));
        self.nodes.push(Node { kind, span });
        id
    }

    /// Moves `scratch[mark..]` into the arena, returning where it now lives.
    pub(crate) fn push_leds(&mut self, scratch: &mut Vec<Led>, mark: usize) -> Slice {
        Self::move_into(&mut self.leds, scratch, mark)
    }

    /// Moves `scratch[mark..]` into the arena, returning where it now lives.
    pub(crate) fn push_args(&mut self, scratch: &mut Vec<Arg>, mark: usize) -> Slice {
        Self::move_into(&mut self.args, scratch, mark)
    }

    fn move_into<T>(dst: &mut Vec<T>, scratch: &mut Vec<T>, mark: usize) -> Slice {
        let start = u32::try_from(dst.len()).expect("expression too large");
        let len = u32::try_from(scratch.len() - mark).expect("expression too large");
        if mark == 0 && dst.is_empty() {
            // Typically the only (or outermost) chain: hand over the whole buffer.
            std::mem::swap(dst, scratch);
        } else {
            dst.extend(scratch.drain(mark..));
        }
        Slice { start, len }
    }

    /// Whether `id` could evaluate to a date. Conservative: `false` only when it certainly
    /// doesn't (numbers, units, intervals, arithmetic on those).
    pub(crate) fn may_be_date(&self, id: NodeId) -> bool {
        match &self.node(id).kind {
            Kind::Value(_)
            | Kind::Number(_)
            | Kind::Unit(_)
            | Kind::Interval { .. }
            | Kind::PercentChange { .. } => false,
            Kind::Prefix { operand, .. } => self.may_be_date(*operand),
            Kind::Ident(_) | Kind::Date(_) | Kind::Call { .. } | Kind::RelPrefix { .. } => true,
            Kind::Chain { head, ops } => self.chain_may_be_date(*head, self.leds(*ops)),
        }
    }

    /// Whether `head` followed by `ops` could evaluate to a date.
    pub(crate) fn chain_may_be_date(&self, head: NodeId, ops: &[Led]) -> bool {
        ops.iter().fold(self.may_be_date(head), |date, led| {
            self.may_yield_date(led, date)
        })
    }

    /// Whether applying `led` to a value that may be a date (`lhs`) may give a date.
    fn may_yield_date(&self, led: &Led, lhs: bool) -> bool {
        match &led.kind {
            LedKind::Binary { rhs, .. } => lhs || self.may_be_date(*rhs),
            LedKind::Postfix(_) => lhs,
            LedKind::Convert(c) => match &c.target {
                Target::Zone { .. } | Target::Expr { .. } => lhs,
                Target::PercentOf(_) | Target::BareHour(_) | Target::Date(_) => false,
            },
            LedKind::Ago | LedKind::FromNow | LedKind::Before { .. } | LedKind::After(_) => true,
            LedKind::PercentTag(_)
            | LedKind::Range { .. }
            | LedKind::Until { .. }
            | LedKind::Property(_) => false,
        }
    }

    /// The last percentage tag written in `id` (`15% tax` -> `tax`), in source order.
    pub(crate) fn last_percent_tag(&self, id: NodeId) -> Option<&'static str> {
        match &self.node(id).kind {
            Kind::Chain { head, ops } => {
                let mut tag = self.last_percent_tag(*head);
                for led in self.leds(*ops) {
                    match &led.kind {
                        LedKind::PercentTag(t) => tag = Some(t),
                        LedKind::Binary { rhs, .. } => tag = self.last_percent_tag(*rhs).or(tag),
                        LedKind::After(after) => {
                            let inner = after.rhs.and_then(|rhs| self.last_percent_tag(rhs));
                            tag = after.tag.map(|(t, _)| t).or(inner).or(tag);
                        }
                        _ => {}
                    }
                }
                tag
            }
            Kind::Prefix { operand, .. } => self.last_percent_tag(*operand),
            _ => None,
        }
    }
}

/// A node of the syntax tree with the source span it was parsed from.
#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub kind: Kind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum Kind {
    /// A number with a unit folded in by the lexer (`5 m`, `$3`).
    Value(Value),
    /// A bare number.
    Number(f64),
    /// A unit on its own (`m`, `km/h`): one of that unit.
    Unit(Arc<Unit>),
    /// A variable or constant, looked up when evaluated.
    Ident(Box<str>),
    /// A date or time literal (boxed: dates are large and rare).
    Date(Box<Date>),
    /// `[lo, hi]`.
    Interval { lo: NodeId, hi: NodeId },
    /// A prefix operator (`-x`, `sqrt x`).
    Prefix { op: Arc<UnaryOp>, operand: NodeId },
    /// A function call.
    Call { func: Arc<FunctionOp>, args: Slice },
    /// `% change from a to b`.
    PercentChange { from: NodeId, to: NodeId },
    /// A leading `in`/`after`/`ago`/… applied to a duration (`in 3 hours`, `before 07-08-2026`).
    RelPrefix { op: RelPrefix, operand: NodeId },
    /// An operand followed by infix and postfix operations, applied left to right.
    Chain { head: NodeId, ops: Slice },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelPrefix {
    /// `in 3 hours`, `after 2 days`: now + duration.
    Forward,
    /// `before 2 days`, `ago 2 days`: now - duration.
    Backward,
    /// `until <date>`: whole days from now.
    Until,
}

/// A function argument.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Arg {
    Expr(NodeId),
    /// `start..end` or `start..end..step`, expanded to its values.
    Range {
        start: NodeId,
        end: NodeId,
        step: Option<NodeId>,
    },
}

/// An infix or postfix operation applied to the value accumulated so far in a [`Kind::Chain`].
#[derive(Debug, Clone)]
pub(crate) struct Led {
    pub kind: LedKind,
    /// Span of the operator token (where errors from this operation point).
    pub span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum LedKind {
    /// `20% tip`: requires a percentage.
    PercentTag(&'static str),
    /// `5!`
    Postfix(Arc<UnaryOp>),
    /// `a + b`
    Binary { op: Arc<BinaryOp>, rhs: NodeId },
    /// `a..b` outside a function call: an interval.
    Range { hi: NodeId },
    /// `a in b` (boxed: conversions are rare and their targets large).
    Convert(Box<Convert>),
    /// `3 days ago`
    Ago,
    /// `3 days from now`
    FromNow,
    /// `3 days before [date]`
    Before { date: Option<NodeId> },
    /// `3 days after [date]` or `$50 after 15% tax`.
    After(After),
    /// `x until <date>`
    Until { target: NodeId },
    /// `.slope`
    Property(Box<str>),
}

/// Operands of `after`, which is either date arithmetic or a percentage increase depending on
/// the left-hand side (decided when evaluated).
#[derive(Debug, Clone, Copy)]
pub(crate) struct After {
    pub rhs: Option<NodeId>,
    /// Whether `rhs` began like a date expression (only then does date arithmetic use it).
    pub rhs_is_date: bool,
    /// A percentage tag right after `rhs` (`$50 after 20% off`).
    pub tag: Option<(&'static str, Span)>,
}

#[derive(Debug, Clone)]
pub(crate) struct Convert {
    /// Whether the converted expression contains a written unit (see `Value::convert_to`).
    pub lhs_has_unit: bool,
    pub target: Target,
}

#[derive(Debug, Clone)]
pub(crate) enum Target {
    /// `x as % of base`
    PercentOf(NodeId),
    /// A bare hour after a date-time (`today at 12 to 1`), else a plain number.
    BareHour(f64),
    /// A date literal: the span between two dates.
    Date(Box<Date>),
    /// A timezone; for non-dates, the unit of the same name (if any).
    Zone {
        tz: TimeZone,
        unit: Result<Arc<Unit>, AbacusError>,
    },
    /// Any other expression.
    Expr {
        node: NodeId,
        /// Whether the target has a magnitude (`in 20 days`) rather than being a pure unit.
        has_magnitude: bool,
    },
}

/// An error with the source location it applies to (the innermost one wins).
///
/// Boxed so that the `Result`s passed around while parsing and evaluating stay small: errors
/// are rare, successes are not.
#[derive(Debug, Clone)]
pub(crate) struct Located(Box<LocatedError>);

#[derive(Debug, Clone)]
pub(crate) struct LocatedError {
    pub error: AbacusError,
    pub span: Option<Span>,
    pub note: Option<UnitsNote>,
}

/// The units on either side of an operator that rejected them, rendered as a note (`m + s`)
/// only when a diagnostic is actually shown.
#[derive(Debug, Clone)]
pub(crate) struct UnitsNote {
    pub lhs: Arc<crate::Unit>,
    pub op: &'static str,
    pub rhs: Arc<crate::Unit>,
}

impl UnitsNote {
    pub(crate) fn render(&self) -> String {
        let name = |u: &crate::Unit| {
            let s = u.display.render();
            if s.is_empty() {
                "a number".to_string()
            } else {
                s
            }
        };
        format!("{} {} {}", name(&self.lhs), self.op, name(&self.rhs))
    }
}

impl Located {
    pub(crate) fn new(error: AbacusError, span: Option<Span>, note: Option<UnitsNote>) -> Self {
        Self(Box::new(LocatedError { error, span, note }))
    }

    pub(crate) fn into_inner(self) -> LocatedError {
        *self.0
    }
}

impl std::ops::Deref for Located {
    type Target = LocatedError;
    fn deref(&self) -> &LocatedError {
        &self.0
    }
}

impl From<AbacusError> for Located {
    fn from(error: AbacusError) -> Self {
        Self::new(error, None, None)
    }
}

/// Attaches a span to an error that doesn't have one yet.
pub(crate) trait At<T> {
    fn at(self, span: Span) -> Result<T, Located>;
}

impl<T> At<T> for Result<T, AbacusError> {
    #[inline]
    fn at(self, span: Span) -> Result<T, Located> {
        self.map_err(|error| Located::new(error, Some(span), None))
    }
}

impl<T> At<T> for Result<T, Located> {
    #[inline]
    fn at(self, span: Span) -> Result<T, Located> {
        self.map_err(|mut located| {
            located.0.span.get_or_insert(span);
            located
        })
    }
}
