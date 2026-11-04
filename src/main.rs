//! `abacus` command-line calculator.
//!
//! ```text
//! abacus "10 N * 5 m" "100 km/h in m/s"   # evaluate each argument
//! echo "x = 5 m" | abacus                 # evaluate stdin line by line
//! abacus --degrees --decimals 3 "sin 30"  # with options
//! abacus --demo                           # feature tour
//! ```

use std::io::{BufRead, IsTerminal};
use std::process::ExitCode;

use abacus::Abacus;

/// Like `println!`, but exits quietly when stdout is closed (e.g. `abacus --demo | head`)
/// instead of panicking on a broken pipe.
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

#[path = "cli_options.rs"]
mod cli_options;

const USAGE_HEAD: &str = "\
Usage: abacus [OPTIONS] [EXPRESSION]...

Evaluates each EXPRESSION (quote expressions containing spaces or shell
metacharacters). With no expressions, reads one expression per line from
stdin; blank lines and lines starting with '#' are skipped. Variables
assigned with `name = expr` persist across expressions.

Options:";

const USAGE_TAIL: &str = "  --demo                 Print a tour of supported features
  -h, --help             Print this help
  -V, --version          Print version
  --                     Treat all following arguments as expressions";

fn usage() -> String {
    format!("{USAGE_HEAD}\n{}\n{USAGE_TAIL}", cli_options::OPTIONS_HELP)
}

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
        out!("{}. {title}", i + 1);
        for expr in *exprs {
            match calc.eval(expr) {
                Ok(res) => out!("  {expr} = {}", calc.format_result(&res)),
                Err(err) => {
                    ok = false;
                    out!("  {expr} => error: {err}");
                }
            }
        }
        out!();
    }
    ok
}

/// Evaluates one expression, printing the result to stdout or the error to stderr.
fn eval_one(calc: &mut Abacus, expr: &str) -> bool {
    match calc.eval_mut_detailed(expr) {
        Ok(res) => {
            out!("{}", calc.format_result(&res));
            true
        }
        Err(diagnostic) => {
            // The diagnostic shows the expression with a caret under the problem.
            eprintln!("error: {diagnostic}");
            false
        }
    }
}

fn main() -> ExitCode {
    let mut calc = Abacus::standard();
    let args = match cli_options::parse_args(&mut calc, std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}\n\n{}", usage());
            return ExitCode::from(2);
        }
    };
    if args.help {
        out!("{}", usage());
        return ExitCode::SUCCESS;
    }
    if args.version {
        out!("abacus {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (demo, exprs) = (args.demo, args.exprs);

    if demo {
        return if run_demo(&calc) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    let mut ok = true;
    if exprs.is_empty() {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            eprintln!("{}", usage());
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
