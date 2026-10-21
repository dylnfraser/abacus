use crate::{
    AbacusError, Date, EvalResult, Value,
    evaluation::tokenizer::registry::function::operators::FunctionOp,
};

/// Extracts a dimensionless whole number within `range` for argument `name` of `date()`.
fn integer_arg(
    v: &Value,
    name: &str,
    range: std::ops::RangeInclusive<i64>,
) -> Result<i64, AbacusError> {
    let x = v.canonical;
    if !v.unit.is_dimensionless() || !x.is_finite() || x.fract() != 0.0 {
        return Err(AbacusError::InvalidDate(format!(
            "date(): {name} must be a whole number, got {x}"
        )));
    }
    let n = x as i64;
    if !range.contains(&n) {
        return Err(AbacusError::InvalidDate(format!(
            "date(): {name} {n} is out of range {}..={}",
            range.start(),
            range.end()
        )));
    }
    Ok(n)
}

/// `date(year, month, day[, hour, minute, second])`. All arguments must be whole numbers in range
/// (years -9999..=9999) and form a valid calendar date.
fn date_fn(args: &[Value]) -> Result<EvalResult, AbacusError> {
    if args.len() != 3 && args.len() != 6 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let year = integer_arg(&args[0], "year", -9999..=9999)? as i32;
    let month = integer_arg(&args[1], "month", 1..=12)? as u32;
    let day = integer_arg(&args[2], "day", 1..=31)? as u32;
    let d = if args.len() == 6 {
        let hour = integer_arg(&args[3], "hour", 0..=23)? as u32;
        let minute = integer_arg(&args[4], "minute", 0..=59)? as u32;
        let second = integer_arg(&args[5], "second", 0..=59)? as u32;
        Date::new_with_hms(year, month, day, hour, minute, second)
    } else {
        Date::new(year, month, day)
    };
    if !d.is_valid() {
        return Err(AbacusError::InvalidDate(format!(
            "date(): {year:04}-{month:02}-{day:02} is not a valid calendar date"
        )));
    }
    Ok(EvalResult::Date(d))
}

fn date_prop_helper(args: &[EvalResult], prop: &str) -> Result<EvalResult, AbacusError> {
    if args.len() != 1 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    match &args[0] {
        EvalResult::Date(d) => {
            if let Some(val) = d.get_property(prop) {
                Ok(EvalResult::Scalar(Value::dimensionless(val)))
            } else {
                Err(AbacusError::IncompatibleFunctionArguments)
            }
        }
        _ => Err(AbacusError::IncompatibleFunctionArguments),
    }
}

fn year_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "year")
}
fn month_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "month")
}
fn day_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "day")
}
fn hour_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "hour")
}
fn minute_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "minute")
}
fn second_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "second")
}
fn millisecond_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "millisecond")
}
fn day_of_week_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "day_of_week")
}
fn day_of_year_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "day_of_year")
}
fn is_weekend_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "is_weekend")
}
fn is_workday_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    date_prop_helper(args, "is_workday")
}

fn workdays_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    if args.len() != 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    match (&args[0], &args[1]) {
        (EvalResult::Date(d1), EvalResult::Date(d2)) => {
            use crate::units::{
                dimensions::Dimensions,
                unit::{Unit, UnitExpr},
            };
            let bdays = d1.business_days_between(d2) as f64;
            let unit = std::sync::Arc::new(Unit {
                scalar: 86400.0,
                offset: 0.0,
                dimensions: Dimensions::TIME,
                display: UnitExpr::single("workdays"),
            });
            Ok(EvalResult::Scalar(Value::new(bdays, unit)))
        }
        _ => Err(AbacusError::IncompatibleFunctionArguments),
    }
}

/// Maps a style name (`YYYY-MM-DD`, `iso`, `ymd`, `DD-MM-YYYY`, `dmy`, `MM-DD-YYYY`, `mdy`,
/// `long`) to a [`DateFormat`](crate::units::date::DateFormat).
fn parse_format_style(spec: &str) -> Option<crate::units::date::DateFormat> {
    use crate::units::date::DateFormat;
    let normalized: String = spec
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    match normalized.as_str() {
        "YYYYMMDD" | "ISO" | "YMD" => Some(DateFormat::YYYYMMDD),
        "DDMMYYYY" | "DMY" => Some(DateFormat::DDMMYYYY),
        "MMDDYYYY" | "MDY" => Some(DateFormat::MMDDYYYY),
        "LONG" | "MONTHDAYYEAR" | "MMMMDYYYY" => Some(DateFormat::MonthDayYear),
        _ => None,
    }
}

fn with_style(d: &Date, style: crate::units::date::DateFormat) -> EvalResult {
    let mut formatted_d = d.clone();
    formatted_d.format = style;
    formatted_d.flags |= crate::units::date::FLAG_EXPLICIT_FORMAT;
    EvalResult::Date(formatted_d)
}

/// `format_date(date[, style])`: with no style the default (`MonthDayYear`) format is used.
fn format_date_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    if args.is_empty() || args.len() > 2 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    let EvalResult::Date(d) = &args[0] else {
        return Err(AbacusError::IncompatibleFunctionArguments);
    };
    let style = if args.len() == 2 {
        let spec = match &args[1] {
            EvalResult::Scalar(v) => v.to_units_display(),
            other => other.to_display(),
        };
        parse_format_style(&spec).ok_or(AbacusError::IncompatibleFunctionArguments)?
    } else {
        crate::units::date::DateFormat::default()
    };
    Ok(with_style(d, style))
}

fn style_fn(
    args: &[EvalResult],
    style: crate::units::date::DateFormat,
) -> Result<EvalResult, AbacusError> {
    match args {
        [EvalResult::Date(d)] => Ok(with_style(d, style)),
        _ => Err(AbacusError::IncompatibleFunctionArguments),
    }
}

fn format_iso_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    style_fn(args, crate::units::date::DateFormat::YYYYMMDD)
}
fn format_dmy_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    style_fn(args, crate::units::date::DateFormat::DDMMYYYY)
}
fn format_mdy_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    style_fn(args, crate::units::date::DateFormat::MMDDYYYY)
}
fn format_long_fn(args: &[EvalResult]) -> Result<EvalResult, AbacusError> {
    style_fn(args, crate::units::date::DateFormat::MonthDayYear)
}

pub fn register_date_functions() -> Vec<FunctionOp> {
    vec![
        FunctionOp::eval_result("date", 3, 6, date_fn),
        FunctionOp::general("year", 1, 1, year_fn),
        FunctionOp::general("month", 1, 1, month_fn),
        FunctionOp::general("day", 1, 1, day_fn),
        FunctionOp::general("hour", 1, 1, hour_fn),
        FunctionOp::general("minute", 1, 1, minute_fn),
        FunctionOp::general("second", 1, 1, second_fn),
        FunctionOp::general("millisecond", 1, 1, millisecond_fn),
        FunctionOp::general("day_of_week", 1, 1, day_of_week_fn),
        FunctionOp::general("weekday", 1, 1, day_of_week_fn),
        FunctionOp::general("day_of_year", 1, 1, day_of_year_fn),
        FunctionOp::general("is_weekend", 1, 1, is_weekend_fn),
        FunctionOp::general("is_workday", 1, 1, is_workday_fn),
        FunctionOp::general("is_business_day", 1, 1, is_workday_fn),
        FunctionOp::general("workdays", 2, 2, workdays_fn),
        FunctionOp::general("business_days", 2, 2, workdays_fn),
        FunctionOp::general("format_date", 1, 2, format_date_fn),
        FunctionOp::general("format_iso", 1, 1, format_iso_fn),
        FunctionOp::general("format_dmy", 1, 1, format_dmy_fn),
        FunctionOp::general("format_mdy", 1, 1, format_mdy_fn),
        FunctionOp::general("format_long", 1, 1, format_long_fn),
    ]
}
