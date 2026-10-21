//! `abacus` command-line calculator.
//!
//! ```text
//! abacus "10 N * 5 m" "100 km/h in m/s"   # evaluate each argument
//! echo "x = 5 m" | abacus                 # evaluate stdin line by line
//! abacus --demo                           # feature tour
//! ```

use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;

use abacus::Abacus;

const USAGE: &str = "\
Usage: abacus [OPTIONS] [EXPRESSION]...

Evaluates each EXPRESSION (quote expressions containing spaces or shell
metacharacters). With no expressions, reads one expression per line from
stdin; blank lines and lines starting with '#' are skipped. Variables
assigned with `name = expr` persist across expressions.

Options:
  --demo         Print a tour of supported features
  -h, --help     Print this help
  -V, --version  Print version
  --             Treat all following arguments as expressions";

const DEMO: &[(&str, &[&str])] = &[
    (
        "Physical Unit Arithmetic",
        &["10 N * 5 m", "100 km / 2 h to m/s", "500 J / 10 s"],
    ),
    (
        "Interval Arithmetic",
        &[
            "[9.8 m, 10.2 m] / [1.9 s, 2.1 s]",
            "5 m + [1 m, 3 m]",
            "[1 km, 2 km] to m",
            "[10 N, 20 N] * [2 m, 5 m]",
        ],
    ),
    (
        "Inferential Statistics & Confidence Intervals",
        &[
            "TInterval(10 m, 12 m, 11 m, 14 m)",
            "ZInterval(100 m, 15 m, 100)",
            "1-PropZInt(45, 100)",
            "2-SampTInt(100 m, 15 m, 25, 90 m, 10 m, 30)",
        ],
    ),
    (
        "Hypothesis Testing",
        &[
            "ZTest(100 m, 105 m, 15 m, 50)",
            "ZTest(100 m, 105 m, 15 m, 50).p_value",
            "TTest(100 m, 105 m, 15 m, 25)",
            "2-SampTTest(100 m, 15 m, 25, 90 m, 10 m, 30)",
            "Chi2Test(15, 25, 10, 30).p_value",
        ],
    ),
    (
        "Dimension-Aware Linear Regression & Dot Property Access",
        &[
            "linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)",
            "linreg(1 s, 2 s, 3 s, 4 s, 15 m, 25 m, 35 m, 45 m).intercept",
            "linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s",
        ],
    ),
    (
        "Dates, Times & Time Zones",
        &[
            "07-08-2026 + 5 days",
            "2026/08/07 - 2 weeks",
            "2026-08-07 10:30:00 + 3 hours",
            "07-08-2026 10:00 AM EST to PST",
            "2026-08-07 10:00:00 -04:00 to +02:00",
            "07-08-2026 to 17-08-2026 in days",
            "year(17-08-2026)",
            "07-08-2026.day_of_week",
            "last thursday at 3pm",
            "in 3 hours",
            "date(2026, 8, 7, 10, 54, 49)",
        ],
    ),
    (
        "Business Days",
        &[
            "07-08-2026 + 5 business days",
            "workdays(07-08-2026, 14-08-2026)",
            "is_weekend(07-08-2026)",
        ],
    ),
    (
        "Customary Units & Currencies",
        &[
            "5 pounds to kg",
            "1 gal in L",
            "1 imp_gal in L",
            "2 tonnes in ton",
            "1 kWh in BTU",
            "$50 + €20 in USD",
        ],
    ),
];

fn run_demo(calc: &Abacus) -> bool {
    let mut ok = true;
    for (i, (title, exprs)) in DEMO.iter().enumerate() {
        println!("{}. {title}", i + 1);
        for expr in *exprs {
            match calc.eval(expr) {
                Ok(res) => println!("  {expr} = {}", res.to_display()),
                Err(err) => {
                    ok = false;
                    println!("  {expr} => error: {err}");
                }
            }
        }
        println!();
    }
    ok
}

/// Evaluates one expression, printing the result to stdout or the error to stderr.
fn eval_one(calc: &mut Abacus, expr: &str) -> bool {
    match calc.eval_mut(expr) {
        Ok(res) => {
            println!("{}", res.to_display());
            true
        }
        Err(err) => {
            eprintln!("error: {err} (in `{expr}`)");
            false
        }
    }
}

fn main() -> ExitCode {
    let mut calc = Abacus::standard();
    let mut exprs = Vec::new();
    let mut flags_done = false;

    for arg in std::env::args().skip(1) {
        if flags_done {
            exprs.push(arg);
            continue;
        }
        match arg.as_str() {
            "--" => flags_done = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("abacus {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--demo" => {
                return if run_demo(&calc) {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
            }
            flag if flag.starts_with("--") => {
                eprintln!("error: unknown option `{flag}`\n\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => exprs.push(arg),
        }
    }

    let mut ok = true;
    if exprs.is_empty() {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        for line in stdin.lock().lines() {
            let Ok(line) = line else {
                eprintln!("error: failed to read stdin");
                return ExitCode::FAILURE;
            };
            let expr = line.trim();
            if expr.is_empty() || expr.starts_with('#') {
                continue;
            }
            ok &= eval_one(&mut calc, expr);
        }
    } else {
        for expr in &exprs {
            ok &= eval_one(&mut calc, expr);
        }
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
