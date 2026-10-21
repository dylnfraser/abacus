use crate::{
    AbacusError, UnitRegistry,
    evaluation::tokenizer::{
        date_literal::try_parse_date_literal_with_anchor,
        implicit::resolve_tokens,
        number::parse_number_token,
        registry::token_registry::{MatchedOpKind, TokenRegistry},
        tokens::Token,
    },
};

const CONVERSION_KEYWORDS: [&str; 2] = ["as", "to"];

/// Zero-allocation byte prefix matching for relative time keywords.
#[inline]
fn match_rel_time_op(remaining: &str) -> Option<(&'static str, usize)> {
    let b = remaining.as_bytes();
    let len = b.len();

    if len >= 8
        && b[..8].eq_ignore_ascii_case(b"from now")
        && (len == 8 || !b[8].is_ascii_alphanumeric())
    {
        return Some(("from_now", 8));
    }
    if len >= 4 && b[..4].eq_ignore_ascii_case(b"from") {
        let rest = remaining[4..].trim_start();
        let ws_len = remaining[4..].len() - rest.len();
        if ws_len > 0
            && rest.len() >= 3
            && rest.as_bytes()[..3].eq_ignore_ascii_case(b"now")
            && (rest.len() == 3 || !rest.as_bytes()[3].is_ascii_alphanumeric())
        {
            return Some(("from_now", 4 + ws_len + 3));
        }
    }
    if len >= 3
        && b[..3].eq_ignore_ascii_case(b"ago")
        && (len == 3 || !b[3].is_ascii_alphanumeric())
    {
        return Some(("ago", 3));
    }
    if len >= 6
        && b[..6].eq_ignore_ascii_case(b"before")
        && (len == 6 || !b[6].is_ascii_alphanumeric())
    {
        return Some(("before", 6));
    }
    if len >= 5
        && b[..5].eq_ignore_ascii_case(b"after")
        && (len == 5 || !b[5].is_ascii_alphanumeric())
    {
        return Some(("after", 5));
    }
    if len >= 5
        && b[..5].eq_ignore_ascii_case(b"until")
        && (len == 5 || !b[5].is_ascii_alphanumeric())
    {
        return Some(("until", 5));
    }
    if len >= 4
        && b[..4].eq_ignore_ascii_case(b"till")
        && (len == 4 || !b[4].is_ascii_alphanumeric())
    {
        return Some(("until", 4));
    }
    None
}

/// Case-insensitive ASCII prefix test that only inspects `prefix.len()` bytes of `s`.
#[inline]
fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// Returns true if `rest` does not continue an identifier (end of input or a non-word character).
#[inline]
fn at_word_boundary(rest: &str) -> bool {
    rest.chars()
        .next()
        .is_none_or(|nc| !nc.is_alphanumeric() && nc != '_')
}

/// Matches `phrase` case-insensitively at the start of `remaining`, followed by a word boundary.
#[inline]
fn match_phrase(remaining: &str, phrase: &str) -> bool {
    starts_with_ci(remaining, phrase)
        && remaining.is_char_boundary(phrase.len())
        && at_word_boundary(&remaining[phrase.len()..])
}

#[inline]
fn match_percent_change_from(remaining: &str) -> Option<usize> {
    const PREFIXES: &[&str] = &[
        "% change from",
        "% difference from",
        "percent change from",
        "percent difference from",
        "percentage change from",
        "percentage difference from",
    ];
    PREFIXES
        .iter()
        .find(|p| match_phrase(remaining, p))
        .map(|p| p.len())
}

#[inline]
fn match_conversational_binary_op(remaining: &str) -> Option<(&'static str, usize)> {
    const OPS: &[(&str, &str)] = &[
        ("more than", "more than"),
        ("less than", "less than"),
        ("out of", "/"),
    ];
    OPS.iter()
        .find(|(phrase, _)| match_phrase(remaining, phrase))
        .map(|&(phrase, alias)| (alias, phrase.len()))
}

#[inline]
fn match_percent_tag(remaining: &str) -> Option<(&'static str, usize)> {
    const TAGS: &[&str] = &["tip", "tax", "vat", "gst", "fee", "discount", "markup"];
    TAGS.iter()
        .find(|tag| match_phrase(remaining, tag))
        .map(|&tag| (tag, tag.len()))
}

/// Returns the leading identifier-like word of `s` (alphanumeric or `_`).
#[inline]
fn leading_word(s: &str) -> &str {
    let len = s
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    &s[..len]
}

/// Detects a timezone offset literal such as `+02:00`, `-04:00` or `+05:30` at the start of `s`,
/// scanning at most a few bytes. Returns the literal's byte length.
#[inline]
fn match_tz_offset_literal(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if !matches!(b.first(), Some(b'+' | b'-')) {
        return None;
    }
    let len = 1 + b[1..]
        .iter()
        .take(6)
        .take_while(|c| c.is_ascii_digit() || **c == b':')
        .count();
    let word = &s[..len];
    // Must be a complete word (e.g. not the start of `+05:30abc` or `-1:30.5`).
    if !word.contains(':') || s[len..].starts_with(|c: char| c.is_alphanumeric() || c == '.') {
        return None;
    }
    crate::units::date::TimeZone::parse(word).ok().map(|_| len)
}

/// Decides whether a `%` (with `rest` being the text after it) is the modulo operator rather
/// than the percent unit: it is modulo only when an operand follows (`10 % 3`, `10%(3)`) and
/// the next word is not `of` (`50% of 200`). Whitespace around `%` never changes the meaning.
#[inline]
pub(crate) fn percent_is_modulo(rest: &str) -> bool {
    let after = rest.trim_start();
    if match_phrase(after, "of") {
        return false;
    }
    let mut it = after.chars();
    match it.next() {
        Some(c) if c.is_ascii_digit() || c == '(' => true,
        Some('.') => it.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// Returns true if `tok` ends an operand (so a following operator is infix/postfix).
#[inline]
fn is_operand_token(tok: Option<&Token<'_>>) -> bool {
    matches!(
        tok,
        Some(
            Token::Float(_)
                | Token::Val(_)
                | Token::Unit(_)
                | Token::Ident(_)
                | Token::Date(_)
                | Token::CloseParen
                | Token::CloseBracket
        )
    )
}

/// Resolves the English indefinite articles `a` / `an` in conversational input:
/// - `5 usd a second`, `60 miles an hour` -> `/` (rate),
/// - `a thousand`, `a meter`, `an hour`, `as a % of` -> the number `1`.
///
/// Capital `A` is the ampere and is only read as `1` before a number-scale word
/// (`A million dollars`). Returns `None` when `sym` should be lexed as a regular unit
/// (e.g. `3 a` = 3 ares, `10 V / A`).
fn resolve_indefinite_article<'a>(
    sym: &str,
    rest_after: &str,
    prev: Option<&Token<'a>>,
    unit_registry: &UnitRegistry,
) -> Option<Token<'a>> {
    let is_cap_a = sym == "A";
    if !is_cap_a && sym != "a" && !sym.eq_ignore_ascii_case("an") {
        return None;
    }
    let rest = rest_after.trim_start();
    let next_word = leading_word(rest);
    let next_is_scale = crate::evaluation::tokenizer::implicit::number_scale_factor(next_word)
        .is_some();
    if is_cap_a {
        return next_is_scale.then_some(Token::Float(1.0));
    }
    let next_is_unit = !next_word.is_empty()
        && !next_is_scale
        && next_word != "in"
        && !CONVERSION_KEYWORDS.contains(&next_word)
        && unit_registry.contains(next_word);
    let prev_is_quantity = matches!(prev, Some(Token::Unit(_) | Token::Val(_) | Token::Float(_)));

    if prev_is_quantity && next_is_unit {
        Some(Token::BinaryOp("/"))
    } else if !prev_is_quantity && (next_is_scale || next_is_unit || rest.starts_with('%')) {
        Some(Token::Float(1.0))
    } else {
        None
    }
}

/// Returns true if `sym` is a standard mathematical constant name.
#[inline]
pub(crate) fn is_standard_constant(sym: &str) -> bool {
    crate::evaluation::tokenizer::registry::constants::is_standard_constant(sym)
}

/// Tokenize an expression string using default settings (implicit multiplication enabled).
pub fn tokenize_string<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input_text: &'a str,
) -> Result<Vec<Token<'a>>, AbacusError> {
    tokenize_string_full(
        token_registry,
        unit_registry,
        None,
        input_text,
        true,
        cfg!(feature = "number-scales"),
    )
}

/// Tokenize an expression string with configurable options.
pub fn tokenize_string_with_options<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    input_text: &'a str,
    implicit_multiplication: bool,
) -> Result<Vec<Token<'a>>, AbacusError> {
    tokenize_string_full(
        token_registry,
        unit_registry,
        None,
        input_text,
        implicit_multiplication,
        cfg!(feature = "number-scales"),
    )
}

/// Tokenize an expression string with variable definitions and configurable options.
pub fn tokenize_string_full<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    input_text: &'a str,
    implicit_multiplication: bool,
    number_scales: bool,
) -> Result<Vec<Token<'a>>, AbacusError> {
    tokenize_string_with_anchor(
        token_registry,
        unit_registry,
        variables,
        input_text,
        implicit_multiplication,
        number_scales,
        None,
    )
}

/// Tokenize an expression string with variable definitions, evaluation options, and an optional anchor date.
pub fn tokenize_string_with_anchor<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    input_text: &'a str,
    implicit_multiplication: bool,
    number_scales: bool,
    anchor_date: Option<&crate::Date>,
) -> Result<Vec<Token<'a>>, AbacusError> {
    let mut tokens = Vec::new();
    let mut chars = input_text.char_indices().peekable();
    let op_index = token_registry.operator_index();
    // True while lexing the target of an `in`/`to`/`as` conversion, where unit names take
    // precedence over variables of the same name.
    let mut in_conversion_target = false;

    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        // Date literals (e.g. today, tomorrow, yesterday, now, @2026-08-07@, 07-08-2026, 12:00, 1:00 PM)
        let is_date_candidate = c == '@'
            || (c.is_ascii_digit() && {
                let rem_bytes = &input_text.as_bytes()[i..];
                let mut has_sep = false;
                let mut idx = 0;
                while idx < rem_bytes.len() && idx < 20 {
                    let b = rem_bytes[idx];
                    if b == b'-' || b == b'/' || b == b':' || b.is_ascii_alphabetic() {
                        has_sep = true;
                        break;
                    }
                    if b == b' '
                        || b == b'\t'
                        || b == b','
                        || b == b')'
                        || b == b']'
                        || b == b'+'
                        || b == b'*'
                    {
                        break;
                    }
                    idx += 1;
                }
                has_sep
            })
            || c.is_ascii_alphabetic();

        if is_date_candidate
            && let Some((date, consumed_bytes)) =
                try_parse_date_literal_with_anchor(&input_text[i..], anchor_date)
        {
            tokens.push(Token::Date(date));
            let target_idx = i + consumed_bytes;
            while let Some(&(idx, _)) = chars.peek() {
                if idx < target_idx {
                    chars.next();
                } else {
                    break;
                }
            }
            continue;
        }

        // Timezone offset literals (e.g. +02:00, -04:00, +05:30), only directly after a date/time
        // or as a conversion target, so `10 -1:30` stays arithmetic.
        if (c == '+' || c == '-')
            && matches!(tokens.last(), Some(Token::Date(_) | Token::ConversionOp))
            && let Some(len) = match_tz_offset_literal(&input_text[i..])
        {
            tokens.push(Token::Unit(&input_text[i..i + len]));
            let target_idx = i + len;
            while let Some(&(idx, _)) = chars.peek() {
                if idx < target_idx {
                    chars.next();
                } else {
                    break;
                }
            }
            continue;
        }

        // Percent change expressions (e.g. "% change from", "percent change from")
        if c == '%' || c == 'p' || c == 'P' {
            let remaining = &input_text[i..];
            if let Some(len) = match_percent_change_from(remaining) {
                tokens.push(Token::PercentChangeFrom);
                let target_idx = i + len;
                while let Some(&(idx, _)) = chars.peek() {
                    if idx < target_idx {
                        chars.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }

        // Percentage descriptor tags (e.g. "18% tip", "15% tax", "20% discount", "10% fee")
        if c.is_ascii_alphabetic() {
            let is_prev_pct = match tokens.last() {
                Some(Token::Unit(u)) => *u == "%" || *u == "percent" || *u == "pct",
                Some(Token::Val(v)) => v.unit.is_percent(),
                _ => false,
            };
            if is_prev_pct {
                let remaining = &input_text[i..];
                if let Some((tag, len)) = match_percent_tag(remaining) {
                    tokens.push(Token::PercentTag(tag));
                    let target_idx = i + len;
                    while let Some(&(idx, _)) = chars.peek() {
                        if idx < target_idx {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    continue;
                }
            }
        }

        // Conversational "off" (e.g. "20% off $120" or "$100 - 20% off")
        if (c == 'o' || c == 'O') && starts_with_ci(&input_text[i..], "off") {
            let remaining = &input_text[i..];
            let next_char = remaining[3..].chars().next();
            if next_char.is_none_or(|nc| !nc.is_alphanumeric() && nc != '_') {
                let after_off = remaining[3..].trim_start();
                let starts_expr = !after_off.is_empty()
                    && !after_off.starts_with(')')
                    && !after_off.starts_with(']')
                    && !after_off.starts_with(',');
                if starts_expr {
                    tokens.push(Token::BinaryOp("off"));
                } else {
                    tokens.push(Token::PercentTag("off"));
                }
                let target_idx = i + 3;
                while let Some(&(idx, _)) = chars.peek() {
                    if idx < target_idx {
                        chars.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }

        // Conversational binary operators (e.g. "more than", "less than", "out of")
        if c == 'm' || c == 'M' || c == 'l' || c == 'L' || c == 'o' || c == 'O' {
            let remaining = &input_text[i..];
            if let Some((op_name, len)) = match_conversational_binary_op(remaining) {
                tokens.push(Token::BinaryOp(op_name));
                let target_idx = i + len;
                while let Some(&(idx, _)) = chars.peek() {
                    if idx < target_idx {
                        chars.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }

        // Relative time operators (e.g. "ago", "from now", "before", "after")
        if c.is_ascii_alphabetic() {
            let remaining = &input_text[i..];
            if let Some((op_name, len)) = match_rel_time_op(remaining) {
                tokens.push(Token::RelTimeOp(op_name));
                let target_idx = i + len;
                while let Some(&(idx, _)) = chars.peek() {
                    if idx < target_idx {
                        chars.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }

        // Multi-word unit symbols (e.g. "business days", "business day", "work days", "work day", "working days", "working day")
        if c == 'b' || c == 'B' || c == 'w' || c == 'W' {
            const MULTI_WORD_UNITS: &[&str] = &[
                "business days",
                "business day",
                "working days",
                "working day",
                "work days",
                "work day",
            ];
            let remaining = &input_text[i..];
            let mut matched_len = 0;
            for &unit_name in MULTI_WORD_UNITS {
                if remaining.len() >= unit_name.len()
                    && remaining.as_bytes()[..unit_name.len()]
                        .eq_ignore_ascii_case(unit_name.as_bytes())
                {
                    let next_char = remaining[unit_name.len()..].chars().next();
                    if next_char.is_none_or(|nc| !nc.is_alphanumeric() && nc != '_') {
                        matched_len = unit_name.len();
                        break;
                    }
                }
            }
            if matched_len > 0 {
                let unit_str = &input_text[i..i + matched_len];
                tokens.push(Token::Unit(unit_str));
                let target_idx = i + matched_len;
                while let Some(&(idx, _)) = chars.peek() {
                    if idx < target_idx {
                        chars.next();
                    } else {
                        break;
                    }
                }
                continue;
            }
        }

        // Grouping Parentheses and Delimiters
        if c == '(' {
            tokens.push(Token::OpenParen);
            chars.next();
            continue;
        }

        if c == ')' {
            tokens.push(Token::CloseParen);
            in_conversion_target = false;
            chars.next();
            continue;
        }

        if c == ',' {
            tokens.push(Token::Comma);
            in_conversion_target = false;
            chars.next();
            continue;
        }

        if c == '[' {
            tokens.push(Token::OpenBracket);
            chars.next();
            continue;
        }

        if c == ']' {
            tokens.push(Token::CloseBracket);
            in_conversion_target = false;
            chars.next();
            continue;
        }

        if c == '.' {
            let mut dot_lookahead = chars.clone();
            dot_lookahead.next(); // skip '.'
            if let Some(&(_, '.')) = dot_lookahead.peek() {
                tokens.push(Token::Range);
                chars.next();
                chars.next();
                continue;
            } else if let Some(&(next_idx, next_c)) = dot_lookahead.peek()
                && (next_c.is_alphabetic() || next_c == '_')
            {
                chars.next(); // consume '.'
                let start = next_idx;
                let mut end = start;
                while let Some(&(idx, sym_c)) = chars.peek() {
                    if sym_c.is_alphanumeric() || sym_c == '_' {
                        end = idx + sym_c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::DotProperty(&input_text[start..end]));
                continue;
            }
        }

        // Registered operator checks (prioritizing longer length operators like `++` over `+`)
        if let Some(candidates) = op_index.candidates(c) {
            let remaining = &input_text[i..];
            let mut best_match: Option<(&str, MatchedOpKind)> = None;

            for (pattern, kind) in candidates {
                let (pattern, kind) = (&**pattern, *kind);
                if let Some(rest) = remaining.strip_prefix(pattern) {
                    match kind {
                        MatchedOpKind::Binary(_) => {
                            if pattern == "%" && !percent_is_modulo(rest) {
                                continue;
                            }
                            if let Some(last_char) = pattern.chars().last()
                                && (last_char.is_alphanumeric() || last_char == '_')
                                && let Some(next_char) = rest.chars().next()
                                && (next_char.is_alphanumeric()
                                    || next_char == '_'
                                    || next_char == '°'
                                    || next_char == 'Å'
                                    || next_char == 'Ω')
                            {
                                continue;
                            }
                        }
                        MatchedOpKind::Unary(op_alias) => {
                            // `++`/`--` directly after an operand are only postfix increment /
                            // decrement at the end of a (sub)expression; otherwise `3--2` and
                            // `2++3` are a binary operator followed by a unary sign.
                            if matches!(op_alias, "++" | "--")
                                && is_operand_token(tokens.last())
                                && !rest
                                    .trim_start()
                                    .chars()
                                    .next()
                                    .is_none_or(|nc| matches!(nc, ')' | ']' | ','))
                            {
                                continue;
                            }
                            if let Some(last_char) = pattern.chars().last()
                                && (last_char.is_alphanumeric() || last_char == '_')
                                && let Some(next_char) = rest.chars().next()
                                && (next_char.is_alphanumeric()
                                    || next_char == '_'
                                    || next_char == '°'
                                    || next_char == 'Å'
                                    || next_char == 'Ω')
                            {
                                continue;
                            }
                        }
                        MatchedOpKind::Func(_) => {
                            if unit_registry.contains(pattern) {
                                let next_slice = rest.trim_start();
                                if !next_slice.starts_with('(') {
                                    continue;
                                }
                            }
                            if let Some(last_char) = pattern.chars().last()
                                && (last_char.is_alphanumeric()
                                    || last_char == '_'
                                    || last_char == '-')
                                && let Some(next_char) = rest.chars().next()
                                && (next_char.is_alphanumeric()
                                    || next_char == '_'
                                    || next_char == '-'
                                    || next_char == '°'
                                    || next_char == 'Å'
                                    || next_char == 'Ω')
                            {
                                continue;
                            }
                        }
                    }

                    best_match = Some((pattern, kind));
                    break;
                }
            }

            if let Some((alias, matched_op)) = best_match {
                let char_count = alias.chars().count();
                for _ in 0..char_count {
                    chars.next();
                }
                match matched_op {
                    MatchedOpKind::Binary(op_alias) => tokens.push(Token::BinaryOp(op_alias)),
                    MatchedOpKind::Unary(op_alias) => tokens.push(Token::UnaryOp(op_alias)),
                    MatchedOpKind::Func(fn_name) => tokens.push(Token::Function(fn_name)),
                }
                continue;
            }
        }

        // Numbers (digit or starting with '.' followed by a digit)
        if c.is_ascii_digit()
            || (c == '.'
                && chars
                    .clone()
                    .nth(1)
                    .is_some_and(|(_, next_c)| next_c.is_ascii_digit()))
        {
            let shadowing_vars = if in_conversion_target {
                None
            } else {
                variables
            };
            let num_token = parse_number_token(
                input_text,
                i,
                &mut chars,
                unit_registry,
                shadowing_vars,
                number_scales,
            )?;
            tokens.push(num_token);
            continue;
        }

        // Currency symbols (e.g. $, €, £, ¥)
        if crate::evaluation::tokenizer::implicit::is_currency_symbol(c) {
            chars.next();
            let sym = &input_text[i..i + c.len_utf8()];
            if unit_registry.contains(sym) {
                tokens.push(Token::Unit(sym));
                continue;
            } else {
                return Err(AbacusError::UnknownUnit(sym.to_string()));
            }
        }

        // Identifiers (units, conversion operators, named unary ops like sqrt)
        // A bare `%` (percent unit) is always a single-character symbol so `20%of 50` works.
        if c == '%' {
            chars.next();
            let sym = &input_text[i..i + 1];
            if unit_registry.contains(sym) {
                tokens.push(Token::Unit(sym));
            } else if let Some(op) = token_registry.binary_operators.get(sym) {
                tokens.push(Token::BinaryOp(op.alias));
            } else {
                return Err(AbacusError::UnknownUnit(sym.to_string()));
            }
            continue;
        }

        if c.is_alphabetic() || c == '_' || c == '°' || c == 'Å' || c == 'Ω' {
            let start = i;
            let mut end = i;
            let vars_shadow = !in_conversion_target;
            while let Some(&(idx, sym_c)) = chars.peek() {
                if sym_c.is_alphanumeric()
                    || sym_c == '_'
                    || sym_c == '°'
                    || sym_c == 'Å'
                    || sym_c == 'Ω'
                {
                    end = idx + sym_c.len_utf8();
                    chars.next();
                } else if sym_c == '^' {
                    let base_sym = &input_text[start..end];
                    if is_standard_constant(base_sym)
                        || (vars_shadow && variables.is_some_and(|v| v.contains_key(base_sym)))
                        || !unit_registry.contains(base_sym)
                    {
                        break;
                    }
                    end = idx + sym_c.len_utf8();
                    chars.next();
                    if let Some(&(sign_idx, sign_c)) = chars.peek()
                        && (sign_c == '+' || sign_c == '-')
                    {
                        end = sign_idx + sign_c.len_utf8();
                        chars.next();
                    }
                    while let Some(&(digit_idx, digit_c)) = chars.peek() {
                        if digit_c.is_ascii_digit() || digit_c == '.' {
                            end = digit_idx + digit_c.len_utf8();
                            chars.next();
                        } else {
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            let sym = &input_text[start..end];

            if CONVERSION_KEYWORDS.contains(&sym) {
                tokens.push(Token::ConversionOp);
                in_conversion_target = true;
            } else if sym == "in" {
                let rest_trimmed = input_text[end..].trim_start();
                let has_no_target = rest_trimmed.is_empty()
                    || rest_trimmed.starts_with(|c: char| {
                        matches!(
                            c,
                            ')' | ']'
                                | ','
                                | ';'
                                | '+'
                                | '-'
                                | '*'
                                | '/'
                                | '^'
                                | '='
                                | '<'
                                | '>'
                                | '!'
                        )
                    });

                let next_word = leading_word(rest_trimmed);
                let is_followed_by_conversion_kw =
                    next_word == "to" || next_word == "as" || next_word == "in";

                let is_unit = match tokens.last() {
                    Some(Token::ConversionOp) => true,
                    Some(Token::Float(_)) => is_followed_by_conversion_kw || has_no_target,
                    Some(
                        Token::BinaryOp(_)
                        | Token::UnaryOp(_)
                        | Token::OpenParen
                        | Token::OpenBracket
                        | Token::Comma,
                    )
                    | None => is_followed_by_conversion_kw || has_no_target,
                    // After a unit/value (e.g. `5 in in`, `5 m in`), `in` is the conversion
                    // keyword; a missing target is then reported instead of silently
                    // multiplying by inches.
                    _ => false,
                };

                if is_unit {
                    tokens.push(Token::Unit(sym));
                } else {
                    tokens.push(Token::ConversionOp);
                    in_conversion_target = true;
                }
            } else if let Some(op) = token_registry.function_operators.get(sym) {
                let rest = input_text[end..].trim_start();
                if rest.starts_with('(') || !unit_registry.contains(sym) {
                    tokens.push(Token::Function(op.name));
                } else {
                    tokens.push(Token::Unit(sym));
                }
            } else if sym.eq_ignore_ascii_case("per") {
                tokens.push(Token::BinaryOp("/"));
            } else if let Some(op) = token_registry.binary_operators.get(sym) {
                tokens.push(Token::BinaryOp(op.alias));
            } else if let Some(op) = token_registry.unary_operators.get(sym) {
                tokens.push(Token::UnaryOp(op.alias));
            } else if in_conversion_target && unit_registry.contains(sym) {
                // Conversion targets are units even if a variable shadows the name.
                tokens.push(Token::Unit(sym));
            } else if is_standard_constant(sym) || variables.is_some_and(|v| v.contains_key(sym)) {
                tokens.push(Token::Ident(sym));
            } else if let Some(tok) = resolve_indefinite_article(
                sym,
                &input_text[end..],
                tokens.last(),
                unit_registry,
            ) {
                tokens.push(tok);
            } else if unit_registry.contains(sym) {
                tokens.push(Token::Unit(sym));
            } else if sym.contains('^') {
                unit_registry.unit(sym)?;
                tokens.push(Token::Unit(sym));
            } else {
                return Err(AbacusError::UnknownUnit(sym.to_string()));
            }

            continue;
        }

        return Err(AbacusError::UnknownUnit(c.to_string()));
    }

    resolve_tokens(
        tokens,
        unit_registry,
        token_registry,
        implicit_multiplication,
        number_scales,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::tokenizer::sig_figs::{
        count_significant_figures, min_significant_figures_in_expr,
    };

    #[test]
    fn tokenizes_spaced_and_unspaced_expressions() {
        let token_reg = TokenRegistry::standard();
        let unit_reg = UnitRegistry::standard();

        let tokens = tokenize_string(&token_reg, &unit_reg, "10m+5m").unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], Token::Val(_)));
        assert_eq!(tokens[1], Token::BinaryOp("+"));
        assert!(matches!(tokens[2], Token::Val(_)));

        let tokens2 = tokenize_string(&token_reg, &unit_reg, "1 bbl in m^3").unwrap();
        assert_eq!(tokens2.len(), 3);
        assert!(matches!(tokens2[0], Token::Val(_)));
        assert_eq!(tokens2[1], Token::ConversionOp);
        assert_eq!(tokens2[2], Token::Unit("m^3"));
    }

    #[test]
    fn tokenizes_parens_and_unary_operators() {
        let token_reg = TokenRegistry::standard();
        let unit_reg = UnitRegistry::standard();

        let tokens = tokenize_string(&token_reg, &unit_reg, "sqrt(9 m^2)").unwrap();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0], Token::UnaryOp("sqrt"));
        assert_eq!(tokens[1], Token::OpenParen);
        assert!(matches!(tokens[2], Token::Val(_)));
        assert_eq!(tokens[3], Token::CloseParen);
    }

    #[test]
    fn tokenizes_functions_and_commas() {
        let token_reg = TokenRegistry::standard();
        let unit_reg = UnitRegistry::standard();

        let tokens = tokenize_string(&token_reg, &unit_reg, "mean(10, 0.5, 5)").unwrap();
        assert_eq!(tokens[1], Token::OpenParen);
        assert_eq!(tokens[2], Token::Float(10.0));
        assert_eq!(tokens[3], Token::Comma);
        assert_eq!(tokens[4], Token::Float(0.5));
        assert_eq!(tokens[5], Token::Comma);
        assert_eq!(tokens[6], Token::Float(5.0));
        assert_eq!(tokens[7], Token::CloseParen);
    }

    #[test]
    fn executes_registered_functions() {
        let token_reg = TokenRegistry::standard();
        let unit_reg = UnitRegistry::standard();

        // sin(45 deg)
        let sin_op = &token_reg.function_operators["sin"];
        let angle = unit_reg.value(45.0, "deg").unwrap();
        let sin_res = sin_op.apply_scalar(&[angle]).unwrap();
        assert!(
            (sin_res.into_scalar().unwrap().canonical - (std::f64::consts::FRAC_1_SQRT_2)).abs()
                < 1e-10
        );

        // mean(10 m, 20 m, 30 m)
        let mean_op = &token_reg.function_operators["mean"];
        let v1 = unit_reg.value(10.0, "m").unwrap();
        let v2 = unit_reg.value(20.0, "m").unwrap();
        let v3 = unit_reg.value(30.0, "m").unwrap();
        let mean_res = mean_op.apply_scalar(&[v1, v2, v3]).unwrap();
        assert_eq!(mean_res.to_display(), "20 m");
    }

    #[test]
    fn fails_on_unknown_tokens() {
        let token_reg = TokenRegistry::standard();
        let unit_reg = UnitRegistry::standard();

        assert!(tokenize_string(&token_reg, &unit_reg, "xyz").is_err());
    }

    #[test]
    fn test_significant_figures_counting() {
        assert_eq!(count_significant_figures("12.30"), Some(4));
        assert_eq!(count_significant_figures("0.00450"), Some(3));
        assert_eq!(count_significant_figures("100."), Some(3));
        assert_eq!(count_significant_figures("1200"), Some(2));
        assert_eq!(count_significant_figures("1205"), Some(4));
        assert_eq!(count_significant_figures("1.23e4"), Some(3));
        assert_eq!(count_significant_figures("0.0"), Some(2));
        assert_eq!(count_significant_figures("5"), Some(1));
    }

    #[test]
    fn test_min_significant_figures_scan() {
        assert_eq!(min_significant_figures_in_expr("12.3 * 4.567"), Some(3));
        assert_eq!(min_significant_figures_in_expr("100.0 m + 2.5 m"), Some(2));
        assert_eq!(min_significant_figures_in_expr("sin(45 deg)"), Some(2));
    }
}
