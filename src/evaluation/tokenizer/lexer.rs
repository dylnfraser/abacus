use crate::{
    AbacusError, UnitRegistry,
    evaluation::tokenizer::{
        date_literal::{DateLexOptions, try_parse_date_literal_with_anchor},
        implicit::resolve_tokens,
        number::parse_number_token,
        registry::token_registry::{MatchedOpKind, TokenRegistry, is_word_char},
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
    // Accept `±HH:MM` or the compact `±HHMM` form.
    let compact = len == 5 && !word.contains(':');
    if !(word.contains(':') || compact)
        || s[len..].starts_with(|c: char| c.is_alphanumeric() || c == '.')
    {
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
    let next_is_scale =
        crate::evaluation::tokenizer::implicit::number_scale_factor(next_word).is_some();
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
    tokenize_string_with_date_options(
        token_registry,
        unit_registry,
        variables,
        input_text,
        implicit_multiplication,
        number_scales,
        anchor_date,
        false,
    )
}

/// `str::strip_prefix` for the short operator patterns: compares bytes inline and stops at the
/// first mismatch (usually the second byte), avoiding an out-of-line `memcmp` per candidate.
#[inline]
fn strip_prefix_inline<'s>(s: &'s str, pattern: &str) -> Option<&'s str> {
    let (sb, pb) = (s.as_bytes(), pattern.as_bytes());
    if sb.len() < pb.len() || !sb.iter().zip(pb).all(|(a, b)| a == b) {
        return None;
    }
    // `pattern` is a whole `str` matching a prefix of `s`, so this is a char boundary.
    Some(&s[pb.len()..])
}

#[derive(Clone, Copy)]
enum OpMap {
    Func,
    Binary,
    Unary,
}

/// Which of the registry's operator maps an identifier is registered in, looked up once.
/// Plain words are answered from the operator index's word table (the same keys, hashed with
/// the fast hasher); anything else falls back to the public maps.
struct RegisteredOps<'r> {
    sym: &'r str,
    token_registry: &'r TokenRegistry,
    /// `Some` when `sym` is a plain word (then this is its complete list of kinds).
    word_kinds: Option<&'r [MatchedOpKind]>,
}

impl<'r> RegisteredOps<'r> {
    fn lookup(
        op_index: &'r crate::evaluation::tokenizer::registry::token_registry::OperatorIndex,
        token_registry: &'r TokenRegistry,
        sym: &'r str,
    ) -> Self {
        let word_kinds =
            crate::evaluation::tokenizer::registry::token_registry::is_word_pattern(sym)
                .then(|| op_index.word_kinds(sym));
        Self {
            sym,
            token_registry,
            word_kinds,
        }
    }

    /// The emitted name/alias if `sym` is registered in `map`.
    fn find(&self, map: OpMap) -> Option<&'static str> {
        if let Some(kinds) = self.word_kinds {
            return kinds.iter().find_map(|kind| match (map, *kind) {
                (OpMap::Func, MatchedOpKind::Func(name))
                | (OpMap::Binary, MatchedOpKind::Binary(name))
                | (OpMap::Unary, MatchedOpKind::Unary(name)) => Some(name),
                _ => None,
            });
        }
        let reg = self.token_registry;
        match map {
            OpMap::Func => reg.function_operators.get(self.sym).map(|op| op.name),
            OpMap::Binary => reg.binary_operators.get(self.sym).map(|op| op.alias),
            OpMap::Unary => reg.unary_operators.get(self.sym).map(|op| op.alias),
        }
    }
}

/// Applies the context rules for an operator `pattern` found at the start of the input, with
/// `rest` the text after it and `prev` the last emitted token. Returns `false` when this
/// candidate must be skipped in favour of a shorter one (or none).
fn accept_operator_candidate(
    pattern: &str,
    kind: MatchedOpKind,
    rest: &str,
    prev: Option<&Token<'_>>,
    unit_registry: &UnitRegistry,
) -> bool {
    let next_char = rest.chars().next();
    let last_char = pattern.chars().last();
    match kind {
        MatchedOpKind::Binary(_) => {
            if pattern == "%" && !percent_is_modulo(rest) {
                return false;
            }
            !(last_char.is_some_and(|lc| lc.is_alphanumeric() || lc == '_')
                && next_char.is_some_and(is_word_char))
        }
        MatchedOpKind::Unary(op_alias) => {
            // `++`/`--` directly after an operand are only postfix increment /
            // decrement at the end of a (sub)expression; otherwise `3--2` and
            // `2++3` are a binary operator followed by a unary sign.
            if matches!(op_alias, "++" | "--")
                && is_operand_token(prev)
                && !rest
                    .trim_start()
                    .chars()
                    .next()
                    .is_none_or(|nc| matches!(nc, ')' | ']' | ','))
            {
                return false;
            }
            !(last_char.is_some_and(|lc| lc.is_alphanumeric() || lc == '_')
                && next_char.is_some_and(is_word_char))
        }
        MatchedOpKind::Func(_) => {
            if unit_registry.contains(pattern) && !rest.trim_start().starts_with('(') {
                return false;
            }
            !(last_char.is_some_and(|lc| lc.is_alphanumeric() || lc == '_' || lc == '-')
                && next_char.is_some_and(|nc| is_word_char(nc) || nc == '-'))
        }
    }
}

/// Cheap pre-filter for [`crate::registry::units::MULTI_WORD_UNITS`]: every entry is an
/// ASCII-alphabetic first word followed by a space (or `$`, as in `R$`), so a match is only
/// possible when the leading letters of `s` are followed by one of those bytes.
#[inline]
fn could_start_multi_word_unit(s: &str) -> bool {
    let b = s.as_bytes();
    let word_len = b.iter().take_while(|c| c.is_ascii_alphabetic()).count();
    matches!(b.get(word_len), Some(b' ' | b'$'))
}

/// Returns `true` if the digit run at the start of `s` may begin a date or time literal
/// (`07-08-2026`, `15:30`, `3pm`, `3 PM`, `16th May`, `16 May 2010`).
fn is_numeric_date_candidate(s: &str) -> bool {
    let bytes = s.as_bytes();
    for (idx, &b) in bytes.iter().enumerate().take(20) {
        if b == b'-' || b == b'/' || b == b':' || b.is_ascii_alphabetic() {
            return true;
        }
        if b == b' ' || b == b'\t' {
            let next = s[idx..].trim_start();
            return crate::evaluation::tokenizer::date_literal::strip_word_prefix(next, "am")
                .or_else(|| {
                    crate::evaluation::tokenizer::date_literal::strip_word_prefix(next, "pm")
                })
                .is_some()
                || crate::evaluation::tokenizer::date_literal::parse_month(next).is_some()
                || crate::evaluation::tokenizer::date_literal::strip_word_prefix(next, "of")
                    .is_some();
        }
        if matches!(b, b',' | b')' | b']' | b'+' | b'*') {
            return false;
        }
    }
    false
}

/// Tokenize an expression string with an optional anchor date, reading ambiguous `NN-NN-YYYY`
/// dates month-first when `date_month_first` is set (day-first otherwise).
#[allow(clippy::too_many_arguments)]
pub fn tokenize_string_with_date_options<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    input_text: &'a str,
    implicit_multiplication: bool,
    number_scales: bool,
    anchor_date: Option<&crate::Date>,
    date_month_first: bool,
) -> Result<Vec<Token<'a>>, AbacusError> {
    tokenize_with_lazy_now(
        token_registry,
        unit_registry,
        variables,
        input_text,
        implicit_multiplication,
        number_scales,
        anchor_date,
        None,
        date_month_first,
    )
}

/// [`tokenize_string_with_date_options`], with an optional lazily resolved "now" used when no
/// anchor is given (see [`DateLexOptions::lazy_now`]).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tokenize_with_lazy_now<'a>(
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    input_text: &'a str,
    implicit_multiplication: bool,
    number_scales: bool,
    anchor_date: Option<&crate::Date>,
    lazy_now: Option<(
        &std::cell::OnceCell<crate::Date>,
        Option<&crate::units::date::TimeZone>,
    )>,
    date_month_first: bool,
) -> Result<Vec<Token<'a>>, AbacusError> {
    // Every token spans at least one byte, so the input length bounds the token count; capping
    // the initial reservation keeps huge inputs from over-allocating up front.
    let mut tokens = Vec::with_capacity(input_text.len().min(256));
    let mut chars = input_text.char_indices().peekable();
    let op_index = token_registry.operator_index_ref();
    // True while lexing the target of an `in`/`to`/`as` conversion, where unit names take
    // precedence over variables of the same name.
    let mut in_conversion_target = false;

    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        // Date literals (e.g. today, tomorrow, yesterday, now, @2026-08-07@, 07-08-2026, 12:00, 1:00 PM, 3 PM, 16 May)
        let is_date_candidate = c == '@'
            || (c.is_ascii_digit() && is_numeric_date_candidate(&input_text[i..]))
            || c.is_ascii_alphabetic();

        if is_date_candidate
            && let Some((date, consumed_bytes)) = try_parse_date_literal_with_anchor(
                &input_text[i..],
                DateLexOptions {
                    anchor: anchor_date,
                    lazy_now,
                    month_first: date_month_first,
                    after_quantity: matches!(tokens.last(), Some(Token::Float(_) | Token::Val(_))),
                },
            )?
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

        // Multi-word unit symbols (e.g. "business days", "fl oz", "long ton", "R$"),
        // listed in `registry::units::MULTI_WORD_UNITS`.
        if c.is_ascii_alphabetic() && could_start_multi_word_unit(&input_text[i..]) {
            let remaining = &input_text[i..];
            let mut matched: Option<&'static str> = None;
            for &unit_name in crate::registry::units::MULTI_WORD_UNITS {
                let name = unit_name.as_bytes();
                if remaining.len() >= name.len()
                    && name[0].eq_ignore_ascii_case(&(c as u8))
                    && remaining.as_bytes()[..name.len()].eq_ignore_ascii_case(name)
                {
                    // Names ending in a symbol (e.g. "R$") may be glued to a number ("R$5").
                    let needs_boundary = name[name.len() - 1].is_ascii_alphanumeric();
                    let next_char = remaining[name.len()..].chars().next();
                    if (!needs_boundary
                        || next_char.is_none_or(|nc| !nc.is_alphanumeric() && nc != '_'))
                        && unit_registry.contains(unit_name)
                    {
                        matched = Some(unit_name);
                        break;
                    }
                }
            }
            if let Some(unit_name) = matched {
                let matched_len = unit_name.len();
                tokens.push(Token::Unit(unit_name));
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

        // Registered operator checks (longest pattern first, e.g. `++` over `+`). Plain-word
        // patterns can only match the whole leading word, so they are found with one map lookup;
        // symbolic patterns are scanned, but only those longer than the leading word can match.
        {
            let remaining = &input_text[i..];
            let word_len = if is_word_char(c) {
                remaining
                    .find(|ch: char| !is_word_char(ch))
                    .unwrap_or(remaining.len())
            } else {
                0
            };
            let mut best_match: Option<(&str, MatchedOpKind)> = None;

            for (pattern, kind) in op_index.symbolic_candidates(c) {
                if pattern.len() <= word_len {
                    // The first `word_len` bytes are all word characters, which a symbolic
                    // pattern of that length or shorter cannot match.
                    break;
                }
                if let Some(rest) = strip_prefix_inline(remaining, pattern)
                    && accept_operator_candidate(pattern, *kind, rest, tokens.last(), unit_registry)
                {
                    best_match = Some((pattern, *kind));
                    break;
                }
            }

            if best_match.is_none() && word_len > 0 {
                let (word, rest) = remaining.split_at(word_len);
                best_match = op_index
                    .word_kinds(word)
                    .iter()
                    .find(|kind| {
                        accept_operator_candidate(word, **kind, rest, tokens.last(), unit_registry)
                    })
                    .map(|kind| (word, *kind));
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
            let registered = RegisteredOps::lookup(&op_index, token_registry, sym);

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
            } else if let Some(name) = registered.find(OpMap::Func) {
                let rest = input_text[end..].trim_start();
                if rest.starts_with('(') || !unit_registry.contains(sym) {
                    tokens.push(Token::Function(name));
                } else {
                    tokens.push(Token::Unit(sym));
                }
            } else if sym.eq_ignore_ascii_case("per") {
                tokens.push(Token::BinaryOp("/"));
            } else if let Some(alias) = registered.find(OpMap::Binary) {
                tokens.push(Token::BinaryOp(alias));
            } else if let Some(alias) = registered.find(OpMap::Unary) {
                tokens.push(Token::UnaryOp(alias));
            } else if in_conversion_target && unit_registry.contains(sym) {
                // Conversion targets are units even if a variable shadows the name.
                tokens.push(Token::Unit(sym));
            } else if is_standard_constant(sym) || variables.is_some_and(|v| v.contains_key(sym)) {
                tokens.push(Token::Ident(sym));
            } else if let Some(tok) =
                resolve_indefinite_article(sym, &input_text[end..], tokens.last(), unit_registry)
            {
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

    #[cfg(feature = "units")]
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

    #[cfg(feature = "units")]
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

    #[cfg(feature = "stats")]
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

    #[cfg(all(feature = "units", feature = "stats"))]
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
