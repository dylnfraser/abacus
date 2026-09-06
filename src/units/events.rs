use crate::error::AbacusError;
use crate::units::date::{Date, DayOfWeek, days_in_month};

/// Computes the nth occurrence of a weekday in a given year and month.
///
/// `n` must be 1, 2, 3, 4, 5, or -1 (representing "last").
pub fn nth_weekday_of_month(
    year: i32,
    month: u32,
    dow: DayOfWeek,
    n: i8,
) -> Result<Date, AbacusError> {
    if !(1..=12).contains(&month) {
        return Err(AbacusError::InvalidDate(format!("invalid month: {month}")));
    }
    if n == 0 || !(-1..=5).contains(&n) {
        return Err(AbacusError::InvalidDate(format!(
            "invalid weekday ordinal: {n}"
        )));
    }

    let target_dow_num = dow as u32;

    if n > 0 {
        let first_dow = Date::new(year, month, 1).day_of_week() as u32;
        let offset = (target_dow_num as i32 - first_dow as i32).rem_euclid(7) as u32;
        let day = 1 + offset + ((n as u32) - 1) * 7;
        if day > days_in_month(year, month) {
            return Err(AbacusError::InvalidDate(format!(
                "{n}th {dow} does not exist in month {month} of year {year}"
            )));
        }
        Ok(Date::new(year, month, day))
    } else {
        // n == -1 => last occurrence in the month
        let total_days = days_in_month(year, month);
        let last_dow = Date::new(year, month, total_days).day_of_week() as u32;
        let offset = (last_dow as i32 - target_dow_num as i32).rem_euclid(7) as u32;
        let day = total_days - offset;
        Ok(Date::new(year, month, day))
    }
}

/// Returns the calendar quarter index (1, 2, 3, or 4) for a given date.
#[must_use]
pub fn quarter_of(date: &Date) -> u32 {
    match date.month {
        1..=3 => 1,
        4..=6 => 2,
        7..=9 => 3,
        _ => 4,
    }
}

/// Returns the start date of a given calendar quarter.
#[must_use]
pub fn quarter_start_date(quarter: u32, year: i32) -> Date {
    match quarter {
        1 => Date::new(year, 1, 1),
        2 => Date::new(year, 4, 1),
        3 => Date::new(year, 7, 1),
        _ => Date::new(year, 10, 1),
    }
}

/// Returns the end date of a given calendar quarter.
#[must_use]
pub fn quarter_end_date(quarter: u32, year: i32) -> Date {
    match quarter {
        1 => Date::new(year, 3, 31),
        2 => Date::new(year, 6, 30),
        3 => Date::new(year, 9, 30),
        _ => Date::new(year, 12, 31),
    }
}

/// Returns the end date of the current quarter relative to `ref_date`.
#[must_use]
pub fn end_of_quarter(ref_date: &Date) -> Date {
    quarter_end_date(quarter_of(ref_date), ref_date.year)
}

/// Returns the start date of the current quarter relative to `ref_date`.
#[must_use]
pub fn start_of_quarter(ref_date: &Date) -> Date {
    quarter_start_date(quarter_of(ref_date), ref_date.year)
}

/// Returns the end date of the next quarter relative to `ref_date`.
#[must_use]
pub fn end_of_next_quarter(ref_date: &Date) -> Date {
    let q = quarter_of(ref_date);
    if q == 4 {
        quarter_end_date(1, ref_date.year + 1)
    } else {
        quarter_end_date(q + 1, ref_date.year)
    }
}

/// Returns the start date of the next quarter relative to `ref_date`.
#[must_use]
pub fn start_of_next_quarter(ref_date: &Date) -> Date {
    let q = quarter_of(ref_date);
    if q == 4 {
        quarter_start_date(1, ref_date.year + 1)
    } else {
        quarter_start_date(q + 1, ref_date.year)
    }
}

/// Returns the end date of the month relative to `ref_date`.
#[must_use]
pub fn end_of_month(ref_date: &Date) -> Date {
    Date::new(
        ref_date.year,
        ref_date.month,
        days_in_month(ref_date.year, ref_date.month),
    )
}

/// Returns the start date of the month relative to `ref_date`.
#[must_use]
pub fn start_of_month(ref_date: &Date) -> Date {
    Date::new(ref_date.year, ref_date.month, 1)
}

/// Returns the end date of the next month relative to `ref_date`.
#[must_use]
pub fn end_of_next_month(ref_date: &Date) -> Date {
    let (year, month) = if ref_date.month == 12 {
        (ref_date.year + 1, 1)
    } else {
        (ref_date.year, ref_date.month + 1)
    };
    Date::new(year, month, days_in_month(year, month))
}

/// Returns the end date of the year relative to `ref_date`.
#[must_use]
pub fn end_of_year(ref_date: &Date) -> Date {
    Date::new(ref_date.year, 12, 31)
}

/// Returns the start date of the year relative to `ref_date`.
#[must_use]
pub fn start_of_year(ref_date: &Date) -> Date {
    Date::new(ref_date.year, 1, 1)
}

/// Returns the start date of the next year relative to `ref_date`.
#[must_use]
pub fn start_of_next_year(ref_date: &Date) -> Date {
    Date::new(ref_date.year + 1, 1, 1)
}

// ─────────────────────────────────────────────────────────────
// Named Holidays & Annual Events
// ─────────────────────────────────────────────────────────────

#[must_use]
pub fn christmas(year: i32) -> Date {
    Date::new(year, 12, 25)
}

#[must_use]
pub fn christmas_eve(year: i32) -> Date {
    Date::new(year, 12, 24)
}

#[must_use]
pub fn boxing_day(year: i32) -> Date {
    Date::new(year, 12, 26)
}

#[must_use]
pub fn new_year(year: i32) -> Date {
    Date::new(year, 1, 1)
}

#[must_use]
pub fn new_years_eve(year: i32) -> Date {
    Date::new(year, 12, 31)
}

#[must_use]
pub fn thanksgiving(year: i32) -> Date {
    nth_weekday_of_month(year, 11, DayOfWeek::Thursday, 4).unwrap_or(Date::new(year, 11, 26))
}

#[must_use]
pub fn black_friday(year: i32) -> Date {
    thanksgiving(year).add_days(1)
}

#[must_use]
pub fn cyber_monday(year: i32) -> Date {
    thanksgiving(year).add_days(4)
}

#[must_use]
pub fn halloween(year: i32) -> Date {
    Date::new(year, 10, 31)
}

#[must_use]
pub fn valentines_day(year: i32) -> Date {
    Date::new(year, 2, 14)
}

#[must_use]
pub fn st_patricks_day(year: i32) -> Date {
    Date::new(year, 3, 17)
}

#[must_use]
pub fn fourth_of_july(year: i32) -> Date {
    Date::new(year, 7, 4)
}

#[must_use]
pub fn labor_day(year: i32) -> Date {
    nth_weekday_of_month(year, 9, DayOfWeek::Monday, 1).unwrap_or(Date::new(year, 9, 1))
}

#[must_use]
pub fn memorial_day(year: i32) -> Date {
    nth_weekday_of_month(year, 5, DayOfWeek::Monday, -1).unwrap_or(Date::new(year, 5, 25))
}

#[must_use]
pub fn mlk_day(year: i32) -> Date {
    nth_weekday_of_month(year, 1, DayOfWeek::Monday, 3).unwrap_or(Date::new(year, 1, 15))
}

#[must_use]
pub fn presidents_day(year: i32) -> Date {
    nth_weekday_of_month(year, 2, DayOfWeek::Monday, 3).unwrap_or(Date::new(year, 2, 15))
}

#[must_use]
pub fn juneteenth(year: i32) -> Date {
    Date::new(year, 6, 19)
}

#[must_use]
pub fn veterans_day(year: i32) -> Date {
    Date::new(year, 11, 11)
}

/// Anonymous Gregorian Computus algorithm for Easter Sunday.
#[must_use]
pub fn easter(year: i32) -> Date {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    Date::new(year, month as u32, day as u32)
}
