use crate::{
    AbacusError, UnitRegistry, Value,
    evaluation::tokenizer::{lexer::percent_is_modulo, tokens::Token},
};

/// Characters that may continue a unit suffix glued to a number (`5km`, `20°C`, `3$`).
fn is_unit_suffix_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(c, '_' | '°' | 'Å' | 'Ω')
        || crate::evaluation::tokenizer::implicit::is_currency_symbol(c)
}

/// Parses a numeric literal at the start of `input` (which starts with a digit, or `.` and a
/// digit), folding an unspaced unit suffix (`5km`) into a `Token::Val`. Returns the token and
/// the number of bytes consumed. A suffix for which `is_variable` holds is left for the lexer
/// so that variables shadow units identically in `2h` and `2 h`.
pub(crate) fn parse_number_token<'a>(
    input: &'a str,
    unit_registry: &UnitRegistry,
    mut is_variable: impl FnMut(&'a str) -> bool,
    number_scales: bool,
) -> Result<(Token<'a>, usize), AbacusError> {
    let b = input.as_bytes();
    let at = |i: usize| b.get(i).copied();
    let mut end = 0;
    let mut has_dot = false;
    let mut has_exp = false;

    while let Some(c) = at(end) {
        if c.is_ascii_digit() {
            end += 1;
        } else if c == b'.' && !has_dot && !has_exp {
            match at(end + 1) {
                Some(n) if n.is_ascii_digit() => {
                    has_dot = true;
                    end += 1;
                }
                // `..` is the range operator.
                Some(b'.') => break,
                // A property access such as `.year` ends the number.
                Some(_)
                    if input[end + 1..].starts_with(|n: char| n.is_alphabetic() || n == '_') =>
                {
                    break;
                }
                _ => {
                    has_dot = true;
                    end += 1;
                }
            }
        } else if (c == b'e' || c == b'E') && !has_exp {
            let mut look = end + 1;
            if matches!(at(look), Some(b'+' | b'-')) {
                look += 1;
            }
            if at(look).is_some_and(|d| d.is_ascii_digit()) {
                has_exp = true;
                end = look;
                continue;
            }
            // A dangling exponent marker (`1e`, `2E+`) is a malformed literal, not `1 * e`.
            let after = input[look..].chars().next();
            if after.is_none_or(|c| !c.is_alphanumeric() && c != '_') {
                return Err(AbacusError::InvalidNumber(input[..look].to_string()));
            }
            break;
        } else {
            break;
        }
    }

    let num_str = &input[..end];
    let val = num_str
        .parse::<f64>()
        .map_err(|_| AbacusError::InvalidNumber(num_str.to_string()))?;
    if !val.is_finite() {
        return Err(AbacusError::InvalidNumber(num_str.to_string()));
    }

    // `%` directly after a number is the percent unit unless an operand follows (modulo).
    if at(end) == Some(b'%') {
        if percent_is_modulo(&input[end + 1..]) {
            return Ok((Token::Float(val), end));
        }
        if let Ok(unit) = unit_registry.unit("%") {
            return Ok((Token::Val(Value::new(val, unit)), end + 1));
        }
        return Ok((Token::Float(val), end));
    }

    // An unspaced unit (`5km`, `1s^-1`).
    let rest = &input[end..];
    if rest.starts_with(|c: char| {
        c.is_alphabetic()
            || matches!(c, '°' | 'Å' | 'Ω')
            || crate::evaluation::tokenizer::implicit::is_currency_symbol(c)
    }) {
        let mut unit_end = 0;
        let mut iter = rest.char_indices().peekable();
        while let Some(&(idx, c)) = iter.peek() {
            if is_unit_suffix_char(c) {
                unit_end = idx + c.len_utf8();
                iter.next();
            } else if c == '^' {
                unit_end = idx + 1;
                iter.next();
                if let Some(&(sign_idx, sign @ ('+' | '-'))) = iter.peek() {
                    unit_end = sign_idx + sign.len_utf8();
                    iter.next();
                }
                while let Some(&(digit_idx, d)) = iter.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        unit_end = digit_idx + 1;
                        iter.next();
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        let unit_candidate = &rest[..unit_end];
        if is_variable(unit_candidate) {
            return Ok((Token::Float(val), end));
        }
        if number_scales
            && let Some(scale) =
                crate::evaluation::tokenizer::implicit::number_scale_factor(unit_candidate)
        {
            return Ok((Token::Float(val * scale), end + unit_end));
        }
        if let Ok(unit) = unit_registry.unit(unit_candidate) {
            return Ok((Token::Val(Value::new(val, unit)), end + unit_end));
        }
    }

    Ok((Token::Float(val), end))
}
