//! Engine options shared by the `abacus` command line and the `repl` (included by both with
//! `#[path]`; not part of the library). Each binary uses only some of it.
#![allow(dead_code)]

use abacus::{Abacus, AngleMode, DateFormat, Notation, TimeZone};

/// Options that take a value (`--decimals 2` or `--decimals=2`).
pub const VALUE_OPTIONS: &[&str] = &[
    "--decimals",
    "--sig-figs",
    "--notation",
    "--timezone",
    "--date-format",
];

/// Help text for the options, as shown by `--help`.
pub const OPTIONS_HELP: &str = "  --degrees, --radians, --gradians
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
                         rounding it to 0 (0.1 + 0.2 - 0.3)";

/// Applies an option that takes no value; returns whether `name` was one.
pub fn apply_switch(calc: &mut Abacus, name: &str) -> bool {
    match name {
        "--degrees" => calc.set_angle_mode(AngleMode::Degrees),
        "--radians" => calc.set_angle_mode(AngleMode::Radians),
        "--gradians" => calc.set_angle_mode(AngleMode::Gradians),
        "--strict" => calc.set_strict_dimensions(true),
        "--no-implicit" => calc.set_implicit_multiplication(false),
        "--exact" => calc.set_snap_cancellation(false),
        _ => return false,
    };
    true
}

/// Applies option `name` (one of [`VALUE_OPTIONS`]) with `value`, or explains what was wrong.
pub fn apply_option(calc: &mut Abacus, name: &str, value: &str) -> Result<(), String> {
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
                return Err(format!("{name} must be at least 1"));
            }
            calc.set_significant_figures(n);
        }
        "--notation" => {
            calc.set_notation(parse_notation(value).ok_or_else(|| {
                format!("{name} expects standard, scientific or engineering, got `{value}`")
            })?);
        }
        "--timezone" => {
            let tz = TimeZone::parse(value).map_err(|e| format!("{name}: {e}"))?;
            calc.set_default_timezone(tz);
        }
        "--date-format" => {
            calc.set_date_format(
                parse_date_format(value)
                    .ok_or_else(|| format!("{name} expects long, iso, us or eu, got `{value}`"))?,
            );
        }
        _ => return Err(format!("unknown option `{name}`")),
    }
    Ok(())
}

pub fn parse_notation(value: &str) -> Option<Notation> {
    Some(match value {
        "standard" | "std" => Notation::Standard,
        "scientific" | "sci" => Notation::Scientific,
        "engineering" | "eng" => Notation::Engineering,
        _ => return None,
    })
}

pub fn notation_name(notation: Notation) -> &'static str {
    match notation {
        Notation::Scientific => "scientific",
        Notation::Engineering => "engineering",
        _ => "standard",
    }
}

pub fn parse_date_format(value: &str) -> Option<DateFormat> {
    Some(match value {
        "long" => DateFormat::MonthDayYear,
        "iso" => DateFormat::YYYYMMDD,
        "us" => DateFormat::MMDDYYYY,
        "eu" => DateFormat::DDMMYYYY,
        _ => return None,
    })
}

pub fn date_format_name(format: DateFormat) -> &'static str {
    match format {
        DateFormat::YYYYMMDD => "iso",
        DateFormat::MMDDYYYY => "us",
        DateFormat::DDMMYYYY => "eu",
        _ => "long",
    }
}

/// What the command-line arguments asked for, after options were applied to the engine.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub exprs: Vec<String>,
    pub help: bool,
    pub version: bool,
    pub demo: bool,
}

/// Applies the options in `args` to `calc` and collects everything else as expressions.
pub fn parse_args(
    calc: &mut Abacus,
    args: impl IntoIterator<Item = String>,
) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--" => {
                parsed.exprs.extend(args);
                break;
            }
            "-h" | "--help" => parsed.help = true,
            "-V" | "--version" => parsed.version = true,
            "--demo" => parsed.demo = true,
            flag if apply_switch(calc, flag) => {}
            // `-5 + 2` is an expression; only `--name` is an option.
            flag if flag.starts_with("--") => {
                let (name, inline) = match flag.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_string())),
                    None => (flag, None),
                };
                if !VALUE_OPTIONS.contains(&name) {
                    return Err(format!("unknown option `{name}`"));
                }
                let value = inline
                    .or_else(|| args.next())
                    .ok_or_else(|| format!("{name} needs a value"))?;
                apply_option(calc, name, &value)?;
            }
            _ => parsed.exprs.push(arg),
        }
    }
    Ok(parsed)
}
