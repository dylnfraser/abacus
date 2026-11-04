//! Interactive `abacus` shell: history, highlighting and completion driven by the engine's own
//! lexer, `:set` commands for engine options, and errors pointed at in the typed line.
//!
//! ```text
//! repl [--degrees] [--decimals N] ...   # the same options as the `abacus` command
//! ```

#[path = "../cli_options.rs"]
mod cli_options;

use std::borrow::Cow;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use abacus::{Abacus, AngleMode, Diagnostic, EvalResult, Span, Token};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{ColorMode, Config, Context, Editor, Helper};

/// Like `println!`, but exits quietly when stdout is closed instead of panicking.
macro_rules! out {
    ($($arg:tt)*) => {{
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

const PROMPT: &str = "› ";
/// Display width of [`PROMPT`], for lining carets up under the input.
const PROMPT_WIDTH: usize = 2;

const HELP: &str = "\
Type an expression to evaluate it (`14 m to ft`, `sin(30 deg)`, `today + 3 weeks`).
`name = expr` defines a variable; `ans` is always the last result.

Commands:
  :set                 Show the engine options
  :set NAME VALUE      Change one: angle (radians|degrees|gradians), decimals N|off,
                       sig-figs N|off, notation (standard|scientific|engineering),
                       timezone TZ|off, date-format (long|iso|us|eu),
                       strict on|off, implicit on|off, exact on|off
  :vars [all]          List variables (`all` includes pi, e, tau, phi)
  :unset NAME...       Remove variables
  :reset               Remove all variables except the standard constants
  :clear               Clear the screen
  :help                Show this help
  :quit                Exit (also Ctrl-D)

Tab completes function, variable, unit and command names. Ctrl-C discards the line.";

const COMMANDS: &[&str] = &[
    ":set", ":vars", ":unset", ":reset", ":clear", ":help", ":quit",
];

/// What a line of input asks the shell to do.
#[derive(Debug, PartialEq)]
enum Action {
    /// Print this (an empty string prints nothing).
    Print(String),
    /// Print this as an error.
    Error(String),
    Clear,
    Quit,
}

/// Handles a `:command` (or one of the bare-word aliases `help`, `clear`, `exit`, `quit`);
/// `None` if `line` is an expression.
fn command(calc: &mut Abacus, line: &str) -> Option<Action> {
    let line = line.trim();
    let mut words = line.split_whitespace();
    let first = words.next()?;
    let name = match first {
        "help" | ".help" | "?" => ":help",
        "clear" | ".clear" => ":clear",
        "exit" | "quit" | ".exit" | ".quit" => ":quit",
        _ if first.starts_with(':') => first,
        _ => return None,
    };
    let args: Vec<&str> = words.collect();
    // Bare aliases take no arguments, so `exit = 5` is still an assignment.
    if !first.starts_with(':') && !args.is_empty() {
        return None;
    }
    Some(match name {
        ":help" | ":h" => Action::Print(HELP.to_string()),
        ":clear" => Action::Clear,
        ":quit" | ":q" | ":exit" => Action::Quit,
        ":set" => match args.as_slice() {
            [] => Action::Print(settings(calc)),
            [key] => Action::Error(format!(":set {key} needs a value (see :help)")),
            [key, value @ ..] => match set(calc, key, &value.join(" ")) {
                Ok(()) => Action::Print(String::new()),
                Err(message) => Action::Error(message),
            },
        },
        ":vars" => Action::Print(variables(calc, args.first() == Some(&"all"))),
        ":unset" if args.is_empty() => Action::Error(":unset needs a variable name".to_string()),
        ":unset" => {
            let missing: Vec<&str> = args
                .iter()
                .copied()
                .filter(|name| calc.remove_variable(name).is_none())
                .collect();
            if missing.is_empty() {
                Action::Print(String::new())
            } else {
                Action::Error(format!("no variable named {}", missing.join(", ")))
            }
        }
        ":reset" => {
            calc.reset_standard_variables();
            Action::Print(String::new())
        }
        other => Action::Error(format!("unknown command `{other}` (see :help)")),
    })
}

/// Changes engine option `key` (see [`HELP`]).
fn set(calc: &mut Abacus, key: &str, value: &str) -> Result<(), String> {
    let on_off = |value: &str| match value {
        "on" | "true" | "yes" => Ok(true),
        "off" | "false" | "no" => Ok(false),
        _ => Err(format!("{key} expects on or off, got `{value}`")),
    };
    match (key, value) {
        ("angle", "radians" | "rad") => {
            calc.set_angle_mode(AngleMode::Radians);
        }
        ("angle", "degrees" | "deg") => {
            calc.set_angle_mode(AngleMode::Degrees);
        }
        ("angle", "gradians" | "grad") => {
            calc.set_angle_mode(AngleMode::Gradians);
        }
        ("angle", _) => {
            return Err(format!(
                "angle expects radians, degrees or gradians, got `{value}`"
            ));
        }
        ("decimals", "off") => {
            calc.set_decimal_places(None);
        }
        ("sig-figs", "off") => {
            calc.set_significant_figures(None);
        }
        ("timezone", "off") => {
            calc.set_default_timezone(None);
        }
        ("decimals" | "sig-figs" | "notation" | "timezone" | "date-format", _) => {
            cli_options::apply_option(calc, &format!("--{key}"), value)
                .map_err(|e| e.trim_start_matches("--").to_string())?;
        }
        ("strict", _) => {
            calc.set_strict_dimensions(on_off(value)?);
        }
        ("implicit", _) => {
            calc.set_implicit_multiplication(on_off(value)?);
        }
        ("exact", _) => {
            calc.set_snap_cancellation(!on_off(value)?);
        }
        _ => return Err(format!("unknown option `{key}` (see :help)")),
    }
    Ok(())
}

/// The current engine options, one per line, in `:set` syntax.
fn settings(calc: &Abacus) -> String {
    let on_off = |b: bool| if b { "on" } else { "off" };
    let or_off = |n: Option<usize>| n.map_or_else(|| "off".to_string(), |n| n.to_string());
    let angle = match calc.angle_mode() {
        AngleMode::Degrees => "degrees",
        AngleMode::Gradians => "gradians",
        AngleMode::Radians => "radians",
    };
    [
        ("angle", angle.to_string()),
        ("decimals", or_off(calc.decimal_places())),
        ("sig-figs", or_off(calc.significant_figures())),
        (
            "notation",
            cli_options::notation_name(calc.notation()).to_string(),
        ),
        (
            "timezone",
            calc.default_timezone()
                .map_or_else(|| "off".to_string(), ToString::to_string),
        ),
        (
            "date-format",
            cli_options::date_format_name(calc.date_format()).to_string(),
        ),
        ("strict", on_off(calc.strict_dimensions()).to_string()),
        (
            "implicit",
            on_off(calc.implicit_multiplication()).to_string(),
        ),
        ("exact", on_off(!calc.snap_cancellation()).to_string()),
    ]
    .iter()
    .map(|(key, value)| format!("  {key:<12} {value}"))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The variables as `name = value` lines, sorted; the standard constants only with `all`.
fn variables(calc: &Abacus, all: bool) -> String {
    let standard = Abacus::standard_variables();
    let mut vars: Vec<(&String, &EvalResult)> = calc
        .variables()
        .iter()
        .filter(|(name, value)| all || standard.get(*name) != Some(value))
        .collect();
    if vars.is_empty() {
        return "  (no variables; define one with `name = expr`)".to_string();
    }
    vars.sort_by(|a, b| a.0.cmp(b.0));
    vars.iter()
        .map(|(name, value)| format!("  {name} = {}", calc.format_result(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `name` if `line` is an assignment `name = expr` (as [`Abacus::eval_mut`] reads it).
fn assigned_name(line: &str) -> Option<&str> {
    let (name, rhs) = line.split_once('=')?;
    let name = name.trim();
    let is_ident = name.starts_with(|c: char| c.is_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    (is_ident && !rhs.starts_with('=')).then_some(name)
}

/// Evaluates `line`, keeping the full-precision result in `ans`, and returns the text to show.
fn evaluate(calc: &mut Abacus, line: &str) -> Result<String, Diagnostic> {
    let raw = if let Some(name) = assigned_name(line) {
        calc.eval_mut_detailed(line)?;
        calc.get_variable(name).cloned().expect("just assigned")
    } else {
        match calc.eval_unformatted(line) {
            Ok(raw) => raw,
            // Evaluate again for the error's location (only on the error path).
            Err(_) => return Err(calc.eval_detailed(line).expect_err("failed once")),
        }
    };
    let shown = calc.apply_output_settings(raw.clone(), line);
    calc.set_variable("ans", raw);
    Ok(format_result(calc, &shown))
}

/// Results are shown as by the `abacus` command, except that plain durations read naturally
/// (`90 min` → `1 hour, 30 minutes`).
fn format_result(calc: &Abacus, result: &EvalResult) -> String {
    match result {
        EvalResult::Scalar(v) if v.unit.is_standard_duration_unit() => {
            abacus::format_human_duration(v.canonical)
        }
        other => calc.format_result(other),
    }
}

/// Lines showing `diagnostic` under input typed after [`PROMPT`]: carets under the problem,
/// then the message and any note.
fn render_error(diagnostic: &Diagnostic) -> String {
    let mut out = String::new();
    if let Some(span) = diagnostic.span() {
        let source = diagnostic.source();
        let before = source.get(..span.start).unwrap_or(source).chars().count();
        let width = span.text(source).chars().count().max(1);
        out.push_str(&" ".repeat(PROMPT_WIDTH + before));
        out.push_str(&"^".repeat(width));
        out.push('\n');
    }
    out.push_str(&format!("error: {}", diagnostic.error()));
    if let Some(note) = diagnostic.note() {
        out.push_str(&format!("\nnote: {note}"));
    }
    out
}

// Highlighting

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Number,
    Unit,
    Function,
    Operator,
    Keyword,
    Variable,
    Date,
    Command,
    Error,
}

impl Style {
    fn ansi(self) -> &'static str {
        match self {
            Style::Number => "\x1b[38;2;241;196;15m",
            Style::Unit | Style::Command => "\x1b[38;2;189;147;249m",
            Style::Function | Style::Operator => "\x1b[38;2;139;233;253m",
            Style::Keyword | Style::Date => "\x1b[38;2;80;250;123m",
            Style::Variable => "\x1b[38;2;255;184;108m",
            Style::Error => "\x1b[4;31m",
        }
    }
}

/// Styled byte ranges of `line` (sorted, non-overlapping), from the engine's own reading of
/// it: a name is coloured as a unit, variable or function exactly when the engine sees it so.
fn styles(calc: &Abacus, line: &str) -> Vec<(Span, Style)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with(':') {
        let start = line.len() - trimmed.len();
        let end = start + trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
        return vec![(Span::new(start, end), Style::Command)];
    }
    if let Some(name) = assigned_name(line) {
        let start = name.as_ptr() as usize - line.as_ptr() as usize;
        let eq = line.find('=').expect("assignment");
        let rhs = &line[eq + 1..];
        let mut out = vec![
            (Span::new(start, start + name.len()), Style::Variable),
            (Span::new(eq, eq + 1), Style::Operator),
        ];
        let shift =
            |(s, style): (Span, Style)| (Span::new(s.start + eq + 1, s.end + eq + 1), style);
        out.extend(styles(calc, rhs).into_iter().map(shift));
        return out;
    }
    match calc.tokenize_spanned(line) {
        Ok(tokens) => token_styles(line, &tokens),
        Err(diagnostic) => {
            // Colour what was read before the problem, and mark the problem itself.
            let Some(bad) = diagnostic.span() else {
                return Vec::new();
            };
            let mut out = calc
                .tokenize_spanned(&line[..bad.start])
                .map(|tokens| token_styles(line, &tokens))
                .unwrap_or_default();
            out.push((bad, Style::Error));
            out
        }
    }
}

fn token_styles(line: &str, tokens: &[(Token<'_>, Span)]) -> Vec<(Span, Style)> {
    let mut out: Vec<(Span, Style)> = Vec::new();
    for (token, span) in tokens {
        let span = *span;
        if span.start == span.end {
            continue; // an inserted implicit `*`
        }
        let wordy = span.text(line).starts_with(char::is_alphabetic);
        let style = match token {
            Token::Float(_) => Style::Number,
            // `5 km`, `$50`, `9 m^2`: the number, and around it the unit.
            Token::Val(_) => {
                let mut seen_number = false;
                let mut after_number = false;
                let mut prev = ' ';
                for (i, c) in span.text(line).char_indices() {
                    let numeric = c.is_ascii_digit()
                        || c == '.'
                        || (seen_number
                            && matches!(c, 'e' | 'E' | '+' | '-')
                            && !after_number
                            && (prev.is_ascii_digit() || matches!(prev, 'e' | 'E')));
                    let style = if c.is_whitespace() {
                        if seen_number {
                            after_number = true;
                        }
                        prev = c;
                        continue;
                    } else if numeric && !after_number {
                        seen_number = true;
                        Style::Number
                    } else {
                        if seen_number {
                            after_number = true;
                        }
                        Style::Unit
                    };
                    prev = c;
                    let at = span.start + i;
                    let end = at + c.len_utf8();
                    match out.last_mut() {
                        Some((s, last)) if *last == style && s.end == at => s.end = end,
                        _ => out.push((Span::new(at, end), style)),
                    }
                }
                continue;
            }
            Token::Unit(_) => Style::Unit,
            Token::Ident(_) => Style::Variable,
            Token::Function(_) | Token::DotProperty(_) => Style::Function,
            // `sqrt` is a prefix operator; `mod`, `per` and `of` are infix ones.
            Token::UnaryOp(_) if wordy => Style::Function,
            Token::BinaryOp(_) if wordy => Style::Keyword,
            Token::UnaryOp(_) | Token::BinaryOp(_) => Style::Operator,
            Token::ConversionOp
            | Token::RelTimeOp(_)
            | Token::PercentChangeFrom
            | Token::PercentTag(_) => Style::Keyword,
            Token::Date(_) => Style::Date,
            _ => Style::Operator,
        };
        out.push((span, style));
    }
    out
}

/// `line` with ANSI colours.
fn highlight(calc: &Abacus, line: &str) -> String {
    const RESET: &str = "\x1b[0m";
    let mut out = String::with_capacity(line.len() * 2);
    let mut at = 0;
    for (span, style) in styles(calc, line) {
        let (Some(gap), Some(text)) = (line.get(at..span.start), line.get(span.start..span.end))
        else {
            continue;
        };
        out.push_str(gap);
        out.push_str(style.ansi());
        out.push_str(text);
        out.push_str(RESET);
        at = span.end;
    }
    out.push_str(line.get(at..).unwrap_or(""));
    out
}

// Completion

/// Names starting with the word before byte `pos`: commands, then variables and functions,
/// then units (once two characters are typed, as there are hundreds).
fn completions(calc: &Abacus, line: &str, pos: usize) -> (usize, Vec<String>) {
    let before = &line[..pos];
    let start = before
        .char_indices()
        .rev()
        .take_while(|&(_, c)| c.is_alphanumeric() || c == '_' || c == ':')
        .last()
        .map_or(pos, |(i, _)| i);
    let prefix = &before[start..];
    if prefix.is_empty() {
        return (pos, Vec::new());
    }
    let mut names: Vec<String> = if prefix.starts_with(':') {
        COMMANDS
            .iter()
            .filter(|c| c.starts_with(prefix))
            .map(ToString::to_string)
            .collect()
    } else {
        let mut first: Vec<&str> = calc
            .variables()
            .keys()
            .map(String::as_str)
            .chain(calc.tokens().function_names())
            .chain(calc.tokens().unary_operator_names())
            .chain(calc.tokens().binary_operator_names())
            .filter(|n| n.starts_with(prefix))
            .collect();
        first.sort_unstable();
        let mut units: Vec<&str> = if prefix.chars().count() >= 2 {
            calc.units()
                .symbols()
                .filter(|n| n.starts_with(prefix) && !first.contains(n))
                .collect()
        } else {
            Vec::new()
        };
        units.sort_unstable_by_key(|n| (n.len(), *n));
        first
            .into_iter()
            .chain(units)
            .map(ToString::to_string)
            .collect()
    };
    names.dedup();
    names.truncate(100);
    (start, names)
}

struct ReplHelper {
    calc: Abacus,
    color: bool,
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, names) = completions(&self.calc, line, pos);
        let pairs = names
            .into_iter()
            .map(|name| Pair {
                display: name.clone(),
                replacement: name,
            })
            .collect();
        Ok((start, pairs))
    }
}

impl Hinter for ReplHelper {
    type Hint = String;
}

impl Validator for ReplHelper {}

impl Helper for ReplHelper {}

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if self.color {
            Cow::Owned(highlight(&self.calc, line))
        } else {
            Cow::Borrowed(line)
        }
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        self.color
    }
}

fn history_path() -> Option<PathBuf> {
    ["HOME", "USERPROFILE", "APPDATA"]
        .iter()
        .find_map(std::env::var_os)
        .map(|home| PathBuf::from(home).join(".abacus_history"))
}

fn paint(color: bool, ansi: &str, text: &str) -> String {
    if color {
        format!("{ansi}{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn main() -> std::process::ExitCode {
    let mut calc = Abacus::standard();
    match cli_options::parse_args(&mut calc, std::env::args().skip(1)) {
        Ok(args) if args.help => {
            out!(
                "Usage: repl [OPTIONS]\n\nOptions:\n{}\n  -h, --help             Print this help\n  -V, --version          Print version\n\n{HELP}",
                cli_options::OPTIONS_HELP
            );
            return std::process::ExitCode::SUCCESS;
        }
        Ok(args) if args.version => {
            out!("abacus repl {}", env!("CARGO_PKG_VERSION"));
            return std::process::ExitCode::SUCCESS;
        }
        Ok(args) if !args.exprs.is_empty() || args.demo => {
            eprintln!("error: the repl takes options only (use `abacus` to evaluate arguments)");
            return std::process::ExitCode::from(2);
        }
        Ok(_) => {}
        Err(message) => {
            eprintln!("error: {message} (see repl --help)");
            return std::process::ExitCode::from(2);
        }
    }

    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let config = Config::builder()
        .color_mode(if color {
            ColorMode::Forced
        } else {
            ColorMode::Disabled
        })
        .auto_add_history(false)
        .build();
    let mut rl: Editor<ReplHelper, rustyline::history::FileHistory> =
        match Editor::with_config(config) {
            Ok(rl) => rl,
            Err(e) => {
                eprintln!("error: failed to start the line editor: {e}");
                return std::process::ExitCode::FAILURE;
            }
        };
    rl.set_helper(Some(ReplHelper { calc, color }));
    let history = history_path();
    if let Some(path) = &history {
        let _ = rl.load_history(path);
    }

    let banner = format!(
        "abacus {} — :help for help, :quit to exit",
        env!("CARGO_PKG_VERSION")
    );
    out!("{}", paint(color, "\x1b[1;36m", &banner));
    let prompt = paint(color, "\x1b[36m", PROMPT);

    loop {
        let line = match rl.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(err) => {
                eprintln!("error: {err}");
                return std::process::ExitCode::FAILURE;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(line);
        if let Some(path) = &history {
            let _ = rl.append_history(path);
        }
        let calc = &mut rl.helper_mut().expect("helper is set").calc;
        let action = command(calc, line).unwrap_or_else(|| match evaluate(calc, line) {
            Ok(shown) => Action::Print(paint(color, Style::Function.ansi(), &shown)),
            Err(diagnostic) => Action::Error(render_error(&diagnostic)),
        });
        match action {
            Action::Print(text) if text.is_empty() => {}
            Action::Print(text) => out!("{text}"),
            Action::Error(text) => out!("{}", paint(color, "\x1b[31m", &text)),
            Action::Clear => {
                if color {
                    print!("\x1b[2J\x1b[1;1H");
                    let _ = std::io::stdout().flush();
                }
            }
            Action::Quit => break,
        }
    }
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn print(action: Option<Action>) -> String {
        match action {
            Some(Action::Print(s)) => s,
            other => panic!("expected output, got {other:?}"),
        }
    }

    #[test]
    fn set_changes_engine_options() {
        let mut calc = Abacus::standard();
        assert_eq!(print(command(&mut calc, ":set angle degrees")), "");
        assert_eq!(print(command(&mut calc, ":set decimals 3")), "");
        assert_eq!(evaluate(&mut calc, "sin(30)").unwrap(), "0.500");
        command(&mut calc, ":set decimals off");
        command(&mut calc, ":set notation sci");
        assert_eq!(evaluate(&mut calc, "12345").unwrap(), "1.2345e4");
        command(&mut calc, ":set exact on");
        assert!(!calc.snap_cancellation());
        let shown = print(command(&mut calc, ":set"));
        assert!(shown.contains("angle        degrees"), "{shown}");
        assert!(shown.contains("notation     scientific"), "{shown}");
        assert!(shown.contains("exact        on"), "{shown}");
    }

    #[test]
    fn bad_commands_explain_themselves() {
        let mut calc = Abacus::standard();
        for (line, want) in [
            (
                ":set decimals x",
                "decimals expects a whole number, got `x`",
            ),
            (
                ":set angle turns",
                "angle expects radians, degrees or gradians",
            ),
            (":set strict maybe", "strict expects on or off"),
            (":set colour red", "unknown option `colour`"),
            (":set decimals", ":set decimals needs a value"),
            (":frobnicate", "unknown command `:frobnicate`"),
            (":unset nope", "no variable named nope"),
        ] {
            match command(&mut calc, line) {
                Some(Action::Error(message)) => {
                    assert!(message.starts_with(want), "{line}: {message}")
                }
                other => panic!("{line}: {other:?}"),
            }
        }
    }

    #[test]
    fn variables_and_ans() {
        let mut calc = Abacus::standard();
        assert_eq!(
            print(command(&mut calc, ":vars")),
            "  (no variables; define one with `name = expr`)"
        );
        calc.set_decimal_places(2);
        assert_eq!(evaluate(&mut calc, "x = 10 / 3").unwrap(), "3.33");
        // `ans` keeps full precision, like the variable itself.
        assert_eq!(evaluate(&mut calc, "ans * 3").unwrap(), "10.00");
        assert_eq!(evaluate(&mut calc, "90 min").unwrap(), "1 hour, 30 minutes");
        let vars = print(command(&mut calc, ":vars"));
        assert!(vars.starts_with("  ans = 90.00 min\n  x = 3.33"), "{vars}");
        assert!(!vars.contains("pi"));
        assert!(print(command(&mut calc, ":vars all")).contains("pi = 3.14"));
        command(&mut calc, ":unset x ans");
        assert!(calc.get_variable("x").is_none());
        // Bare aliases with arguments are expressions, so `exit = 5` assigns.
        assert_eq!(command(&mut calc, "exit = 5"), None);
        assert_eq!(command(&mut calc, "quit"), Some(Action::Quit));
    }

    #[test]
    fn errors_point_under_the_input() {
        let mut calc = Abacus::standard();
        let err = evaluate(&mut calc, "5 m + 3 s").unwrap_err();
        assert_eq!(
            render_error(&err),
            "      ^\nerror: cannot perform operation on incompatible dimensions\nnote: m + s"
        );
        // Spans in an assignment's right-hand side are relative to the whole line.
        let err = evaluate(&mut calc, "y = 2 * foo(1)").unwrap_err();
        assert_eq!(render_error(&err).lines().next(), Some("          ^^^"));
        // Multi-byte characters before the problem count as one column each.
        let err = evaluate(&mut calc, "€5 + # 2").unwrap_err();
        assert_eq!(render_error(&err).lines().next(), Some("       ^"));
    }

    fn styled(calc: &Abacus, line: &str) -> Vec<(String, Style)> {
        styles(calc, line)
            .into_iter()
            .map(|(span, style)| (span.text(line).to_string(), style))
            .collect()
    }

    #[test]
    fn highlighting_follows_the_engine() {
        use Style::*;
        let mut calc = Abacus::standard();
        let s = |calc: &Abacus, line| styled(calc, line);
        assert_eq!(
            s(&calc, "2 sqrt(9 m^2) in ft"),
            [
                ("2".into(), Number),
                ("sqrt".into(), Function),
                ("(".into(), Operator),
                ("9".into(), Number),
                ("m^2".into(), Unit),
                (")".into(), Operator),
                ("in".into(), Keyword),
                ("ft".into(), Unit),
            ]
        );
        // `h` is the hour until a variable shadows it.
        assert_eq!(s(&calc, "h")[0].1, Unit);
        calc.set_variable("h", 2.0);
        assert_eq!(s(&calc, "h")[0].1, Variable);
        // Unknown names are marked; what came before is still coloured.
        assert_eq!(
            s(&calc, "1 + foo"),
            [
                ("1".into(), Number),
                ("+".into(), Operator),
                ("foo".into(), Error)
            ]
        );
        assert_eq!(
            s(&calc, "x = 5 kg"),
            [
                ("x".into(), Variable),
                ("=".into(), Operator),
                ("5".into(), Number),
                ("kg".into(), Unit),
            ]
        );
        assert_eq!(s(&calc, ":set angle deg"), [(":set".into(), Command)]);
        // Highlighting never changes the text.
        let line = "$50 + €20 in USD after 3 days ago, é";
        let plain: String = {
            let h = highlight(&calc, line);
            let mut out = String::new();
            let mut chars = h.chars();
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    for c in chars.by_ref() {
                        if c == 'm' {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                }
            }
            out
        };
        assert_eq!(plain, line);
    }

    #[test]
    fn completion_offers_engine_names() {
        let calc = Abacus::standard().with_variable("speed", 3.0);
        let (start, names) = completions(&calc, "2 * sq", 6);
        assert_eq!(start, 4);
        assert!(names.contains(&"sqrt".to_string()), "{names:?}");
        let (_, names) = completions(&calc, "spe", 3);
        assert_eq!(names.first().map(String::as_str), Some("speed"));
        let (_, names) = completions(&calc, "1 kilo", 6);
        assert!(names.iter().any(|n| n == "kilogram"), "{names:?}");
        let (start, names) = completions(&calc, ":se", 3);
        assert_eq!((start, names), (0, vec![":set".to_string()]));
        assert!(completions(&calc, "1 + ", 4).1.is_empty());
    }
}
