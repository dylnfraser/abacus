//! Pratt parser building the syntax tree in [`super::ast`].
//!
//! Parsing only looks at tokens; everything that depends on values (dimensions, whether the
//! left side of `in` is a date, ...) is decided by the evaluator.

use std::sync::Arc;

use crate::{
    AbacusError, Token, TokenRegistry, UnitRegistry,
    evaluation::{
        parser::ast::{
            After, Arg, Ast, At, Buffers, Convert, Kind, Led, LedKind, Located, NodeId, RelPrefix,
            Target,
        },
        tokenizer::{registry::binary::operators::BinaryOp, tokens::Span},
    },
};

/// Default maximum parser recursion depth.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// Hard upper bound accepted by [`crate::Abacus::with_max_recursion_depth`]. Deeper limits are
/// clamped so that hostile input cannot overflow the stack.
pub const MAX_RECURSION_DEPTH_CAP: usize = 256;

/// Maximum stack the parser (and separately the evaluator) may consume for one expression.
/// Frames are much larger in debug builds, so in addition to the depth limit, recursion stops
/// once this much stack has been used. 1 MiB leaves headroom on the default 2 MiB stack of
/// spawned threads.
pub(crate) const STACK_BUDGET_BYTES: usize = 1024 * 1024;

/// Approximate current stack position (address of a local).
#[inline(always)]
pub(crate) fn stack_position() -> usize {
    let marker = 0u8;
    std::ptr::from_ref(std::hint::black_box(&marker)) as usize
}

/// Binding power of `..` (left, right).
const RANGE_BP: (u8, u8) = (5, 6);
/// Left binding power of `in`/`to`/`as`, the loosest infix operator.
const CONVERSION_BP: u8 = 1;
/// Left binding power of the relative-time operators (`ago`, `after`, ...).
const REL_TIME_BP: u8 = 5;
/// Binding power of a single unparenthesized function argument (`sin 30 deg`).
const BARE_ARG_BP: u8 = 8;

/// Builds a syntax tree from tokens.
pub(crate) struct Parser<'a> {
    tokens: &'a [Token<'a>],
    spans: &'a [Span],
    pos: usize,
    token_registry: &'a TokenRegistry,
    unit_registry: &'a UnitRegistry,
    max_depth: usize,
    recursion_depth: usize,
    /// Stack position at the outermost `parse_expr` call (0 until parsing starts).
    stack_base: usize,
    /// Nesting of function-call argument lists; `..` there is a range, not an interval.
    function_arg_depth: usize,
    /// Whether any `in`/`to`/`as` conversion was parsed.
    pub has_explicit_conversion: bool,
    /// Whether any date literal was parsed (their value depends on when they were lexed).
    pub has_dates: bool,
    /// The tree being built.
    ast: Ast,
    /// Operations of the chains being parsed, innermost last (see [`Ast::push_leds`]).
    led_scratch: Vec<Led>,
    /// Arguments of the calls being parsed, innermost last.
    arg_scratch: Vec<Arg>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(
        tokens: &'a [Token<'a>],
        spans: &'a [Span],
        token_registry: &'a TokenRegistry,
        unit_registry: &'a UnitRegistry,
        max_depth: usize,
        buffers: &mut Buffers,
    ) -> Self {
        debug_assert_eq!(tokens.len(), spans.len());
        let mut ast = std::mem::take(&mut buffers.ast);
        ast.reserve_for(tokens.len());
        Self {
            tokens,
            spans,
            pos: 0,
            token_registry,
            unit_registry,
            max_depth: max_depth.min(MAX_RECURSION_DEPTH_CAP),
            recursion_depth: 0,
            stack_base: 0,
            function_arg_depth: 0,
            has_explicit_conversion: false,
            has_dates: false,
            ast,
            led_scratch: std::mem::take(&mut buffers.leds),
            arg_scratch: std::mem::take(&mut buffers.args),
        }
    }

    /// The tree built by [`Self::parse`]; the scratch stacks go back to `buffers`.
    pub(crate) fn into_ast(self, buffers: &mut Buffers) -> Ast {
        buffers.leds = self.led_scratch;
        buffers.args = self.arg_scratch;
        self.ast
    }

    // ── Token access ──

    #[inline(always)]
    fn peek(&self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.pos)
    }

    #[inline(always)]
    fn peek_at(&self, offset: usize) -> Option<&'a Token<'a>> {
        self.tokens.get(self.pos + offset)
    }

    #[inline(always)]
    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Span of token `i`, or an empty span at the end of the input.
    fn span_of(&self, i: usize) -> Span {
        self.spans.get(i).copied().unwrap_or_else(|| {
            let end = self.spans.last().map_or(0, |s| s.end);
            Span::new(end, end)
        })
    }

    fn here(&self) -> Span {
        self.span_of(self.pos)
    }

    /// Span from token `start` through the last consumed token.
    fn span_from(&self, start: usize) -> Span {
        let first = self.span_of(start);
        if self.pos > start {
            first.to(self.span_of(self.pos - 1))
        } else {
            first
        }
    }

    /// Error at the current token: unexpected token, or unexpected end.
    fn unexpected(&self) -> Located {
        let error = match self.peek() {
            Some(tok) => AbacusError::UnexpectedToken(tok.to_string()),
            None => AbacusError::UnexpectedEnd,
        };
        Located::new(error, Some(self.here()), None)
    }

    /// Consumes `expected`, or fails with `error` (an unexpected `,` is reported as such).
    fn expect(&mut self, expected: &Token<'_>, error: AbacusError) -> Result<(), Located> {
        match self.peek() {
            // Only used with unit-like tokens (`)`, `]`), so comparing variants is enough.
            Some(tok) if std::mem::discriminant(tok) == std::mem::discriminant(expected) => {
                self.advance();
                Ok(())
            }
            Some(Token::Comma) => Err(self.unexpected()),
            _ => Err(error).at(self.here()),
        }
    }

    /// Whether the next token can start a date expression.
    fn can_start_date_expr(&self) -> bool {
        match self.peek() {
            Some(Token::Date(_) | Token::OpenParen) => true,
            Some(Token::Function(name)) => {
                matches!(*name, "date" | "today" | "tomorrow" | "yesterday" | "now")
            }
            _ => false,
        }
    }

    /// Whether the next token can start an operand (used for the right side of `after`).
    fn can_start_operand(&self) -> bool {
        match self.peek() {
            Some(
                Token::Val(_)
                | Token::Float(_)
                | Token::Unit(_)
                | Token::Ident(_)
                | Token::Date(_)
                | Token::OpenParen
                | Token::OpenBracket
                | Token::Function(_)
                | Token::PercentChangeFrom,
            ) => true,
            Some(Token::UnaryOp(name)) => self
                .token_registry
                .unary_operators
                .get(*name)
                .is_some_and(|op| op.prefix),
            _ => false,
        }
    }

    // ── Binding powers ──

    /// (`left_bp`, `right_bp`) of a binary operator, derived from its precedence
    /// (`+ -` = 0 -> (2, 3), `* / % of` = 1 -> (4, 5), `^` = 3 right-assoc -> (9, 8)).
    fn infix_bp(op: &BinaryOp) -> (u8, u8) {
        let base = op.precedence.saturating_mul(2).saturating_add(2);
        if op.right_associative {
            (base.saturating_add(1), base)
        } else {
            (base, base.saturating_add(1))
        }
    }

    /// Binding power of a unary operator (`-`, `sqrt` = 2 -> 6; `!` = 4 -> 10).
    fn unary_bp(&self, name: &str) -> u8 {
        self.token_registry
            .unary_operators
            .get(name)
            .map_or(10, |op| op.precedence.saturating_mul(2).saturating_add(2))
    }

    // ── Expressions ──

    /// Parses the whole token stream.
    pub(crate) fn parse(&mut self) -> Result<NodeId, Located> {
        let node = self.parse_expr(0)?;
        if self.pos < self.tokens.len() {
            return Err(self.unexpected());
        }
        Ok(node)
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<NodeId, Located> {
        if self.recursion_depth >= self.max_depth {
            return Err(AbacusError::RecursionLimitExceeded).at(self.here());
        }
        let here = stack_position();
        if self.recursion_depth == 0 {
            self.stack_base = here;
        } else if self.stack_base.abs_diff(here) > STACK_BUDGET_BYTES {
            return Err(AbacusError::RecursionLimitExceeded).at(self.here());
        }
        self.recursion_depth += 1;
        let res = self.parse_expr_inner(min_bp);
        self.recursion_depth -= 1;
        res
    }

    fn parse_expr_inner(&mut self, min_bp: u8) -> Result<NodeId, Located> {
        let start = self.pos;
        let head = self.parse_prefix()?;
        // This chain's operations are `led_scratch[mark..]`; nested chains push and drain
        // their own above them.
        let mark = self.led_scratch.len();

        loop {
            let op_pos = self.pos;
            let op_span = self.here();
            let led = |kind| Led {
                kind,
                span: op_span,
            };
            match self.peek() {
                // `18% tip`: a descriptor after a percentage, at any binding power.
                Some(Token::PercentTag(tag)) => {
                    self.advance();
                    self.led_scratch.push(led(LedKind::PercentTag(tag)));
                }

                Some(Token::UnaryOp(name))
                    if self
                        .token_registry
                        .unary_operators
                        .get(*name)
                        .is_some_and(|op| !op.prefix || op.alias == "++" || op.alias == "--") =>
                {
                    if self.unary_bp(name) < min_bp {
                        break;
                    }
                    let op = Arc::clone(&self.token_registry.unary_operators[*name]);
                    self.advance();
                    self.led_scratch.push(led(LedKind::Postfix(op)));
                }

                Some(Token::BinaryOp(name)) => {
                    let Some(op) = self.token_registry.binary_operators.get(*name) else {
                        // Unknown operators bind at 0: report them at the outermost level.
                        if min_bp > 0 {
                            break;
                        }
                        return Err(self.unexpected());
                    };
                    let (l_bp, r_bp) = Self::infix_bp(op);
                    if l_bp < min_bp {
                        break;
                    }
                    let op = Arc::clone(op);
                    self.advance();
                    let rhs = self.parse_expr(r_bp)?;
                    self.led_scratch.push(led(LedKind::Binary { op, rhs }));
                }

                Some(Token::Range) if self.function_arg_depth == 0 => {
                    if RANGE_BP.0 < min_bp {
                        break;
                    }
                    self.advance();
                    let hi = self.parse_expr(RANGE_BP.1)?;
                    self.led_scratch.push(led(LedKind::Range { hi }));
                }

                Some(Token::ConversionOp) => {
                    if CONVERSION_BP < min_bp {
                        break;
                    }
                    let lhs_has_unit = self.tokens_contain_unit(start, op_pos);
                    self.advance();
                    // Whether the value converted so far could be a date (see `parse_conversion`).
                    let lhs_may_be_date =
                        self.ast.chain_may_be_date(head, &self.led_scratch[mark..]);
                    let convert = self.parse_conversion(lhs_has_unit, lhs_may_be_date)?;
                    self.led_scratch
                        .push(led(LedKind::Convert(Box::new(convert))));
                }

                Some(Token::RelTimeOp(name)) => {
                    if REL_TIME_BP < min_bp {
                        break;
                    }
                    self.advance();
                    let kind = match *name {
                        "ago" => LedKind::Ago,
                        "from_now" => LedKind::FromNow,
                        "until" => LedKind::Until {
                            target: self.parse_expr(REL_TIME_BP)?,
                        },
                        "before" => LedKind::Before {
                            date: if self.can_start_date_expr() {
                                Some(self.parse_expr(REL_TIME_BP)?)
                            } else {
                                None
                            },
                        },
                        _ => LedKind::After(self.parse_after()?),
                    };
                    self.led_scratch.push(led(kind));
                }

                Some(Token::DotProperty(prop)) => {
                    self.advance();
                    self.led_scratch
                        .push(led(LedKind::Property((*prop).into())));
                }

                _ => break,
            }
        }

        if self.led_scratch.len() == mark {
            return Ok(head);
        }
        let ops = self.ast.push_leds(&mut self.led_scratch, mark);
        let span = self.span_from(start);
        Ok(self.ast.push(Kind::Chain { head, ops }, span))
    }

    /// Parses what follows `after` (already consumed). Whether it is date arithmetic
    /// (`3 days after 07-08-2026`) or a percentage increase (`$50 after 15% tax`) depends on
    /// the left side's value, so both readings are kept.
    fn parse_after(&mut self) -> Result<After, Located> {
        let rhs_is_date = self.can_start_date_expr();
        let rhs = if rhs_is_date || self.can_start_operand() {
            Some(self.parse_expr(REL_TIME_BP)?)
        } else {
            None
        };
        let tag = match self.peek() {
            Some(Token::PercentTag(tag)) if rhs.is_some() => {
                let span = self.here();
                self.advance();
                Some((*tag, span))
            }
            _ => None,
        };
        Ok(After {
            rhs,
            rhs_is_date,
            tag,
        })
    }

    /// Parses an operand (NUD).
    fn parse_prefix(&mut self) -> Result<NodeId, Located> {
        let start = self.pos;
        let span = self.here();
        let Some(tok) = self.peek() else {
            return Err(self.unexpected());
        };
        self.advance();
        let kind = match tok {
            Token::Val(val) => Kind::Value(val.clone()),
            Token::Float(num) => Kind::Number(*num),
            Token::Ident(name) => Kind::Ident((*name).into()),
            Token::Unit(sym) => Kind::Unit(self.unit_registry.unit(sym).at(span)?),
            Token::Date(d) => {
                self.has_dates = true;
                Kind::Date(Box::new(d.clone()))
            }

            // `in 3 hours`, `before 07-08-2026`, `until 2026-12-25`
            Token::ConversionOp => Kind::RelPrefix {
                op: RelPrefix::Forward,
                operand: self.parse_expr(4)?,
            },
            Token::RelTimeOp(name) => Kind::RelPrefix {
                op: match *name {
                    "until" => RelPrefix::Until,
                    "before" | "ago" => RelPrefix::Backward,
                    _ => RelPrefix::Forward,
                },
                operand: self.parse_expr(4)?,
            },

            Token::PercentChangeFrom => {
                let from = self.parse_expr(2)?;
                if matches!(self.peek(), Some(Token::ConversionOp)) {
                    self.advance();
                } else {
                    return Err(AbacusError::EvaluationError(
                        "expected 'to' in percentage change expression".to_string(),
                    ))
                    .at(self.here());
                }
                let to = self.parse_expr(2)?;
                Kind::PercentChange { from, to }
            }

            // Grouping: ( expr ). Inside parentheses `..` is an interval again.
            Token::OpenParen => {
                let prev_depth = std::mem::replace(&mut self.function_arg_depth, 0);
                let inner = self.parse_expr(0);
                self.function_arg_depth = prev_depth;
                let inner = inner?;
                self.expect(&Token::CloseParen, AbacusError::UnclosedParen)?;
                return Ok(inner);
            }

            // Interval: [ lo , hi ]
            Token::OpenBracket => {
                let prev_depth = std::mem::replace(&mut self.function_arg_depth, 0);
                let bounds = self.parse_bracket_bounds();
                self.function_arg_depth = prev_depth;
                let (lo, hi) = bounds?;
                self.expect(&Token::CloseBracket, AbacusError::UnclosedBracket)?;
                Kind::Interval { lo, hi }
            }

            Token::BinaryOp("-") => {
                let op = self
                    .token_registry
                    .unary_operators
                    .get("-")
                    .cloned()
                    .ok_or_else(|| AbacusError::UnexpectedToken("-".to_string()))
                    .at(span)?;
                let operand = self.parse_expr(self.unary_bp("-"))?;
                Kind::Prefix { op, operand }
            }

            // Unary plus is the identity (`+5`, `2++3` = 2 + (+3)).
            Token::BinaryOp("+") => return self.parse_expr(self.unary_bp("-")),

            Token::UnaryOp(name) => {
                let operand = self.parse_expr(self.unary_bp(name))?;
                let op = self
                    .token_registry
                    .unary_operators
                    .get(*name)
                    .cloned()
                    .ok_or_else(|| AbacusError::UnexpectedToken((*name).to_string()))
                    .at(span)?;
                Kind::Prefix { op, operand }
            }

            Token::Function(name) => self.parse_function_call(name, span)?,

            _ => {
                self.pos -= 1;
                return Err(self.unexpected());
            }
        };
        let span = self.span_from(start);
        Ok(self.ast.push(kind, span))
    }

    /// Parses `lo , hi` inside `[ ... ]` (the opening bracket is already consumed).
    fn parse_bracket_bounds(&mut self) -> Result<(NodeId, NodeId), Located> {
        let lo = self.parse_expr(0)?;
        if matches!(self.peek(), Some(Token::Comma)) {
            self.advance();
        } else {
            return Err(AbacusError::UnexpectedToken(
                "expected ',' between interval bounds".to_string(),
            ))
            .at(self.here());
        }
        let hi = self.parse_expr(0)?;
        Ok((lo, hi))
    }

    // ── Function calls ──

    fn parse_function_call(
        &mut self,
        name: &'static str,
        name_span: Span,
    ) -> Result<Kind, Located> {
        let func = self
            .token_registry
            .function_operators
            .get(name)
            .cloned()
            .ok_or_else(|| AbacusError::UnknownFunction(name.to_string()))
            .at(name_span)?;

        // This call's arguments are `arg_scratch[mark..]`; nested calls push and drain their
        // own above them.
        let mark = self.arg_scratch.len();
        if matches!(self.peek(), Some(Token::OpenParen)) {
            self.advance();
            self.function_arg_depth += 1;
            let parsed = self.parse_arguments();
            self.function_arg_depth -= 1;
            parsed?;
            self.expect(&Token::CloseParen, AbacusError::UnclosedParen)?;
        } else {
            // A single unparenthesized argument (`sin 13deg`, `cos 45 deg`).
            let arg = self.parse_expr(BARE_ARG_BP)?;
            self.arg_scratch.push(Arg::Expr(arg));
        }
        let args = self.ast.push_args(&mut self.arg_scratch, mark);
        Ok(Kind::Call { func, args })
    }

    fn parse_arguments(&mut self) -> Result<(), Located> {
        if matches!(self.peek(), Some(Token::CloseParen)) {
            return Ok(());
        }
        loop {
            let arg = self.parse_range_endpoint()?;
            if matches!(self.peek(), Some(Token::Range)) {
                self.advance();
                let end = self.parse_range_endpoint()?;
                let step = if matches!(self.peek(), Some(Token::Range)) {
                    self.advance();
                    Some(self.parse_range_endpoint()?)
                } else {
                    None
                };
                self.arg_scratch.push(Arg::Range {
                    start: arg,
                    end,
                    step,
                });
            } else {
                self.arg_scratch.push(Arg::Expr(arg));
            }
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            } else {
                return Ok(());
            }
        }
    }

    /// Parses a function argument, rejecting range endpoints such as `max(1..2*3)` whose
    /// top-level operators bind looser than `..`. At the top level `1..2*3` means `(1..2)*3`, so
    /// silently reading it as `1..(2*3)` inside a function call would give a different answer;
    /// parentheses are required instead.
    fn parse_range_endpoint(&mut self) -> Result<NodeId, Located> {
        let start = self.pos;
        let node = self.parse_expr(0)?;
        let is_range_part = matches!(self.peek(), Some(Token::Range))
            || (start > 0 && matches!(self.tokens.get(start - 1), Some(Token::Range)));
        if is_range_part {
            self.check_range_endpoint(start)?;
        }
        Ok(node)
    }

    fn check_range_endpoint(&self, start: usize) -> Result<(), Located> {
        let mut depth = 0usize;
        let mut prev_is_operand = false;
        for (i, tok) in self.tokens[start..self.pos].iter().enumerate() {
            let at = self.span_of(start + i);
            match tok {
                Token::OpenParen | Token::OpenBracket => {
                    depth += 1;
                    prev_is_operand = false;
                }
                Token::CloseParen | Token::CloseBracket => {
                    depth = depth.saturating_sub(1);
                    prev_is_operand = true;
                }
                Token::BinaryOp(name) => {
                    let loose = self
                        .token_registry
                        .binary_operators
                        .get(*name)
                        .is_none_or(|op| Self::infix_bp(op).0 < RANGE_BP.1);
                    if depth == 0 && prev_is_operand && loose {
                        return Err(AbacusError::UnexpectedToken(format!(
                            "'{name}' in a range endpoint; parenthesize it, e.g. sum(1..(n * 2))"
                        )))
                        .at(at);
                    }
                    prev_is_operand = false;
                }
                Token::ConversionOp | Token::RelTimeOp(_) if depth == 0 => {
                    return Err(AbacusError::UnexpectedToken(
                        "conversion in a range endpoint; parenthesize it".to_string(),
                    ))
                    .at(at);
                }
                Token::UnaryOp(name) => {
                    // Postfix operators (`!`) keep the operand; prefix ones start a new one.
                    prev_is_operand = prev_is_operand
                        && self
                            .token_registry
                            .unary_operators
                            .get(*name)
                            .is_some_and(|op| !op.prefix);
                }
                Token::Function(_) | Token::Comma => prev_is_operand = false,
                _ => prev_is_operand = true,
            }
        }
        Ok(())
    }

    // ── Conversions ──

    /// Returns true if `tokens[start..end]` contains a written unit (`m`, `%`, `5 kg`, `$3`).
    fn tokens_contain_unit(&self, start: usize, end: usize) -> bool {
        self.tokens[start..end.min(self.tokens.len())]
            .iter()
            .any(|tok| match tok {
                Token::Unit(_) => true,
                Token::Val(v) => !v.unit.is_plain_number(),
                _ => false,
            })
    }

    /// Parses the target of `in`/`to`/`as` (the keyword is already consumed).
    fn parse_conversion(
        &mut self,
        lhs_has_unit: bool,
        lhs_may_be_date: bool,
    ) -> Result<Convert, Located> {
        self.has_explicit_conversion = true;
        let convert = |target| Convert {
            lhs_has_unit,
            target,
        };

        // `x as % of base`
        let is_pct_of = match (self.peek(), self.peek_at(1)) {
            (Some(Token::Unit(u)), Some(&Token::BinaryOp("of"))) => {
                matches!(*u, "%" | "percent" | "pct")
            }
            (Some(Token::Val(v)), Some(&Token::BinaryOp("of"))) => v.unit.is_percent(),
            _ => false,
        };
        if is_pct_of {
            self.advance();
            self.advance();
            return Ok(convert(Target::PercentOf(self.parse_expr(CONVERSION_BP)?)));
        }

        // Targets that only make sense after a date are recognised from the tokens alone, when
        // the left side may be a date; the evaluator falls back to the general meaning if it
        // turns out not to be one.
        let ends_target = |tok: Option<&Token<'_>>| {
            matches!(
                tok,
                None | Some(Token::ConversionOp | Token::CloseParen | Token::Comma)
            )
        };
        match self.peek() {
            _ if !lhs_may_be_date => {}
            // A bare hour (`today at 12 to 1`).
            Some(Token::Float(h))
                if ends_target(self.peek_at(1)) && h.fract() == 0.0 && (0.0..=24.0).contains(h) =>
            {
                let h = *h;
                self.advance();
                return Ok(convert(Target::BareHour(h)));
            }
            Some(Token::Date(d)) => {
                let d = Box::new(d.clone());
                self.has_dates = true;
                self.advance();
                return Ok(convert(Target::Date(d)));
            }
            Some(Token::Unit(sym)) => {
                let exact = crate::TimeZone::SUPPORTED_TIMEZONES
                    .iter()
                    .any(|(name, _)| name == sym)
                    || crate::TimeZone::is_literal_offset(sym);
                if (exact || ends_target(self.peek_at(1)))
                    && let Ok(tz) = crate::TimeZone::parse(sym)
                {
                    self.advance();
                    let unit = self.unit_registry.unit(sym);
                    return Ok(convert(Target::Zone { tz, unit }));
                }
            }
            _ => {}
        }

        let target_start = self.pos;
        let node = self.parse_expr(CONVERSION_BP)?;
        let target = &self.tokens[target_start..self.pos];
        let has_magnitude = target.iter().enumerate().any(|(i, tok)| {
            matches!(tok, Token::Val(_))
                || (matches!(tok, Token::Float(_))
                    && !matches!(
                        i.checked_sub(1).map(|p| &target[p]),
                        Some(Token::BinaryOp("^"))
                    ))
        });
        Ok(convert(Target::Expr {
            node,
            has_magnitude,
        }))
    }
}
