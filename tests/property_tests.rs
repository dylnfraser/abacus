//! Property tests: invariants checked over many generated inputs.
//!
//! Inputs come from a small seeded generator, so every run checks the same cases and a
//! failure names the input that broke the property. Set `ABACUS_PROPTEST_SEED` to explore a
//! different (still reproducible) set.
#![cfg(all(feature = "units", feature = "stats"))]

use abacus::{Abacus, AbacusError, EvalResult, Value};

/// xorshift64*: tiny, deterministic and good enough for test inputs.
struct Rng(u64);

impl Rng {
    fn new(stream: u64) -> Self {
        let seed = std::env::var("ABACUS_PROPTEST_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0x5eed_ab ^ 0x9e37_79b9_7f4a_7c15);
        Rng((seed ^ stream.wrapping_mul(0x2545_f491_4f6c_dd1d)) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// A magnitude spread over many orders, including zero, negatives and whole numbers.
    fn magnitude(&mut self) -> f64 {
        match self.below(6) {
            0 => 0.0,
            1 => self.below(2000) as f64 - 1000.0,
            _ => {
                let mantissa = (self.next() >> 11) as f64 / (1u64 << 53) as f64 + 0.5;
                let exponent = self.below(19) as i32 - 9;
                let sign = if self.below(3) == 0 { -1.0 } else { 1.0 };
                sign * mantissa * 10f64.powi(exponent)
            }
        }
    }
}

const CASES: usize = 2_000;

/// Units grouped by dimension; any two in a group convert into each other.
const UNIT_GROUPS: &[&[&str]] = &[
    &["m", "km", "cm", "mm", "ft", "inch", "mi", "yd", "nmi"],
    &["kg", "g", "mg", "lb", "oz", "tonne"],
    &["s", "ms", "min", "h", "day", "week"],
    &["J", "kJ", "cal", "kWh", "BTU", "eV"],
    &["m/s", "km/h", "mph", "ft/s", "knot"],
    &["L", "mL", "gal", "m^3", "cup"],
    &["Pa", "kPa", "bar", "atm", "psi"],
    &["W", "kW", "hp"],
    &["K", "degC", "degF"],
];

fn calc() -> Abacus {
    Abacus::standard()
}

/// The unit `symbol` names (compound symbols like `m/s` included).
fn unit(calc: &Abacus, symbol: &str) -> std::sync::Arc<abacus::Unit> {
    calc.eval_scalar(&format!("1 {symbol}"))
        .unwrap_or_else(|e| panic!("{symbol}: {e}"))
        .unit
}

fn random_value(rng: &mut Rng, calc: &Abacus) -> Value {
    let group = rng.pick(UNIT_GROUPS);
    let unit = unit(&calc, rng.pick(group));
    Value::new(rng.magnitude(), unit)
}

#[test]
fn units_used_by_these_tests_exist() {
    let calc = calc();
    for group in UNIT_GROUPS {
        let dims: Vec<_> = group
            .iter()
            .map(|u| {
                calc.eval_scalar(&format!("1 {u}"))
                    .unwrap_or_else(|e| panic!("{u}: {e}"))
                    .unit
                    .dimensions
            })
            .collect();
        assert!(dims.windows(2).all(|w| w[0] == w[1]), "{group:?}");
    }
}

#[test]
fn value_equality_is_an_equivalence() {
    let calc = calc();
    let mut rng = Rng::new(1);
    for _ in 0..CASES {
        let a = random_value(&mut rng, &calc);
        // Often the same quantity in another unit of its dimension, so equal pairs occur.
        let b = if rng.below(2) == 0 {
            let group = UNIT_GROUPS
                .iter()
                .find(|g| unit(&calc, g[0]).dimensions == a.unit.dimensions)
                .unwrap();
            Value::from_canonical(a.canonical, unit(&calc, rng.pick(group)))
        } else {
            random_value(&mut rng, &calc)
        };
        let c = Value::from_canonical(b.canonical, b.unit.clone());
        assert_eq!(a, a, "reflexive: {a}");
        assert_eq!(a == b, b == a, "symmetric: {a} vs {b}");
        if a == b && b == c {
            assert_eq!(a, c, "transitive: {a}, {b}, {c}");
        }
        if a == b {
            assert!(a.approx_eq(&b, 0.0), "equal implies approx_eq: {a} vs {b}");
        }
        assert_eq!(a.approx_eq(&b, 1e-9), b.approx_eq(&a, 1e-9), "{a} ~ {b}");
    }
}

#[test]
fn value_addition_is_commutative_and_exact() {
    let calc = calc();
    let mut rng = Rng::new(2);
    for _ in 0..CASES {
        let a = random_value(&mut rng, &calc);
        let b = Value::from_canonical(rng.magnitude(), a.unit.clone());
        if a.unit.is_affine() {
            continue;
        }
        let ab = (&a + &b).unwrap();
        let ba = (&b + &a).unwrap();
        assert_eq!(ab, ba, "{a} + {b}");
        // No snapping: the result is exactly the IEEE sum of the canonical magnitudes.
        assert_eq!(ab.canonical, a.canonical + b.canonical, "{a} + {b}");
        assert_eq!((&a - &b).unwrap().canonical, a.canonical - b.canonical);
    }
}

#[test]
fn values_of_different_dimensions_never_add() {
    let calc = calc();
    let mut rng = Rng::new(3);
    for _ in 0..CASES {
        let a = random_value(&mut rng, &calc);
        let b = random_value(&mut rng, &calc);
        if a.unit.dimensions != b.unit.dimensions {
            assert!((&a + &b).is_err(), "{a} + {b}");
            assert_ne!(a, b);
        }
    }
}

#[test]
fn conversions_round_trip() {
    let calc = calc();
    let mut rng = Rng::new(4);
    for _ in 0..CASES {
        let group = rng.pick(UNIT_GROUPS);
        let (from, to) = (*rng.pick(group), *rng.pick(group));
        let x = rng.magnitude();
        let start = Value::new(x, unit(&calc, from));
        let there = start.convert_to(unit(&calc, to)).unwrap();
        let back = there.convert_to(unit(&calc, from)).unwrap();
        assert!(
            back.approx_eq(&start, 1e-12) || (back.amount() - x).abs() < 1e-9,
            "{x} {from} -> {to} -> {from}: {} ({} {to})",
            back.amount(),
            there.amount()
        );
        assert!(there.approx_eq(&start, 1e-12), "conversion keeps the quantity");
    }
}

#[test]
fn conversions_through_the_parser_round_trip() {
    let calc = calc();
    let mut rng = Rng::new(5);
    for _ in 0..CASES / 4 {
        let group = rng.pick(UNIT_GROUPS);
        let (from, to) = (*rng.pick(group), *rng.pick(group));
        let x = rng.below(1_000_000) as f64 / 100.0;
        let expr = format!("({x} {from} to {to}) to {from}");
        let back = calc
            .eval_unformatted(&expr)
            .and_then(EvalResult::into_scalar)
            .unwrap_or_else(|e| panic!("{expr}: {e}"));
        let start = Value::new(x, unit(&calc, from));
        assert!(
            back.approx_eq(&start, 1e-12),
            "{expr} = {back} (want {start})"
        );
    }
}

#[test]
fn integer_arithmetic_is_exact() {
    let calc = calc();
    let mut rng = Rng::new(6);
    for _ in 0..CASES {
        let a = rng.below(2_000_001) as i64 - 1_000_000;
        let b = rng.below(2_001) as i64 - 1_000;
        let (op, want) = match rng.below(3) {
            0 => ("+", a + b),
            1 => ("-", a - b),
            _ => ("*", a * b),
        };
        let expr = format!("{a} {op} ({b})");
        let got = calc.eval_scalar(&expr).unwrap().canonical;
        assert_eq!(got, want as f64, "{expr}");
    }
}

#[test]
fn display_reads_back_as_the_same_value() {
    // Re-evaluating a result's display text gives the same quantity.
    let calc = calc();
    let mut rng = Rng::new(7);
    for _ in 0..CASES / 2 {
        let group = rng.pick(UNIT_GROUPS);
        let unit = *rng.pick(group);
        let x = rng.magnitude();
        let Ok(first) = calc.eval_scalar(&format!("{x} {unit}")) else {
            continue;
        };
        let shown = first.to_display();
        let second = calc
            .eval_scalar(&shown)
            .unwrap_or_else(|e| panic!("{shown} (from {x} {unit}): {e}"));
        if first.unit.scalar == 1.0 && first.unit.offset == 0.0 {
            assert_eq!(shown, second.to_display(), "from {x} {unit}");
        } else {
            // Scaling to and from the unit's SI magnitude can move the last binary digit.
            assert!(second.approx_eq(&first, 1e-14), "{shown} reads back as {second}");
        }
    }
}

#[test]
fn compiled_expressions_match_eval_as_variables_change() {
    const TEMPLATES: &[&str] = &[
        "x + y",
        "2x^2 - y",
        "x m / (y s)",
        "sqrt(x^2 + y^2)",
        "max(x, y) * 3 kg",
        "x h + 2 h",
        "2h * x",
        "m^2 * y",
        "x km in m",
        "[x, x + 1] * y",
        "mean(x, y, 3)",
        "y% of x",
    ];
    let mut rng = Rng::new(8);
    let mut calc = calc().with_variable("x", 1.0).with_variable("y", 2.0);
    let compiled: Vec<_> = TEMPLATES
        .iter()
        .map(|src| (src, calc.compile(src).unwrap()))
        .collect();
    for _ in 0..CASES / 4 {
        // Change values, and sometimes shadow (or stop shadowing) the units `h` and `m`.
        calc.set_variable("x", (rng.below(200) as f64 - 100.0) / 4.0);
        calc.set_variable("y", (rng.below(200) as f64 + 1.0) / 8.0);
        for name in ["h", "m"] {
            match rng.below(4) {
                0 => {
                    calc.set_variable(name, rng.below(10) as f64);
                }
                1 => {
                    calc.remove_variable(name);
                }
                _ => {}
            }
        }
        for (src, expr) in &compiled {
            let show = |r: Result<EvalResult, AbacusError>| r.map(|r| r.to_display());
            assert_eq!(
                show(calc.eval_compiled(expr)),
                show(calc.eval(src)),
                "{src} with {:?}",
                ["x", "y", "h", "m"].map(|v| calc.get_variable(v).map(|r| r.to_display()))
            );
        }
    }
}

#[test]
fn diagnostic_spans_lie_within_the_source() {
    const PIECES: &[&str] = &[
        "1", "2.5", "1e3", " ", "+", "-", "*", "/", "^", "(", ")", "[", "]", ",", "..", "m", "km",
        "s", "h", "kg", "x", "sin", "sqrt", "mean", "in", "to", "%", "!", "$", "€", "°", "é", "π",
        "#", "@", "of", "ago", "days", "2026-01-01", "pm", "deg", "foo", ".slope",
    ];
    let calc = calc();
    let mut rng = Rng::new(9);
    for _ in 0..CASES * 2 {
        let len = 1 + rng.below(9);
        let src: String = (0..len).map(|_| *rng.pick(PIECES)).collect();
        let Err(d) = calc.eval_detailed(&src) else {
            continue;
        };
        if let Some(span) = d.span() {
            assert!(
                span.start <= span.end && src.get(span.start..span.end).is_some(),
                "{src:?}: span {span:?} out of bounds or splits a character"
            );
        }
        // Rendering never panics and always names the error.
        assert!(d.to_string().starts_with(&d.error().to_string()), "{src:?}");
    }
}
