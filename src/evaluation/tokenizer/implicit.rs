use crate::{
    AbacusError, UnitRegistry, Value,
    evaluation::tokenizer::{registry::token_registry::TokenRegistry, tokens::Token},
};

/// Returns scale multiplier for natural number words (e.g. "million" -> 1e6).
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

/// Returns true if `c` is a supported currency symbol.
#[inline]
#[must_use]
pub fn is_currency_symbol(c: char) -> bool {
    matches!(c, '$' | '€' | '£' | '¥' | '₹' | '₩' | '₺' | '₪' | '฿')
}

/// Folds a compound measurement such as `5 ft 3 in` or `1 h 30 min` into a single value.
///
/// Only applies to adjacent values of identical dimensions written largest unit first, and never
/// to percentages, currencies, or affine (offset) units, where juxtaposition has no
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

/// Combines adjacent `Float + Unit` tokens and inserts implicit `BinaryOp("*")` between adjacent terms.
///
/// Two bare numbers next to each other (`1 000`, `2 3`, `5 m 3`) are rejected rather than
/// silently multiplied, since they almost always indicate a typo or a thousands separator.
pub(crate) fn resolve_tokens<'a>(
    tokens: Vec<Token<'a>>,
    unit_registry: &UnitRegistry,
    token_registry: &TokenRegistry,
    implicit_multiplication: bool,
    number_scales: bool,
) -> Result<Vec<Token<'a>>, AbacusError> {
    // 1. Combine adjacent Float + Unit into Val if separated by space (e.g. `5.0` + `km`),
    // fold prefix currency units (e.g. `$50`, `€ 100`, `$ 3 million`),
    // and scale numbers with scale words (e.g. `3 million`, `3 million km`) if enabled.
    let mut resolved: Vec<Token<'a>> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some(tok) = iter.next() {
        // Prefix currency: e.g. `$ 50`, `€100`, `$ 3 million`
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
                // Fold chained number scale words: e.g. `3 million`, `100 thousand million`
                while let Some(Token::Unit(unit_sym)) = iter.peek() {
                    if let Some(scale) = number_scale_factor(unit_sym) {
                        iter.next();
                        num *= scale;
                        continue;
                    }
                    break;
                }
            }

            // Check if followed by a physical unit (e.g. `5.0 km`, or `3 million km`)
            if let Some(Token::Unit(unit_sym)) = iter.peek()
                && let Ok(unit) = unit_registry.unit(unit_sym)
                && (!number_scales || number_scale_factor(unit_sym).is_none())
            {
                iter.next();
                push_value(&mut resolved, Value::new(num, unit));
                continue;
            }
            resolved.push(Token::Float(num));
        } else if let Token::Val(v) = tok {
            push_value(&mut resolved, v);
        } else {
            resolved.push(tok);
        }
    }

    // Reject juxtaposed bare numbers (`1 000`, `1.2.3`, `2 3`, `5 m 3`) and stray trailing
    // currency symbols (`$5 $`), which would otherwise be silently multiplied.
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

    // 2. Insert implicit multiplication `BinaryOp("*")` between adjacent terms (e.g. `5(2+3)`, `2 sqrt(9)`, `2 pi`, `2x`)
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

    let mut final_tokens: Vec<Token<'a>> = Vec::with_capacity(resolved.len() * 2);
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
fn push_value<'a>(resolved: &mut Vec<Token<'a>>, v: Value) {
    if let Some(Token::Val(prev)) = resolved.last_mut()
        && let Some(sum) = fold_compound_measurement(prev, &v)
    {
        *prev = sum;
        return;
    }
    resolved.push(Token::Val(v));
}
