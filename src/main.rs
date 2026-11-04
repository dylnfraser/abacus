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

const USAGE: &str = "\
Usage: abacus [OPTIONS] [EXPRESSION]...

Evaluates each EXPRESSION (quote expressions containing spaces or shell
metacharacters). With no expressions, reads one expression per line from
stdin; blank lines and lines starting with '#' are skipped. Variables
assigned with `name = expr` persist across expressions.

Options:
  --degrees, --radians, --gradians
                         Angle unit for bare numbers in trig functions
                         (default: radians)
  --decimals N           Round results to N decimal places
  --sig-figs N           Round results to N significant figures
  --notation MODE        standard, scientific or engineering
  --timezone TZ          Timezone for dates and times written without one
                         (EST, UTC, +05:30, ...)
  --date-format FORMAT   long (August 7, 2026), iso (2026-08-07),
                         us (08-07-2026) or eu (07-08-2026); us also reads
                         ambiguous NN-NN-YYYY input month-first
  --strict               Reject adding a bare number to a quantity (5 m + 3)
  --no-implicit          Require an explicit * (reject 2(3), 2 pi)
  --exact                Report floating-point cancellation noise instead of
                         rounding it to 0 (0.1 + 0.2 - 0.3)
  --demo                 Print a tour of supported features
  -h, --help             Print this help
  -V, --version          Print version
  --                     Treat all following arguments as expressions";

/// Options that take a value (`--decimals 2` or `--decimals=2`).
const VALUE_OPTIONS: &[&str] = &[
    "--decimals",
    "--sig-figs",
    "--notation",
    "--timezone",
    "--date-format",
];

/// Applies option `name` with `value` to `calc`, or explains what was wrong with it.
fn apply_option(calc: &mut Abacus, name: &str, value: &str) -> Result<(), String> {
    use abacus::{DateFormat, Notation, TimeZone};
    let count = || {
        value
            .parse::<usize>()
            .map_err(|_| format!("{name} expects a whole number, got `{value}`"))
    };
    match name {
        "--decimals" => {
            calc.set_decimal_places(count()?);
        }
        "--sig-figs" => {
            let n = count()?;
            if n == 0 {
                return Err("--sig-figs must be at least 1".to_string());
            }
            calc.set_significant_figures(n);
        }
        "--notation" => {
            calc.set_notation(match value {
                "standard" | "std" => Notation::Standard,
                "scientific" | "sci" => Notation::Scientific,
                "engineering" | "eng" => Notation::Engineering,
                _ => {
                    return Err(format!(
                        "--notation expects standard, scientific or engineering, got `{value}`"
                    ));
                }
            });
        }
        "--timezone" => {
            let tz = TimeZone::parse(value).map_err(|e| format!("--timezone: {e}"))?;
            calc.set_default_timezone(tz);
        }
        "--date-format" => {
            calc.set_date_format(match value {
                "long" => DateFormat::MonthDayYear,
                "iso" => DateFormat::YYYYMMDD,
                "us" => DateFormat::MMDDYYYY,
                "eu" => DateFormat::DDMMYYYY,
                _ => {
                    return Err(format!(
                        "--date-format expects long, iso, us or eu, got `{value}`"
                    ));
                }
            });
        }
        _ => return Err(format!("unknown option `{name}`")),
    }
    Ok(())
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
    let mut exprs = Vec::new();
    let mut flags_done = false;
    let mut demo = false;
    let mut args = std::env::args().skip(1);
    let usage_error = |message: String| {
        eprintln!("error: {message}\n\n{USAGE}");
        ExitCode::from(2)
    };

    while let Some(arg) = args.next() {
        if flags_done {
            exprs.push(arg);
            continue;
        }
        match arg.as_str() {
            "--" => flags_done = true,
            "-h" | "--help" => {
                out!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                out!("abacus {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--demo" => demo = true,
            "--degrees" => {
                calc.set_angle_mode(abacus::AngleMode::Degrees);
            }
            "--radians" => {
                calc.set_angle_mode(abacus::AngleMode::Radians);
            }
            "--gradians" => {
                calc.set_angle_mode(abacus::AngleMode::Gradians);
            }
            "--strict" => {
                calc.set_strict_dimensions(true);
            }
            "--no-implicit" => {
                calc.set_implicit_multiplication(false);
            }
            "--exact" => {
                calc.set_snap_cancellation(false);
            }
            flag if flag.starts_with("--") => {
                let (name, inline) = match flag.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_string())),
                    None => (flag, None),
                };
                if !VALUE_OPTIONS.contains(&name) {
                    return usage_error(format!("unknown option `{name}`"));
                }
                let Some(value) = inline.or_else(|| args.next()) else {
                    return usage_error(format!("{name} needs a value"));
                };
                if let Err(message) = apply_option(&mut calc, name, &value) {
                    return usage_error(message);
                }
            }
            _ => exprs.push(arg),
        }
    }

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
