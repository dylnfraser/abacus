//! Locks in how a broad sample of expressions evaluates, so refactors can't silently change
//! results. `tests/data/golden.tsv` holds one expression per line followed by its expected
//! results under the standard engine and under degrees + strict dimensions (tab-separated).
//!
//! After an intended change in behaviour, regenerate the file and review its diff:
//!
//! ```text
//! ABACUS_BLESS=1 cargo test --test golden_tests
//! ```
#![cfg(all(
    feature = "units",
    feature = "stats",
    feature = "distributions",
    feature = "date",
    feature = "financial",
    feature = "number-scales",
    feature = "currencies"
))]

use abacus::{Abacus, AngleMode, Date, TimeZone};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden.tsv");

fn engines() -> [Abacus; 2] {
    // A fixed timezone, so results don't depend on the machine's.
    let tz = TimeZone::parse("UTC").unwrap();
    let now: Date = "2026-10-21 09:30:00".parse().unwrap();
    [
        Abacus::standard()
            .with_now(now.clone())
            .with_default_timezone(tz.clone()),
        Abacus::standard()
            .with_now(now)
            .with_default_timezone(tz)
            .with_angle_mode(AngleMode::Degrees)
            .with_strict_dimensions(true),
    ]
}

fn outcome(calc: &Abacus, expr: &str) -> String {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| calc.eval(expr)));
    let text = match result {
        Ok(Ok(value)) => format!("OK {}", value.to_display()),
        Ok(Err(error)) => format!("ERR {error:?}"),
        Err(_) => "PANIC".to_string(),
    };
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

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

/// Whether two outcomes match, allowing the last few bits of each number to differ: `sin`,
/// `exp` and friends come from the platform's libm, which isn't the same on every OS.
fn same_outcome(expected: &str, actual: &str) -> bool {
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

#[test]
fn golden_results_are_unchanged() {
    let engines = engines();
    let file = std::fs::read_to_string(PATH).unwrap();
    let bless = std::env::var_os("ABACUS_BLESS").is_some();
    let mut blessed = String::new();
    let mut failures = Vec::new();
    let mut count = 0;
    for line in file.lines().filter(|l| !l.is_empty()) {
        let mut fields = line.split('\t');
        let expr = fields.next().unwrap();
        let expected: Vec<&str> = fields.collect();
        let actual: Vec<String> = engines.iter().map(|e| outcome(e, expr)).collect();
        count += 1;
        if bless {
            blessed.push_str(expr);
            for a in &actual {
                blessed.push('\t');
                blessed.push_str(a);
            }
            blessed.push('\n');
        } else if !expected
            .iter()
            .zip(&actual)
            .all(|(e, a)| same_outcome(e, a))
            || expected.len() != actual.len()
        {
            failures.push(format!(
                "{expr}\n    expected {expected:?}\n    actual   {actual:?}"
            ));
        }
    }
    if bless {
        std::fs::write(PATH, blessed).unwrap();
        return;
    }
    assert!(count > 1000, "golden file looks truncated ({count} lines)");
    assert!(
        failures.is_empty(),
        "{} of {count} golden expressions changed (rerun with ABACUS_BLESS=1 if intended):\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
