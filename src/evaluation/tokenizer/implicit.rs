use crate::{
    AbacusError, UnitRegistry, Value,
    evaluation::tokenizer::{
        lexer::Lexed,
        registry::token_registry::TokenRegistry,
        tokens::{Span, Token},
    },
};

/// The multiplier for a number word ("million" -> 1e6).
#[must_use]
pub fn number_scale_factor(word: &str) -> Option<f64> {
    match word {
        "hundred" | "hundreds" => Some(100.0),
        "thousand" | "thousands" => Some(1_000.0),
        "million" | "millions" => Some(1_000_000.0),
        "billion" | "billions" => Some(1_000_000_000.0),
        "trillion" | "trillions" => Some(1_000_000_000_000.0),
        "quadrillion" | "quadrillions" => Some(1e15),
        "quintillion" | "quintillions" => Some(1e18),
        "sextillion" | "sextillions" => Some(1e21),
        "septillion" | "septillions" => Some(1e24),
        "googol" | "googols" => Some(1e100),
        "dozen" | "dozens" => Some(12.0),
        "bakers_dozen" | "bakers_dozens" | "baker_dozen" => Some(13.0),
        "gross" | "grosses" => Some(144.0),
        "myriad" | "myriads" => Some(10_000.0),
        _ => None,
    }
}

/// Whether `c` is a supported currency symbol.
#[inline]
#[must_use]
pub fn is_currency_symbol(c: char) -> bool {
    matches!(c, '$' | '€' | '£' | '¥' | '₹' | '₩' | '₺' | '₪' | '฿')
}

/// Folds a compound measurement such as `5 ft 3 in` or `1 h 30 min` into a single value.
///
/// Only applies to adjacent values of the same dimensions written largest unit first. Never
/// applies to percentages, currencies or affine (offset) units, where juxtaposition has no
/// conventional "sum" reading.
fn fold_compound_measurement(lhs: &Value, rhs: &Value) -> Option<Value> {
    let foldable = |v: &Value| {
        !v.unit.is_percent()
            && !v.unit.is_affine()
            && !v.unit.is_dimensionless()
            && v.unit.dimensions != crate::units::dimensions::Dimensions::CURRENCY
    };
    if foldable(lhs)
        && foldable(rhs)
        && lhs.unit.dimensions == rhs.unit.dimensions
        && lhs.unit.scalar > rhs.unit.scalar
    {
        Some(Value {
            canonical: lhs.canonical + rhs.canonical,
            unit: std::sync::Arc::clone(&lhs.unit),
        })
    } else {
        None
    }
}

/// Folds numbers into their units and inserts implicit `BinaryOp("*")` between adjacent terms.
///
/// Two bare numbers next to each other (`1 000`, `5 m 3`) are rejected rather than silently
/// multiplied: they almost always mean a typo or a thousands separator.
pub(crate) fn resolve_tokens<'a>(
    lexed: Lexed<'a>,
    unit_registry: &UnitRegistry,
    token_registry: &TokenRegistry,
    implicit_multiplication: bool,
    number_scales: bool,
) -> Result<Lexed<'a>, (AbacusError, Span)> {
    let Lexed {
        mut tokens,
        mut spans,
    } = lexed;
    // 1. Fold a number and a following unit into a `Val` (`5.0 km`), fold prefix currency
    // (`€ 100`, `$ 3 million`), and apply scale words (`3 million km`) if enabled.
    //
    // Done in place: `tokens[..w]` is the resolved output and `tokens[r..]` the unread input.
    // Each step reads at least one token and writes at most one, so `w < r` whenever a token is
    // written and unread input is never overwritten. `spans` is kept in step with `tokens`.
    let len = tokens.len();
    let (mut r, mut w) = (0, 0);
    while r < len {
        let first = r;
        let tok = std::mem::replace(&mut tokens[r], Token::Comma);
        r += 1;
        // Span of the input tokens consumed so far by this step.
        let consumed = |r: usize, spans: &[Span]| spans[first].to(spans[r - 1]);
        // Prefix currency: `$ 50`, `€100`, `$ 3 million`
        if let Token::Unit(unit_sym) = tok
            && let Ok(unit) = unit_registry.unit(unit_sym)
            && unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY
        {
            if let Some(&Token::Float(mut num)) = tokens.get(r) {
                r += 1;
                if number_scales {
                    while let Some(Token::Unit(scale_sym)) = tokens.get(r) {
                        if let Some(scale) = number_scale_factor(scale_sym) {
                            r += 1;
                            num *= scale;
                            continue;
                        }
                        break;
                    }
                }
                let span = consumed(r, &spans);
                emit(
                    &mut tokens,
                    &mut spans,
                    &mut w,
                    Token::Val(Value::new(num, unit)),
                    span,
                );
                continue;
            }
            let span = consumed(r, &spans);
            emit(&mut tokens, &mut spans, &mut w, Token::Unit(unit_sym), span);
            continue;
        }

        if let Token::Float(mut num) = tok {
            if number_scales {
                // Chained scale words: `100 thousand million`
                while let Some(Token::Unit(unit_sym)) = tokens.get(r) {
                    if let Some(scale) = number_scale_factor(unit_sym) {
                        r += 1;
                        num *= scale;
                        continue;
                    }
                    break;
                }
            }

            // A following unit: `5.0 km`, `3 million km`
            if let Some(&Token::Unit(unit_sym)) = tokens.get(r)
                && let Ok(unit) = unit_registry.unit(unit_sym)
                && (!number_scales || number_scale_factor(unit_sym).is_none())
            {
                r += 1;
                let span = consumed(r, &spans);
                push_value(&mut tokens, &mut spans, &mut w, Value::new(num, unit), span);
                continue;
            }
            let span = consumed(r, &spans);
            emit(&mut tokens, &mut spans, &mut w, Token::Float(num), span);
        } else if let Token::Val(v) = tok {
            let span = consumed(r, &spans);
            push_value(&mut tokens, &mut spans, &mut w, v, span);
        } else {
            let span = consumed(r, &spans);
            emit(&mut tokens, &mut spans, &mut w, tok, span);
        }
    }
    tokens.truncate(w);
    spans.truncate(w);
    let mut resolved = tokens;

    // Reject juxtaposed bare numbers (`1 000`, `1.2.3`, `5 m 3`) and stray trailing currency
    // symbols (`$5 $`), which would otherwise be silently multiplied.
    for (i, pair) in resolved.windows(2).enumerate() {
        let at = spans[i + 1];
        match (&pair[0], &pair[1]) {
            (Token::Float(_) | Token::Val(_), Token::Float(n)) => {
                return Err((
                    AbacusError::UnexpectedToken(format!(
                        "adjacent numbers without an operator near '{n}'"
                    )),
                    at,
                ));
            }
            (Token::Float(_), Token::Val(v)) => {
                return Err((
                    AbacusError::UnexpectedToken(format!(
                        "adjacent numbers without an operator near '{v}'"
                    )),
                    at,
                ));
            }
            (Token::Val(v), Token::Unit(sym))
                if v.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY
                    && sym.chars().count() == 1
                    && sym.chars().all(is_currency_symbol) =>
            {
                return Err((AbacusError::UnexpectedToken((*sym).to_string()), at));
            }
            _ => {}
        }
    }

    if !implicit_multiplication {
        return Ok(Lexed {
            tokens: resolved,
            spans,
        });
    }

    // 2. Insert implicit multiplication between adjacent terms (`5(2+3)`, `2 sqrt(9)`, `2x`).
    let is_left = |tok: &Token<'a>| {
        matches!(
            tok,
            Token::Val(_)
                | Token::Float(_)
                | Token::Unit(_)
                | Token::Ident(_)
                | Token::CloseParen
                | Token::CloseBracket
        )
    };

    let is_right = |tok: &Token<'a>| match tok {
        Token::Val(_)
        | Token::Float(_)
        | Token::Unit(_)
        | Token::Ident(_)
        | Token::OpenParen
        | Token::OpenBracket
        | Token::Function(_) => true,
        Token::UnaryOp(name) => token_registry
            .unary_operators
            .get(*name)
            .is_some_and(|op| op.prefix && op.alias != "++" && op.alias != "--"),
        _ => false,
    };

    // Also in place: count the insertions, grow the buffer once, then move tokens to their final
    // positions from the back. While handling token `i`, slots in `i + 1..dst` hold placeholders
    // and token `i + 1` already sits at `next_dst`. An inserted `*` gets an empty span at the
    // start of the token it precedes.
    let len = resolved.len();
    let insertions = (1..len)
        .filter(|&i| is_left(&resolved[i - 1]) && is_right(&resolved[i]))
        .count();
    if insertions > 0 {
        resolved.resize_with(len + insertions, || Token::Comma);
        spans.resize(len + insertions, Span::default());
        let mut dst = len + insertions;
        let mut next_dst: Option<usize> = None;
        for i in (0..len).rev() {
            if let Some(nd) = next_dst
                && is_left(&resolved[i])
                && is_right(&resolved[nd])
            {
                dst -= 1;
                resolved[dst] = Token::BinaryOp("*");
                let at = spans[nd].start;
                spans[dst] = Span::new(at, at);
            }
            dst -= 1;
            resolved.swap(i, dst);
            spans.swap(i, dst);
            next_dst = Some(dst);
        }
    }

    Ok(Lexed {
        tokens: resolved,
        spans,
    })
}

/// Writes `tok` at the in-place output cursor `w` (see [`resolve_tokens`]).
#[inline]
fn emit<'a>(
    tokens: &mut [Token<'a>],
    spans: &mut [Span],
    w: &mut usize,
    tok: Token<'a>,
    span: Span,
) {
    tokens[*w] = tok;
    spans[*w] = span;
    *w += 1;
}

/// Emits a value token, merging it into the preceding output value when the pair forms a
/// compound measurement (`5 ft 3 in` -> `5.25 ft`).
fn push_value<'a>(
    tokens: &mut [Token<'a>],
    spans: &mut [Span],
    w: &mut usize,
    v: Value,
    span: Span,
) {
    if *w > 0
        && let Token::Val(prev) = &mut tokens[*w - 1]
        && let Some(sum) = fold_compound_measurement(prev, &v)
    {
        *prev = sum;
        spans[*w - 1] = spans[*w - 1].to(span);
        return;
    }
    emit(tokens, spans, w, Token::Val(v), span);
}

#[cfg(all(test, feature = "units", feature = "currencies"))]
mod reference_tests {
    use super::*;
    use std::sync::Arc;

    // A straightforward allocating implementation, used to check that the in-place version
    // produces identical tokens and errors.
    /// Folds numbers into their units and inserts implicit `BinaryOp("*")` between adjacent terms.
    ///
    /// Two bare numbers next to each other (`1 000`, `5 m 3`) are rejected rather than silently
    /// multiplied: they almost always mean a typo or a thousands separator.
    fn resolve_tokens_reference<'a>(
        mut tokens: Vec<Token<'a>>,
        unit_registry: &UnitRegistry,
        token_registry: &TokenRegistry,
        implicit_multiplication: bool,
        number_scales: bool,
    ) -> Result<Vec<Token<'a>>, AbacusError> {
        // 1. Fold a number and a following unit into a `Val` (`5.0 km`), fold prefix currency
        // (`€ 100`, `$ 3 million`), and apply scale words (`3 million km`) if enabled.
        let mut resolved: Vec<Token<'a>> = Vec::with_capacity(tokens.len());
        // Drain (rather than consume) so `tokens`' buffer can be reused for the final output.
        let mut iter = tokens.drain(..).peekable();
        while let Some(tok) = iter.next() {
            // Prefix currency: `$ 50`, `€100`, `$ 3 million`
            if let Token::Unit(unit_sym) = tok
                && let Ok(unit) = unit_registry.unit(unit_sym)
                && unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY
            {
                if let Some(&Token::Float(mut num)) = iter.peek() {
                    iter.next();
                    if number_scales {
                        while let Some(Token::Unit(scale_sym)) = iter.peek() {
                            if let Some(scale) = number_scale_factor(scale_sym) {
                                iter.next();
                                num *= scale;
                                continue;
                            }
                            break;
                        }
                    }
                    resolved.push(Token::Val(Value::new(num, unit)));
                    continue;
                }
                resolved.push(Token::Unit(unit_sym));
                continue;
            }

            if let Token::Float(mut num) = tok {
                if number_scales {
                    // Chained scale words: `100 thousand million`
                    while let Some(Token::Unit(unit_sym)) = iter.peek() {
                        if let Some(scale) = number_scale_factor(unit_sym) {
                            iter.next();
                            num *= scale;
                            continue;
                        }
                        break;
                    }
                }

                // A following unit: `5.0 km`, `3 million km`
                if let Some(Token::Unit(unit_sym)) = iter.peek()
                    && let Ok(unit) = unit_registry.unit(unit_sym)
                    && (!number_scales || number_scale_factor(unit_sym).is_none())
                {
                    iter.next();
                    push_value_reference(&mut resolved, Value::new(num, unit));
                    continue;
                }
                resolved.push(Token::Float(num));
            } else if let Token::Val(v) = tok {
                push_value_reference(&mut resolved, v);
            } else {
                resolved.push(tok);
            }
        }
        drop(iter);

        // Reject juxtaposed bare numbers (`1 000`, `1.2.3`, `5 m 3`) and stray trailing currency
        // symbols (`$5 $`), which would otherwise be silently multiplied.
        for pair in resolved.windows(2) {
            match (&pair[0], &pair[1]) {
                (Token::Float(_) | Token::Val(_), Token::Float(n)) => {
                    return Err(AbacusError::UnexpectedToken(format!(
                        "adjacent numbers without an operator near '{n}'"
                    )));
                }
                (Token::Float(_), Token::Val(v)) => {
                    return Err(AbacusError::UnexpectedToken(format!(
                        "adjacent numbers without an operator near '{v}'"
                    )));
                }
                (Token::Val(v), Token::Unit(sym))
                    if v.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY
                        && sym.chars().count() == 1
                        && sym.chars().all(is_currency_symbol) =>
                {
                    return Err(AbacusError::UnexpectedToken((*sym).to_string()));
                }
                _ => {}
            }
        }

        if !implicit_multiplication {
            return Ok(resolved);
        }

        // 2. Insert implicit multiplication between adjacent terms (`5(2+3)`, `2 sqrt(9)`, `2x`).
        let is_left = |tok: &Token<'a>| {
            matches!(
                tok,
                Token::Val(_)
                    | Token::Float(_)
                    | Token::Unit(_)
                    | Token::Ident(_)
                    | Token::CloseParen
                    | Token::CloseBracket
            )
        };

        let is_right = |tok: &Token<'a>| match tok {
            Token::Val(_)
            | Token::Float(_)
            | Token::Unit(_)
            | Token::Ident(_)
            | Token::OpenParen
            | Token::OpenBracket
            | Token::Function(_) => true,
            Token::UnaryOp(name) => token_registry
                .unary_operators
                .get(*name)
                .is_some_and(|op| op.prefix && op.alias != "++" && op.alias != "--"),
            _ => false,
        };

        // Reuse the (now empty) input buffer; it already holds at least `resolved.len()` tokens.
        let mut final_tokens = tokens;
        let mut iter = resolved.into_iter().peekable();
        while let Some(tok) = iter.next() {
            let insert_mul = if let Some(next_tok) = iter.peek() {
                is_left(&tok) && is_right(next_tok)
            } else {
                false
            };
            final_tokens.push(tok);
            if insert_mul {
                final_tokens.push(Token::BinaryOp("*"));
            }
        }

        Ok(final_tokens)
    }

    /// Pushes a value token, merging it into a preceding value when the pair forms a compound
    /// measurement (`5 ft 3 in` -> `5.25 ft`).
    fn push_value_reference<'a>(resolved: &mut Vec<Token<'a>>, v: Value) {
        if let Some(Token::Val(prev)) = resolved.last_mut()
            && let Some(sum) = fold_compound_measurement(prev, &v)
        {
            *prev = sum;
            return;
        }
        resolved.push(Token::Val(v));
    }

    #[test]
    fn in_place_resolution_matches_reference() {
        let units = UnitRegistry::standard();
        let ops = TokenRegistry::standard();
        let m = units.unit("m").unwrap();
        let ft = units.unit("ft").unwrap();
        let usd = units.unit("$").unwrap();
        let pool: Vec<Token<'static>> = vec![
            Token::Float(1.0),
            Token::Float(2.5),
            Token::Unit("m"),
            Token::Unit("ft"),
            Token::Unit("in"),
            Token::Unit("$"),
            Token::Unit("million"),
            Token::Unit("thousand"),
            Token::Val(Value::new(3.0, Arc::clone(&ft))),
            Token::Val(Value::new(4.0, Arc::clone(&m))),
            Token::Val(Value::new(5.0, Arc::clone(&usd))),
            Token::OpenParen,
            Token::CloseParen,
            Token::OpenBracket,
            Token::CloseBracket,
            Token::Ident("pi"),
            Token::Function("sin"),
            Token::UnaryOp("-"),
            Token::UnaryOp("sqrt"),
            Token::UnaryOp("++"),
            Token::BinaryOp("+"),
            Token::Comma,
        ];
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..100_000 {
            #[allow(clippy::cast_possible_truncation)]
            let len = (next() % 12) as usize;
            #[allow(clippy::cast_possible_truncation)]
            let toks: Vec<Token<'static>> = (0..len)
                .map(|_| pool[(next() % pool.len() as u64) as usize].clone())
                .collect();
            for (implicit, scales) in [(true, true), (true, false), (false, true)] {
                let expected =
                    resolve_tokens_reference(toks.clone(), &units, &ops, implicit, scales);
                let lexed = Lexed {
                    tokens: toks.clone(),
                    spans: (0..toks.len()).map(|i| Span::new(i, i + 1)).collect(),
                };
                let actual = resolve_tokens(lexed, &units, &ops, implicit, scales)
                    .map(|out| {
                        // Spans stay aligned with tokens and in source order.
                        assert_eq!(out.spans.len(), out.tokens.len());
                        assert!(out.spans.windows(2).all(|p| p[0].start <= p[1].start));
                        out.tokens
                    })
                    .map_err(|(error, _)| error);
                assert_eq!(
                    actual, expected,
                    "tokens {toks:?} implicit={implicit} scales={scales}"
                );
            }
        }
    }
}
