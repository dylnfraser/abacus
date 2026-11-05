//! Helpers shared by integration tests.

enum Piece {
    Number(f64),
    Char(char),
}

/// Splits `text` into numbers and the characters between them.
fn pieces(text: &str) -> Vec<Piece> {
    let bytes = text.as_bytes();
    let digit = |i: usize| bytes.get(i).is_some_and(u8::is_ascii_digit);
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if digit(i) || (bytes[i] == b'.' && digit(i + 1)) {
            let start = i;
            while digit(i) || bytes.get(i) == Some(&b'.') {
                i += 1;
            }
            if matches!(bytes.get(i), Some(b'e' | b'E')) {
                let sign = usize::from(matches!(bytes.get(i + 1), Some(b'+' | b'-')));
                if digit(i + 1 + sign) {
                    i += 1 + sign;
                    while digit(i) {
                        i += 1;
                    }
                }
            }
            match text[start..i].parse() {
                Ok(n) => out.push(Piece::Number(n)),
                Err(_) => out.extend(text[start..i].chars().map(Piece::Char)),
            }
        } else {
            let c = text[i..].chars().next().unwrap();
            out.push(Piece::Char(c));
            i += c.len_utf8();
        }
    }
    out
}

/// Whether two results match, allowing the last few bits of each number to differ: `sin`,
/// `exp` and friends come from the platform's libm, which isn't the same on every OS.
pub fn same_outcome(expected: &str, actual: &str) -> bool {
    if expected == actual {
        return true;
    }
    let (e, a) = (pieces(expected), pieces(actual));
    e.len() == a.len()
        && e.iter().zip(&a).all(|pair| match pair {
            (Piece::Number(x), Piece::Number(y)) => {
                x == y || (x - y).abs() <= 1e-12 * x.abs().max(y.abs())
            }
            (Piece::Char(x), Piece::Char(y)) => x == y,
            _ => false,
        })
}
