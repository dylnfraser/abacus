use crate::{AbacusError, UnitRegistry, Value, evaluation::tokenizer::tokens::Token};
use std::iter::Peekable;
use std::str::CharIndices;

pub(crate) fn parse_number_token<'a>(
    input_text: &'a str,
    start: usize,
    chars: &mut Peekable<CharIndices<'a>>,
    unit_registry: &UnitRegistry,
    number_scales: bool,
) -> Result<Token<'a>, AbacusError> {
    let mut has_dot = false;
    let mut has_exp = false;

    while let Some(&(_, num_c)) = chars.peek() {
        if num_c.is_ascii_digit() {
            chars.next();
        } else if num_c == '.' && !has_dot && !has_exp {
            let mut dot_lookahead = chars.clone();
            dot_lookahead.next(); // skip past the '.'
            match dot_lookahead.peek() {
                Some(&(_, next_c)) if next_c.is_ascii_digit() => {
                    has_dot = true;
                    chars.next();
                }
                Some(&(_, '.')) => {
                    break; // range operator `..`
                }
                Some(&(_, next_c)) if next_c.is_alphabetic() || next_c == '_' => {
                    break; // stop number here for property access e.g. .year
                }
                _ => {
                    has_dot = true;
                    chars.next();
                }
            }
        } else if (num_c == 'e' || num_c == 'E') && !has_exp {
            let mut exp_lookahead = chars.clone();
            exp_lookahead.next(); // skip 'e' or 'E'
            if let Some(&(_, sign_c)) = exp_lookahead.peek()
                && (sign_c == '+' || sign_c == '-')
            {
                exp_lookahead.next();
            }
            if let Some(&(_, digit_c)) = exp_lookahead.peek()
                && digit_c.is_ascii_digit()
            {
                has_exp = true;
                chars.next(); // consume 'e'/'E'
                if let Some(&(_, sign_c)) = chars.peek()
                    && (sign_c == '+' || sign_c == '-')
                {
                    chars.next(); // consume '+' or '-'
                }
                continue;
            }
            break;
        } else {
            break;
        }
    }

    let num_str = &input_text[start..chars.peek().map_or(input_text.len(), |&(idx, _)| idx)];
    let val = num_str
        .parse::<f64>()
        .map_err(|_| AbacusError::InvalidNumber(num_str.to_string()))?;

    // Check if immediately followed by an unspaced unit identifier (e.g. 5km, 10m, 1s^-1)
    if let Some(&(unit_start, unit_c)) = chars.peek()
        && (unit_c.is_alphabetic()
            || unit_c == '°'
            || unit_c == 'Å'
            || unit_c == 'Ω'
            || unit_c == '%'
            || crate::evaluation::tokenizer::implicit::is_currency_symbol(unit_c))
    {
        let mut unit_end = unit_start;
        let mut unit_chars = chars.clone();
        while let Some((idx, sym_c)) = unit_chars.peek().copied() {
            if sym_c.is_alphanumeric()
                || sym_c == '_'
                || sym_c == '°'
                || sym_c == 'Å'
                || sym_c == 'Ω'
                || sym_c == '%'
                || crate::evaluation::tokenizer::implicit::is_currency_symbol(sym_c)
            {
                unit_end = idx + sym_c.len_utf8();
                unit_chars.next();
            } else if sym_c == '^' {
                unit_end = idx + sym_c.len_utf8();
                unit_chars.next();
                if let Some((sign_idx, sign_c)) = unit_chars.peek().copied()
                    && (sign_c == '+' || sign_c == '-')
                {
                    unit_end = sign_idx + sign_c.len_utf8();
                    unit_chars.next();
                }
                while let Some((digit_idx, digit_c)) = unit_chars.peek().copied() {
                    if digit_c.is_ascii_digit() || digit_c == '.' {
                        unit_end = digit_idx + digit_c.len_utf8();
                        unit_chars.next();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        let unit_candidate = &input_text[unit_start..unit_end];
        if number_scales
            && let Some(scale) =
                crate::evaluation::tokenizer::implicit::number_scale_factor(unit_candidate)
        {
            *chars = unit_chars;
            return Ok(Token::Float(val * scale));
        }
        if let Ok(unit) = unit_registry.unit(unit_candidate) {
            *chars = unit_chars;
            return Ok(Token::Val(Value::new(val, unit)));
        }
    }

    Ok(Token::Float(val))
}
