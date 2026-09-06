use abacus::Abacus;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, Editor, Helper};
use std::borrow::Cow;
use std::env;
use std::io::Write;
use std::path::PathBuf;

struct AbacusHelper;

impl Completer for AbacusHelper {
    type Candidate = String;
}

impl Hinter for AbacusHelper {
    type Hint = String;
}

impl Validator for AbacusHelper {}

impl Helper for AbacusHelper {}

impl Highlighter for AbacusHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        highlight_syntax(line).into()
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _kind: rustyline::highlight::CmdKind,
    ) -> bool {
        true
    }
}

fn highlight_syntax(line: &str) -> String {
    let mut result = String::new();
    let mut chars = line.char_indices().peekable();

    while let Some(&(i, ch)) = chars.peek() {
        if ch.is_ascii_digit() {
            let start = i;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_digit() || c == '.' || c == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map_or(line.len(), |&(idx, _)| idx);
            let num = &line[start..end];
            // Numbers in Yellow/Gold
            result.push_str("\x1b[38;2;241;196;15m");
            result.push_str(num);
            result.push_str("\x1b[0m");
        } else if ch.is_alphabetic() || ch == '_' || ch == '.' {
            let start = i;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map_or(line.len(), |&(idx, _)| idx);
            let word = &line[start..end];

            if word.starts_with('.') {
                // Command like .help or .clear
                result.push_str("\x1b[38;2;189;147;249m");
                result.push_str(word);
                result.push_str("\x1b[0m");
            } else {
                match word.to_lowercase().as_str() {
                    // Functions & constants
                    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sqrt" | "log" | "ln"
                    | "exp" | "abs" | "ceil" | "floor" | "round" | "min" | "max" | "sum"
                    | "avg" | "mean" | "std" | "e" | "pi" => {
                        result.push_str("\x1b[38;2;139;233;253m"); // Lime Green Function
                        result.push_str(word);
                        result.push_str("\x1b[0m");
                    }
                    // Date & Relative Date keywords
                    "last" | "next" | "this" | "today" | "tomorrow" | "yesterday" | "monday"
                    | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
                    | "january" | "february" | "march" | "april" | "may" | "june" | "july"
                    | "august" | "september" | "october" | "november" | "december" | "pm"
                    | "am" => {
                        result.push_str("\x1b[38;2;80;250;123m"); // Teal Date Keyword
                        result.push_str(word);
                        result.push_str("\x1b[0m");
                    }
                    // Units & Conversion keywords
                    "meters" | "meter" | "m" | "inches" | "inch" | "in" | "feet" | "ft" | "cm"
                    | "mm" | "km" | "miles" | "mi" | "kg" | "g" | "lbs" | "to" | "as" | "at"
                    | "per" | "of" | "hours" | "hour" | "h" | "minutes" | "mins" | "seconds"
                    | "s" | "days" | "day" | "weeks" | "week" | "months" | "years" | "workdays"
                    | "business" => {
                        result.push_str("\x1b[38;2;189;147;249m"); // Soft Purple Unit
                        result.push_str(word);
                        result.push_str("\x1b[0m");
                    }
                    _ => {
                        result.push_str(word);
                    }
                }
            }
        } else if matches!(ch, '$' | '€' | '£' | '¥' | '₹' | '₩' | '₺' | '₪' | '฿') {
            chars.next();
            result.push_str("\x1b[38;2;189;147;249m"); // Soft Purple Currency
            result.push(ch);
            result.push_str("\x1b[0m");
        } else if "+-*/^!()[]{},".contains(ch) {
            chars.next();
            result.push_str("\x1b[38;2;139;233;253m"); // Lime Green Operator
            result.push(ch);
            result.push_str("\x1b[0m");
        } else {
            chars.next();
            result.push(ch);
        }
    }

    result
}

use std::io::IsTerminal;

fn get_history_path() -> Option<PathBuf> {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .or_else(|_| env::var("APPDATA"))
        .ok()
        .map(|h| PathBuf::from(h).join(".abacus_history"))
}

fn print_welcome_banner(use_color: bool) {
    if use_color {
        println!("\x1b[1;36m=============================================================\x1b[0m");
        println!("               \x1b[1;33mABACUS INTERACTIVE SHELL (1.0)\x1b[0m               ");
        println!(" \x1b[1;34mType physical math expressions, date calculations, etc.\x1b[0m   ");
        println!(" \x1b[90mUse ↑/↓ Arrow Keys to navigate command history.\x1b[0m            ");
        println!(" \x1b[1;33mExamples:\x1b[0m                                                 ");
        println!(
            "    • \x1b[38;2;241;196;15m14 meters to inches\x1b[0m                               "
        );
        println!(
            "    • \x1b[38;2;139;233;253msin(30deg)\x1b[0m                                          "
        );
        println!(
            "    • \x1b[38;2;80;250;123mlast thursday at 3pm\x1b[0m                                 "
        );
        println!(
            "    • \x1b[38;2;139;233;253msqrt(14 m^3)\x1b[0m                                       "
        );
        println!(" \x1b[90mType '.help' for help, '.clear' to clear, 'exit' to leave.\x1b[0m");
        println!(
            "\x1b[1;36m=============================================================\x1b[0m\n"
        );
    } else {
        println!("=============================================================");
        println!("               ABACUS INTERACTIVE SHELL (1.0)               ");
        println!(" Type physical math expressions, date calculations, etc.   ");
        println!(" Use ↑/↓ Arrow Keys to navigate command history.            ");
        println!(" Examples:                                                 ");
        println!("    • 14 meters to inches                               ");
        println!("    • sin(30deg)                                          ");
        println!("    • last thursday at 3pm                                 ");
        println!("    • sqrt(14 m^3)                                       ");
        println!(" Type '.help' for help, '.clear' to clear, 'exit' to leave.");
        println!("=============================================================\n");
    }
}

use rustyline::ColorMode;

fn main() {
    let use_color = std::io::stdout().is_terminal();
    let calc = Abacus::standard();
    let config = Config::builder()
        .color_mode(if use_color {
            ColorMode::Forced
        } else {
            ColorMode::Disabled
        })
        .build();
    let mut rl = match Editor::with_config(config) {
        Ok(rl) => rl,
        Err(e) => {
            eprintln!("Failed to initialize interactive editor: {e}");
            return;
        }
    };
    rl.set_helper(Some(AbacusHelper));

    let history_path = get_history_path();
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

    print_welcome_banner(use_color);

    let prompt = if use_color {
        "\x1b[36m›\x1b[0m "
    } else {
        "› "
    };

    loop {
        let readline = rl.readline(prompt);

        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);
                if let Some(ref path) = history_path {
                    let _ = rl.save_history(path);
                }

                if trimmed.eq_ignore_ascii_case("exit")
                    || trimmed.eq_ignore_ascii_case("quit")
                    || trimmed == ":q"
                {
                    if use_color {
                        println!("\x1b[33mGoodbye!\x1b[0m");
                    } else {
                        println!("Goodbye!");
                    }
                    break;
                }

                if trimmed == "clear" {
                    if use_color {
                        print!("\x1b[2J\x1b[1;1H");
                        let _ = std::io::stdout().flush();
                    }
                    print_welcome_banner(use_color);
                    continue;
                }

                if trimmed == "help" {
                    if use_color {
                        println!("\x1b[38;2;189;147;249m  Abacus REPL Commands & Syntax:\x1b[0m");
                    } else {
                        println!("  Abacus REPL Commands & Syntax:");
                    }
                    println!("    help       Show this help summary");
                    println!("    clear      Clear the terminal screen");
                    println!("    :q / exit  Exit the REPL\n");
                    continue;
                }

                match calc.eval(trimmed) {
                    Ok(result) => {
                        let formatted = format_repl_result(&result);
                        if use_color {
                            println!("\x1b[38;2;139;233;253m{formatted}\x1b[0m");
                        } else {
                            println!("{formatted}");
                        }
                    }
                    Err(err) => {
                        if use_color {
                            println!("\x1b[31mError: {err}\x1b[0m");
                        } else {
                            println!("Error: {err}");
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                if use_color {
                    println!("\x1b[33mCTRL-C\x1b[0m");
                } else {
                    println!("CTRL-C");
                }
                break;
            }
            Err(ReadlineError::Eof) => {
                if use_color {
                    println!("\x1b[33mGoodbye!\x1b[0m");
                } else {
                    println!("Goodbye!");
                }
                break;
            }
            Err(err) => {
                if use_color {
                    println!("\x1b[31mError: {err:?}\x1b[0m");
                } else {
                    println!("Error: {err:?}");
                }
                break;
            }
        }
    }
}

fn format_repl_result(result: &abacus::units::eval_result::EvalResult) -> String {
    match result {
        abacus::units::eval_result::EvalResult::Scalar(v) => {
            if v.unit.is_standard_duration_unit() {
                abacus::units::value::format_human_duration(v.canonical)
            } else {
                v.to_display()
            }
        }
        other => other.to_display(),
    }
}
