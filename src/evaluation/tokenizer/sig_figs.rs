//! Automatic significant figures counting and detection according to standard scientific precision rules.
//!
//! # The 5 Rules of Significant Figures:
//! - **Rule 1**: Non-zero digits are always significant (`1..=9`, e.g. `45.2` has 3 sig figs).
//! - **Rule 2**: Captive zeros between non-zero digits are significant (e.g. `5005` has 4 sig figs).
//! - **Rule 3**: Leading zeros are not significant; they act only as position placeholders (e.g. `0.0045` has 2 sig figs).
//! - **Rule 4**: Trailing zeros are significant only if a decimal point is present (e.g. `150.` has 3 sig figs, `150` has 2, `150.0` has 4, `12.30` has 4, `0.00450` has 3).
//! - **Rule 5**: Exact numbers have infinite significant figures. Pure integer multipliers/coefficients (like `3` in `3 * 4.52 g`), mathematical powers (`^2`), and defined conversion factors do not limit calculation precision.

/// Count the number of significant figures in a numeric string literal without heap allocations.
#[must_use]
pub fn count_significant_figures(s: &str) -> Option<usize> {
    let s = s.trim().trim_start_matches('+').trim_start_matches('-');
    if s.is_empty() {
        return None;
    }

    // Split mantissa from exponent if present (e.g. 1.23e4 -> 1.23, 150.e2 -> 150.)
    let mantissa = s.split(['e', 'E']).next()?.trim();
    if mantissa.is_empty() {
        return None;
    }

    if let Some((int_part, frac_part)) = mantissa.split_once('.') {
        let int_bytes = int_part.as_bytes();
        let frac_bytes = frac_part.as_bytes();

        let int_digits_count = int_bytes.iter().filter(|&&b| b.is_ascii_digit()).count();
        let frac_digits_count = frac_bytes.iter().filter(|&&b| b.is_ascii_digit()).count();

        if int_digits_count + frac_digits_count == 0 {
            return None;
        }

        let first_int_non_zero = int_bytes
            .iter()
            .filter(|&&b| b.is_ascii_digit())
            .position(|&b| b != b'0');
        let first_frac_non_zero = frac_bytes
            .iter()
            .filter(|&&b| b.is_ascii_digit())
            .position(|&b| b != b'0');

        if let Some(first_non_zero) = first_int_non_zero {
            // Rule 1, 2, 4: Non-zero in integer part, all remaining integer digits (captive/trailing)
            // and all fractional digits (trailing zeros after decimal are significant) count!
            let count = (int_digits_count - first_non_zero) + frac_digits_count;
            Some(count.max(1))
        } else if let Some(first_non_zero) = first_frac_non_zero {
            // Rule 3: Leading zeros before first non-zero digit are not significant.
            let count = frac_digits_count - first_non_zero;
            Some(count.max(1))
        } else {
            // All zeros with decimal point (e.g. "0.0" -> 2 sig figs, "0.00" -> 3 sig figs, "0." -> 1 sig fig)
            Some((int_digits_count + frac_digits_count).max(1))
        }
    } else {
        // No decimal point (e.g. "150", "1200", "5005", "45")
        let bytes = mantissa.as_bytes();
        let digit_count = bytes.iter().filter(|&&b| b.is_ascii_digit()).count();
        if digit_count == 0 {
            return None;
        }
        let first_non_zero = bytes
            .iter()
            .filter(|&&b| b.is_ascii_digit())
            .position(|&b| b != b'0');
        let rev_non_zero = bytes
            .iter()
            .filter(|&&b| b.is_ascii_digit())
            .rev()
            .position(|&b| b != b'0');

        match (first_non_zero, rev_non_zero) {
            (Some(first), Some(from_end)) => {
                // Rule 3: leading zeros not significant.
                // Rule 4: trailing zeros without decimal not significant.
                // Rule 1 & 2: non-zero digits and captive zeros between them count!
                let count = digit_count.saturating_sub(first + from_end);
                Some(count.max(1))
            }
            _ => Some(1), // All zeros without decimal (e.g. "0", "000") -> 1 sig fig
        }
    }
}

/// Scans an expression string for numeric literals and returns the minimum number of significant figures
/// across all measured numbers found according to the 5 rules.
#[must_use]
pub fn min_significant_figures_in_expr(expr: &str) -> Option<usize> {
    let mut explicit_sig: Option<usize> = None;
    let mut integer_sig: Option<usize> = None;
    let mut chars = expr.char_indices().peekable();
    let mut prev_non_ws: Option<char> = None;

    while let Some(&(i, c)) = chars.peek() {
        // Skip date blocks between '@'
        if c == '@' {
            chars.next();
            while let Some(&(_, ch)) = chars.peek() {
                chars.next();
                if ch == '@' {
                    break;
                }
            }
            prev_non_ws = Some('@');
            continue;
        }

        // Identifiers (function names like `log10`/`atan2`, unit symbols) never contain
        // measured numbers; skip them whole.
        if c.is_alphabetic() || c == '_' {
            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_alphanumeric() || ch == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            prev_non_ws = Some('a');
            continue;
        }

        // A sign directly after `^` (e.g. `^-2`) still belongs to the exact exponent.
        if (c == '-' || c == '+') && prev_non_ws == Some('^') {
            chars.next();
            continue;
        }

        // Parenthesized exponents (e.g. `2^(3.0)`) are exact powers: skip the whole group.
        if c == '(' && prev_non_ws == Some('^') {
            let mut depth = 0usize;
            for (_, ch) in chars.by_ref() {
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            prev_non_ws = Some(')');
            continue;
        }

        // Detect number start (digit or leading dot followed by digit)
        if c.is_ascii_digit()
            || (c == '.'
                && chars
                    .clone()
                    .nth(1)
                    .is_some_and(|(_, next_c)| next_c.is_ascii_digit()))
        {
            let start = i;
            let is_exponent = prev_non_ws == Some('^');
            let mut has_dot = false;
            let mut has_exp = false;

            while let Some(&(_, num_c)) = chars.peek() {
                if num_c.is_ascii_digit() {
                    chars.next();
                } else if num_c == '.' && !has_dot && !has_exp {
                    let mut lookahead = chars.clone();
                    lookahead.next();
                    match lookahead.peek() {
                        Some(&(_, next_c)) if next_c.is_ascii_digit() => {
                            has_dot = true;
                            chars.next();
                        }
                        Some(&(_, '.')) => {
                            break; // range operator `..`
                        }
                        Some(&(_, next_c)) if next_c.is_alphabetic() || next_c == '_' => {
                            break; // Stop number here for property access e.g. .year
                        }
                        _ => {
                            // Trailing decimal point e.g. "150." or "150. m"
                            has_dot = true;
                            chars.next();
                        }
                    }
                } else if (num_c == 'e' || num_c == 'E') && !has_exp {
                    let mut lookahead = chars.clone();
                    lookahead.next();
                    if let Some(&(_, sign_c)) = lookahead.peek()
                        && (sign_c == '+' || sign_c == '-')
                    {
                        lookahead.next();
                    }
                    if let Some(&(_, digit_c)) = lookahead.peek()
                        && digit_c.is_ascii_digit()
                    {
                        has_exp = true;
                        chars.next(); // 'e' or 'E'
                        if let Some(&(_, sign_c)) = chars.peek()
                            && (sign_c == '+' || sign_c == '-')
                        {
                            chars.next();
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            let end_idx = chars.peek().map_or(expr.len(), |&(idx, _)| idx);
            let num_str = &expr[start..end_idx];

            // Rule 5: Exponents (e.g. `^2`) are exact powers with infinite precision.
            if !is_exponent && let Some(sig) = count_significant_figures(num_str) {
                if has_dot || has_exp {
                    // Explicit measurement precision (e.g. `4.52`, `150.`, `1.23e4`)
                    explicit_sig = Some(explicit_sig.map_or(sig, |m| m.min(sig)));
                } else {
                    // Pure integer literal (e.g. `3`, `150`)
                    integer_sig = Some(integer_sig.map_or(sig, |m| m.min(sig)));
                }
            }
            prev_non_ws = num_str.chars().last();
            continue;
        }

        if !c.is_whitespace() {
            prev_non_ws = Some(c);
        }
        chars.next();
    }

    // Rule 5: If any explicit measurement is present, exact integer coefficients/counts
    // do not limit precision. If only integers are present, integer sig figs apply.
    explicit_sig.or(integer_sig)
}
