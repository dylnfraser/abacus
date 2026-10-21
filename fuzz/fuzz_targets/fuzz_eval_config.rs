#![no_main]

//! Fuzzes evaluation under varied engine configurations. The first two bytes select the
//! configuration; the rest is the expression.

use abacus::{Abacus, AngleMode, IntervalStyle, Notation};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let [a, b, rest @ ..] = data else {
        return;
    };
    let Ok(expr) = std::str::from_utf8(rest) else {
        return;
    };

    let mut calc = Abacus::standard()
        .with_angle_mode(match a & 0b11 {
            0 => AngleMode::Radians,
            1 => AngleMode::Degrees,
            _ => AngleMode::Gradians,
        })
        .with_notation(match (a >> 2) & 0b11 {
            0 => Notation::Standard,
            1 => Notation::Scientific,
            _ => Notation::Engineering,
        })
        .with_interval_style(if a & 0b1_0000 == 0 {
            IntervalStyle::Bracket
        } else {
            IntervalStyle::Range
        })
        .with_implicit_multiplication(a & 0b10_0000 == 0)
        .with_auto_derived_units(a & 0b100_0000 == 0)
        .with_follow_significant_figures(a & 0b1000_0000 != 0);

    match b & 0b11 {
        1 => calc = calc.with_significant_figures(usize::from(b >> 2) % 20 + 1),
        2 => calc = calc.with_decimal_places(usize::from(b >> 2) % 20),
        _ => {}
    }

    // Exercise both the immutable and the variable-assigning entry points.
    let _ = calc.eval(expr);
    let _ = calc.eval_mut(expr);
});
