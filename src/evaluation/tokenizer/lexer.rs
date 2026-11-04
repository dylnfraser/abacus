use crate::{
    AbacusError, UnitRegistry,
    evaluation::tokenizer::{
        date_literal::{DateLexOptions, try_parse_date_literal_with_anchor},
        implicit::resolve_tokens,
        number::parse_number_token,
        registry::token_registry::{MatchedOpKind, TokenRegistry, is_word_char},
        tokens::{Span, Token},
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
/// the next word is not `of` (`50% of 200`). A signed operand counts only after a space
/// (`5 % -3` is modulo; `10% - 3` and `10%-3` are a percentage minus 3).
#[inline]
pub(crate) fn percent_is_modulo(rest: &str) -> bool {
    let after = rest.trim_start();
    if match_phrase(after, "of") {
        return false;
    }
    let starts_operand = |s: &str| {
        let mut it = s.chars();
        match it.next() {
            Some(c) if c.is_ascii_digit() || c == '(' => true,
            Some('.') => it.next().is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        }
    };
    if starts_operand(after) {
        return true;
    }
    // A signed operand after a space is a negative/positive divisor (`5 % -3`), whereas
    // `10% - 3` and `10%-3` remain "percentage minus a number".
    rest.starts_with(char::is_whitespace)
        && after.strip_prefix(['-', '+']).is_some_and(starts_operand)
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
        cached: Option<&'r [MatchedOpKind]>,
    ) -> Self {
        let word_kinds =
            crate::evaluation::tokenizer::registry::token_registry::is_word_pattern(sym)
                .then(|| cached.unwrap_or_else(|| op_index.word_kinds(sym)));
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

/// Lexer settings (see [`lex`]).
#[derive(Clone, Copy)]
pub(crate) struct LexOptions<'o> {
    pub implicit_multiplication: bool,
    pub number_scales: bool,
    pub anchor_date: Option<&'o crate::Date>,
    pub lazy_now: Option<(
        &'o std::cell::OnceCell<crate::Date>,
        Option<&'o crate::units::date::TimeZone>,
    )>,
    pub date_month_first: bool,
}

/// Tokens with the byte span of the source each one came from (`spans[i]` is `tokens[i]`'s).
#[derive(Default)]
pub(crate) struct Lexed<'a> {
    pub tokens: Vec<Token<'a>>,
    pub spans: Vec<Span>,
}

/// Tokenizes `input_text`, recording the span of every token. Errors carry the span of the
/// input being lexed when they occurred.
///
/// When `variable_lookups` is given, every name whose reading depended on whether it is a
/// variable is appended to it, with the answer, so a caller can tell later whether the same
/// input would still be read the same way.
///
/// The tokens are written into `buffers`, which should be empty (reusing their allocations).
pub(crate) fn lex<'a>(
    buffers: Lexed<'a>,
    token_registry: &TokenRegistry,
    unit_registry: &UnitRegistry,
    variables: Option<&std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    input_text: &'a str,
    options: LexOptions<'_>,
    variable_lookups: Option<&mut Vec<(&'a str, bool)>>,
) -> Result<Lexed<'a>, (AbacusError, Span)> {
    // Every token spans at least one byte, so the input length bounds the token count; capping
    // the initial reservation keeps huge inputs from over-allocating up front.
    let capacity = input_text.len().min(256);
    let Lexed {
        mut tokens,
        mut spans,
    } = buffers;
    debug_assert!(tokens.is_empty() && spans.is_empty());
    tokens.reserve(capacity);
    spans.reserve(capacity);
    let mut lexer = Lexer {
        input: input_text,
        pos: 0,
        tokens,
        spans,
        in_conversion_target: false,
        token_registry,
        unit_registry,
        op_index: token_registry.operator_index(),
        variables,
        options,
        word_lookup: None,
        record_lookups: variable_lookups.is_some(),
        lookups: Vec::new(),
    };
    while let Some(c) = lexer.peek_char() {
        if c.is_whitespace() {
            lexer.pos += c.len_utf8();
            continue;
        }
        let start = lexer.pos;
        match lexer.next_token(c) {
            Ok((token, len)) => {
                lexer.pos = start + len;
                lexer.tokens.push(token);
                lexer.spans.push(Span::new(start, lexer.pos));
            }
            Err(error) => {
                // Point at the word (or character) being lexed.
                let rest = &input_text[start..];
                let word = rest
                    .find(|ch: char| !is_word_char(ch))
                    .unwrap_or(rest.len())
                    .max(c.len_utf8());
                return Err((error, Span::new(start, start + word)));
            }
        }
    }

    let Lexer {
        tokens,
        spans,
        lookups,
        ..
    } = lexer;
    if let Some(out) = variable_lookups {
        out.extend(lookups);
    }
    resolve_tokens(
        Lexed { tokens, spans },
        unit_registry,
        token_registry,
        options.implicit_multiplication,
        options.number_scales,
    )
}

/// Lexer state: a byte cursor into the input and the tokens produced so far.
struct Lexer<'a, 'r> {
    input: &'a str,
    pos: usize,
    tokens: Vec<Token<'a>>,
    spans: Vec<Span>,
    /// True while lexing the target of an `in`/`to`/`as` conversion, where unit names take
    /// precedence over variables of the same name.
    in_conversion_target: bool,
    token_registry: &'r TokenRegistry,
    unit_registry: &'r UnitRegistry,
    op_index: &'r crate::evaluation::tokenizer::registry::token_registry::OperatorIndex,
    variables: Option<&'r std::collections::HashMap<String, crate::units::eval_result::EvalResult>>,
    options: LexOptions<'r>,
    /// The operator kinds of the plain word at byte `.0` (of length `.1`), as looked up by
    /// [`Lexer::registered_operator`], so [`Lexer::classify_word`] needn't hash it again.
    word_lookup: Option<(usize, usize, &'r [MatchedOpKind])>,
    /// Whether to record [`Lexer::lookups`].
    record_lookups: bool,
    /// Names looked up as variables, and whether they were (see [`lex`]).
    lookups: Vec<(&'a str, bool)>,
}

/// A lexing rule's result: the token and how many bytes of input it consumed.
type Lexeme<'a> = (Token<'a>, usize);

impl<'a> Lexer<'a, '_> {
    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn last(&self) -> Option<&Token<'a>> {
        self.tokens.last()
    }

    /// Whether `name` is a variable, which then takes precedence over a unit of that name.
    fn is_variable(&mut self, name: &'a str) -> bool {
        let found = self.variables.is_some_and(|v| v.contains_key(name));
        if self.record_lookups {
            self.lookups.push((name, found));
        }
        found
    }

    /// Lexes one token starting with `c` at the cursor. Rules are tried in this order, and the
    /// first that matches wins; the order is what resolves overlaps (a date literal before a
    /// number, a phrase like `% change from` before `%`, an operator word before a unit):
    ///
    /// 1. date and time literals            7. relative time (`ago`, `from now`, `until`)
    /// 2. timezone offsets after a date     8. multi-word units (`fl oz`, `business days`)
    /// 3. `% change from` and friends       9. `..` and `.property`
    /// 4. percentage tags (`18% tip`)      10. registered operators and functions
    /// 5. `off` (`20% off $120`)           11. numbers, currency symbols, `%`, words
    /// 6. `more than`, `less than`, `out of`
    ///
    /// Punctuation (`( ) [ ] ,`) is checked first: none of these rules can match it.
    fn next_token(&mut self, c: char) -> Result<Lexeme<'a>, AbacusError> {
        // Punctuation can't start any earlier rule's match, so skip straight to it.
        if let Some(lexeme) = self.punctuation(c)? {
            return Ok(lexeme);
        }
        macro_rules! try_rule {
            ($rule:ident) => {
                if let Some(lexeme) = self.$rule(c)? {
                    return Ok(lexeme);
                }
            };
        }
        try_rule!(date_literal);
        try_rule!(timezone_offset);
        try_rule!(percent_change);
        try_rule!(percent_tag);
        try_rule!(off);
        try_rule!(conversational_operator);
        try_rule!(relative_time);
        try_rule!(multi_word_unit);
        try_rule!(dot);
        try_rule!(registered_operator);
        if c.is_ascii_digit()
            || (c == '.' && self.rest()[1..].starts_with(|d: char| d.is_ascii_digit()))
        {
            return self.number();
        }
        if crate::evaluation::tokenizer::implicit::is_currency_symbol(c) {
            let sym = &self.rest()[..c.len_utf8()];
            return if self.unit_registry.contains(sym) {
                Ok((Token::Unit(sym), sym.len()))
            } else {
                Err(AbacusError::UnknownUnit(sym.to_string()))
            };
        }
        // A bare `%` (percent unit) is always a single-character symbol so `20%of 50` works.
        if c == '%' {
            let sym = &self.rest()[..1];
            return if self.unit_registry.contains(sym) {
                Ok((Token::Unit(sym), 1))
            } else if let Some(op) = self.token_registry.binary_operators.get(sym) {
                Ok((Token::BinaryOp(op.alias), 1))
            } else {
                Err(AbacusError::UnknownUnit(sym.to_string()))
            };
        }
        if c.is_alphabetic() || matches!(c, '_' | '°' | 'Å' | 'Ω') {
            return self.word();
        }
        Err(AbacusError::UnexpectedCharacter(c))
    }

    /// `today`, `tomorrow`, `@2026-08-07@`, `07-08-2026`, `12:00`, `1:00 PM`, `3 PM`, `16 May`, ...
    #[inline]
    fn date_literal(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        let rest = self.rest();
        let candidate = c == '@'
            || (c.is_ascii_digit() && is_numeric_date_candidate(rest))
            || c.is_ascii_alphabetic();
        if !candidate {
            return Ok(None);
        }
        let opts = DateLexOptions {
            anchor: self.options.anchor_date,
            lazy_now: self.options.lazy_now,
            month_first: self.options.date_month_first,
            after_quantity: matches!(self.last(), Some(Token::Float(_) | Token::Val(_))),
        };
        Ok(try_parse_date_literal_with_anchor(rest, opts)?
            .map(|(date, len)| (Token::Date(date), len)))
    }

    /// `+02:00`, `-04:00`, `+05:30`, only directly after a date/time or as a conversion
    /// target, so `10 -1:30` stays arithmetic.
    #[inline]
    fn timezone_offset(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if !matches!(c, '+' | '-')
            || !matches!(self.last(), Some(Token::Date(_) | Token::ConversionOp))
        {
            return Ok(None);
        }
        let rest = self.rest();
        Ok(match_tz_offset_literal(rest).map(|len| (Token::Unit(&rest[..len]), len)))
    }

    /// `% change from`, `percent change from`, ...
    #[inline]
    fn percent_change(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if !matches!(c, '%' | 'p' | 'P') {
            return Ok(None);
        }
        Ok(match_percent_change_from(self.rest()).map(|len| (Token::PercentChangeFrom, len)))
    }

    /// `18% tip`, `15% tax`, `20% discount`, `10% fee` (only right after a percentage).
    #[inline]
    fn percent_tag(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if !c.is_ascii_alphabetic() {
            return Ok(None);
        }
        let after_percent = match self.last() {
            Some(Token::Unit(u)) => matches!(*u, "%" | "percent" | "pct"),
            Some(Token::Val(v)) => v.unit.is_percent(),
            _ => false,
        };
        if !after_percent {
            return Ok(None);
        }
        Ok(match_percent_tag(self.rest()).map(|(tag, len)| (Token::PercentTag(tag), len)))
    }

    /// `off`: an operator when an expression follows (`20% off $120`), else a percentage tag
    /// (`$100 - 20% off`).
    #[inline]
    fn off(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        let rest = self.rest();
        if !matches!(c, 'o' | 'O') || !starts_with_ci(rest, "off") || !at_word_boundary(&rest[3..])
        {
            return Ok(None);
        }
        let after = rest[3..].trim_start();
        let starts_expr = !after.is_empty() && !after.starts_with([')', ']', ',']);
        let token = if starts_expr {
            Token::BinaryOp("off")
        } else {
            Token::PercentTag("off")
        };
        Ok(Some((token, 3)))
    }

    /// `more than`, `less than`, `out of`.
    #[inline]
    fn conversational_operator(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if !matches!(c, 'm' | 'M' | 'l' | 'L' | 'o' | 'O') {
            return Ok(None);
        }
        Ok(match_conversational_binary_op(self.rest())
            .map(|(alias, len)| (Token::BinaryOp(alias), len)))
    }

    /// `ago`, `from now`, `before`, `after`, `until`.
    #[inline]
    fn relative_time(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if !c.is_ascii_alphabetic() {
            return Ok(None);
        }
        Ok(match_rel_time_op(self.rest()).map(|(op, len)| (Token::RelTimeOp(op), len)))
    }

    /// Unit names containing a space or symbol (`business days`, `fl oz`, `long ton`, `R$`),
    /// listed in `registry::units::MULTI_WORD_UNITS`.
    #[inline]
    fn multi_word_unit(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        let rest = self.rest();
        if !c.is_ascii_alphabetic() || !could_start_multi_word_unit(rest) {
            return Ok(None);
        }
        for &unit_name in crate::registry::units::MULTI_WORD_UNITS {
            let name = unit_name.as_bytes();
            if rest.len() >= name.len()
                && name[0].eq_ignore_ascii_case(&(c as u8))
                && rest.as_bytes()[..name.len()].eq_ignore_ascii_case(name)
            {
                // Names ending in a symbol (e.g. "R$") may be glued to a number ("R$5").
                let needs_boundary = name[name.len() - 1].is_ascii_alphanumeric();
                if (!needs_boundary || at_word_boundary(&rest[name.len()..]))
                    && self.unit_registry.contains(unit_name)
                {
                    return Ok(Some((Token::Unit(unit_name), unit_name.len())));
                }
            }
        }
        Ok(None)
    }

    /// `(`, `)`, `[`, `]`, `,`. Closing a group or starting the next argument ends a
    /// conversion target.
    #[inline]
    fn punctuation(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        let token = match c {
            '(' => Token::OpenParen,
            ')' => Token::CloseParen,
            '[' => Token::OpenBracket,
            ']' => Token::CloseBracket,
            ',' => Token::Comma,
            _ => return Ok(None),
        };
        if matches!(c, ')' | ']' | ',') {
            self.in_conversion_target = false;
        }
        Ok(Some((token, 1)))
    }

    /// `..` (range) and `.property` (`.slope`, `.year`).
    #[inline]
    fn dot(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        if c != '.' {
            return Ok(None);
        }
        let after = &self.rest()[1..];
        if after.starts_with('.') {
            return Ok(Some((Token::Range, 2)));
        }
        if after.starts_with(|n: char| n.is_alphabetic() || n == '_') {
            let len = after
                .find(|n: char| !n.is_alphanumeric() && n != '_')
                .unwrap_or(after.len());
            return Ok(Some((Token::DotProperty(&after[..len]), 1 + len)));
        }
        Ok(None)
    }

    /// Registered operators and functions, longest pattern first (`++` over `+`). Plain-word
    /// patterns can only match the whole leading word, so they are found with one map lookup;
    /// symbolic patterns are scanned, but only those longer than the leading word can match.
    fn registered_operator(&mut self, c: char) -> Result<Option<Lexeme<'a>>, AbacusError> {
        let remaining = self.rest();
        let word_len = if is_word_char(c) {
            remaining
                .find(|ch: char| !is_word_char(ch))
                .unwrap_or(remaining.len())
        } else {
            0
        };
        let prev = self.last();
        let mut best: Option<(&str, MatchedOpKind)> = None;
        for (pattern, kind) in self.op_index.symbolic_candidates(c) {
            if pattern.len() <= word_len {
                // The first `word_len` bytes are all word characters, which a symbolic
                // pattern of that length or shorter cannot match.
                break;
            }
            if let Some(rest) = strip_prefix_inline(remaining, pattern)
                && accept_operator_candidate(pattern, *kind, rest, prev, self.unit_registry)
            {
                best = Some((pattern, *kind));
                break;
            }
        }
        if best.is_none() && word_len > 0 {
            let (word, rest) = remaining.split_at(word_len);
            let kinds = self.op_index.word_kinds(word);
            best = kinds
                .iter()
                .find(|kind| {
                    accept_operator_candidate(word, **kind, rest, prev, self.unit_registry)
                })
                .map(|kind| (word, *kind));
            self.word_lookup = Some((self.pos, word_len, kinds));
        }
        Ok(best.map(|(pattern, kind)| {
            let token = match kind {
                MatchedOpKind::Binary(alias) => Token::BinaryOp(alias),
                MatchedOpKind::Unary(alias) => Token::UnaryOp(alias),
                MatchedOpKind::Func(name) => Token::Function(name),
            };
            (token, pattern.len())
        }))
    }

    /// A number, with an unspaced unit or scale word folded in (`5km`, `3million`).
    fn number(&mut self) -> Result<Lexeme<'a>, AbacusError> {
        let (rest, units, scales) = (self.rest(), self.unit_registry, self.options.number_scales);
        // Conversion targets are units even if a variable shadows the name.
        let shadowing = !self.in_conversion_target;
        parse_number_token(
            rest,
            units,
            |name| shadowing && self.is_variable(name),
            scales,
        )
    }

    /// The length of the identifier at the cursor, including a unit exponent (`m^2`, `s^-1`)
    /// when the base is a unit rather than a constant or variable.
    fn word_len(&mut self) -> usize {
        let rest = self.rest();
        let vars_shadow = !self.in_conversion_target;
        let mut end = 0;
        let mut iter = rest.char_indices().peekable();
        while let Some(&(idx, ch)) = iter.peek() {
            if ch.is_alphanumeric() || matches!(ch, '_' | '°' | 'Å' | 'Ω') {
                end = idx + ch.len_utf8();
                iter.next();
            } else if ch == '^' {
                let base = &rest[..end];
                if is_standard_constant(base)
                    || (vars_shadow && self.is_variable(base))
                    || !self.unit_registry.contains(base)
                {
                    break;
                }
                end = idx + 1;
                iter.next();
                if let Some(&(sign_idx, sign @ ('+' | '-'))) = iter.peek() {
                    end = sign_idx + sign.len_utf8();
                    iter.next();
                }
                while let Some(&(digit_idx, d)) = iter.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        end = digit_idx + 1;
                        iter.next();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        end
    }

    /// An identifier: a conversion keyword, function, operator word, constant, variable or unit.
    fn word(&mut self) -> Result<Lexeme<'a>, AbacusError> {
        let len = self.word_len();
        let rest = self.rest();
        let sym = &rest[..len];
        let after = &rest[len..];
        let token = self.classify_word(sym, after)?;
        Ok((token, len))
    }

    fn classify_word(&mut self, sym: &'a str, after: &'a str) -> Result<Token<'a>, AbacusError> {
        if CONVERSION_KEYWORDS.contains(&sym) {
            self.in_conversion_target = true;
            return Ok(Token::ConversionOp);
        }
        if sym == "in" {
            return Ok(if self.in_is_unit(after) {
                Token::Unit(sym)
            } else {
                self.in_conversion_target = true;
                Token::ConversionOp
            });
        }
        let cached = self
            .word_lookup
            .filter(|&(pos, len, _)| pos == self.pos && len == sym.len())
            .map(|(_, _, kinds)| kinds);
        let registered = RegisteredOps::lookup(self.op_index, self.token_registry, sym, cached);
        if let Some(name) = registered.find(OpMap::Func) {
            let followed_by_call = after.trim_start().starts_with('(');
            return Ok(if followed_by_call || !self.unit_registry.contains(sym) {
                Token::Function(name)
            } else {
                Token::Unit(sym)
            });
        }
        if sym.eq_ignore_ascii_case("per") {
            return Ok(Token::BinaryOp("/"));
        }
        if let Some(alias) = registered.find(OpMap::Binary) {
            return Ok(Token::BinaryOp(alias));
        }
        if let Some(alias) = registered.find(OpMap::Unary) {
            return Ok(Token::UnaryOp(alias));
        }
        if self.in_conversion_target && self.unit_registry.contains(sym) {
            // Conversion targets are units even if a variable shadows the name.
            return Ok(Token::Unit(sym));
        }
        if is_standard_constant(sym) || self.is_variable(sym) {
            return Ok(Token::Ident(sym));
        }
        if let Some(tok) = resolve_indefinite_article(sym, after, self.last(), self.unit_registry) {
            return Ok(tok);
        }
        if self.unit_registry.contains(sym) {
            return Ok(Token::Unit(sym));
        }
        if sym.contains('^') {
            self.unit_registry.unit(sym)?;
            return Ok(Token::Unit(sym));
        }
        if after.trim_start().starts_with('(') {
            return Err(AbacusError::UnknownFunction(sym.to_string()));
        }
        Err(AbacusError::UnknownUnit(sym.to_string()))
    }

    /// Whether `in` (followed by `after`) is the inch rather than the conversion keyword.
    fn in_is_unit(&self, after: &str) -> bool {
        let rest = after.trim_start();
        let has_no_target = rest.is_empty()
            || rest.starts_with([
                ')', ']', ',', ';', '+', '-', '*', '/', '^', '=', '<', '>', '!',
            ]);
        let next_word = leading_word(rest);
        let followed_by_conversion_kw = matches!(next_word, "to" | "as" | "in");
        match self.last() {
            Some(Token::ConversionOp) => true,
            Some(Token::Float(_)) => followed_by_conversion_kw || has_no_target,
            Some(
                Token::BinaryOp(_)
                | Token::UnaryOp(_)
                | Token::OpenParen
                | Token::OpenBracket
                | Token::Comma,
            )
            | None => followed_by_conversion_kw || has_no_target,
            // After a unit/value (e.g. `5 in in`, `5 m in`), `in` is the conversion keyword; a
            // missing target is then reported instead of silently multiplying by inches.
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::tokenizer::sig_figs::{
        count_significant_figures, min_significant_figures_in_expr,
    };

    /// Tokenizes with default settings.
    fn tokenize_string<'a>(
        token_registry: &TokenRegistry,
        unit_registry: &UnitRegistry,
        input: &'a str,
    ) -> Result<Vec<Token<'a>>, AbacusError> {
        let options = LexOptions {
            implicit_multiplication: true,
            number_scales: cfg!(feature = "number-scales"),
            anchor_date: None,
            lazy_now: None,
            date_month_first: false,
        };
        lex(
            Lexed::default(),
            token_registry,
            unit_registry,
            None,
            input,
            options,
            None,
        )
        .map(|lexed| lexed.tokens)
        .map_err(|(error, _)| error)
    }

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
