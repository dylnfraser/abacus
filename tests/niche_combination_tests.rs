// Helpers and imports are only used by tests gated on default features.
#![cfg_attr(
    not(all(
        feature = "units",
        feature = "stats",
        feature = "distributions",
        feature = "date",
        feature = "financial",
        feature = "number-scales",
        feature = "currencies"
    )),
    allow(unused)
)]
use abacus::{Abacus, Date};

#[cfg(all(feature = "units", feature = "number-scales", feature = "currencies"))]
#[test]
fn test_event_calculations_with_financial_rates_and_scales() {
    let anchor = Date::new(2026, 9, 1);
    let calc = Abacus::standard().with_now(anchor);

    // ($100 per day) * (days until christmas) -> 115 days * $100/d = $11500
    let res1 = calc
        .eval("($100 per day) * (days until christmas)")
        .unwrap();
    assert_eq!(res1.to_display(), "$11500");

    // ($500 per business day) * (business days until end of quarter)
    // 21 bdays * $500/bday = $10500
    let res2 = calc
        .eval("($500 per business_days) * (business days until end of quarter)")
        .unwrap();
    assert_eq!(res2.to_display(), "$10500");

    // ($500 per bday) * (bdays until end of quarter)
    let res2_b = calc
        .eval("($500 per bday) * (bdays until end of quarter)")
        .unwrap();
    assert_eq!(res2_b.to_display(), "$10500");

    // (a thousand dollars per day) * (days until end of quarter)
    // 29 days * $1000/d = $29000
    let res3 = calc
        .eval("(a thousand dollars per day) * (days until end of quarter)")
        .unwrap();
    assert_eq!(res3.to_display(), "$29000");

    // 20% off (($100 per day) * (days until end of quarter))
    // 29 days * $100 = $2900 -> 20% off is $2320
    let res4 = calc
        .eval("20% off (($100 per day) * (days until end of quarter))")
        .unwrap();
    assert_eq!(res4.to_display(), "$2320");
}

#[cfg(all(feature = "units", feature = "number-scales", feature = "currencies"))]
#[test]
fn test_conversational_percentages_with_number_scales() {
    let calc = Abacus::standard();

    // 15% off $1.2 million -> $1020000
    let res1 = calc.eval("15% off $1.2 million").unwrap();
    assert_eq!(res1.to_display(), "$1020000");

    // $2.5 million after 10% tax -> $2750000
    let res2 = calc.eval("$2.5 million after 10% tax").unwrap();
    assert_eq!(res2.to_display(), "$2750000");

    // 500 thousand out of 2 million as % -> 25%
    let res3 = calc.eval("500 thousand out of 2 million as %").unwrap();
    assert_eq!(res3.to_display(), "25%");

    // % change from $2 million to $2.5 million -> +25%
    let res4 = calc
        .eval("% change from $2 million to $2.5 million")
        .unwrap();
    assert_eq!(res4.to_display(), "+25%");

    // 10% more than $5 million -> $5500000
    let res5 = calc.eval("10% more than $5 million").unwrap();
    assert_eq!(res5.to_display(), "$5500000");

    // 25% less than a billion dollars -> $750000000
    let res6 = calc.eval("25% less than a billion dollars").unwrap();
    assert_eq!(res6.to_display(), "$750000000");

    // Combining two percentage changes
    // (% change from 100 to 150) + (% change from 50 to 75) = 50% + 50% = 100%
    let res7 = calc
        .eval("(% change from 100 to 150) + (% change from 50 to 75)")
        .unwrap();
    assert_eq!(res7.to_display(), "+100%");
}

#[cfg(feature = "units")]
#[test]
fn test_ordinal_weekdays_edge_cases_and_leap_years() {
    let calc = Abacus::standard();

    // 2028 is a leap year (29 days). Feb 1, 2028 is Tuesday.
    // Tuesdays in Feb 2028: 1, 8, 15, 22, 29. The 5th Tuesday exists!
    let d_leap = calc.eval_date("5th tuesday in february 2028").unwrap();
    assert_eq!(d_leap.year, 2028);
    assert_eq!(d_leap.month, 2);
    assert_eq!(d_leap.day, 29);

    // In 2026 (non-leap year, 28 days), 5th Tuesday does NOT exist.
    let d_non_leap = calc.eval_date("5th tuesday in february 2026");
    assert!(d_non_leap.is_err());

    // Last Sunday of February 2028 (leap year) -> 2028-02-27
    let d_last_sun_leap = calc.eval_date("last sunday of february 2028").unwrap();
    assert_eq!(d_last_sun_leap.year, 2028);
    assert_eq!(d_last_sun_leap.month, 2);
    assert_eq!(d_last_sun_leap.day, 27);

    // Last Sunday of February 2026 (non-leap year) -> 2026-02-22
    let d_last_sun_non_leap = calc.eval_date("last sunday of february 2026").unwrap();
    assert_eq!(d_last_sun_non_leap.year, 2026);
    assert_eq!(d_last_sun_non_leap.month, 2);
    assert_eq!(d_last_sun_non_leap.day, 22);

    // Explicit Quarter boundary dates
    let q4_end = calc.eval_date("end of q4 2026").unwrap();
    assert_eq!(q4_end.year, 2026);
    assert_eq!(q4_end.month, 12);
    assert_eq!(q4_end.day, 31);

    let q1_start = calc.eval_date("start of q1 2027").unwrap();
    assert_eq!(q1_start.year, 2027);
    assert_eq!(q1_start.month, 1);
    assert_eq!(q1_start.day, 1);

    // Q4 cross-year quarter rollover:
    // Anchor in Q4 (Nov 15, 2026):
    let q4_anchor = Date::new(2026, 11, 15);
    let q4_calc = Abacus::standard().with_now(q4_anchor);

    // "end of quarter" -> 2026-12-31
    let q4_cur_end = q4_calc.eval_date("end of quarter").unwrap();
    assert_eq!(q4_cur_end.year, 2026);
    assert_eq!(q4_cur_end.month, 12);
    assert_eq!(q4_cur_end.day, 31);

    // "end of next quarter" -> rolls into next year Q1: 2027-03-31
    let q4_next_end = q4_calc.eval_date("end of next quarter").unwrap();
    assert_eq!(q4_next_end.year, 2027);
    assert_eq!(q4_next_end.month, 3);
    assert_eq!(q4_next_end.day, 31);

    // "start of next quarter" -> 2027-01-01
    let q4_next_start = q4_calc.eval_date("start of next quarter").unwrap();
    assert_eq!(q4_next_start.year, 2027);
    assert_eq!(q4_next_start.month, 1);
    assert_eq!(q4_next_start.day, 1);

    // Days until end of year from Sept 1, 2026:
    // 29 (Sept) + 31 (Oct) + 30 (Nov) + 31 (Dec) = 121 days
    let sept_anchor = Date::new(2026, 9, 1);
    let sept_calc = Abacus::standard().with_now(sept_anchor);
    let res_eoy = sept_calc.eval("days until end of year").unwrap();
    assert_eq!(res_eoy.to_display(), "121 d");
}

#[cfg(feature = "units")]
#[test]
fn test_until_and_till_edge_cases() {
    let anchor = Date::new(2026, 9, 1);
    let calc = Abacus::standard().with_now(anchor);

    // "till" as synonym for "until"
    let res_till_1 = calc.eval("days till christmas").unwrap();
    assert_eq!(res_till_1.to_display(), "115 d");

    let res_till_2 = calc.eval("business days till end of quarter").unwrap();
    assert_eq!(res_till_2.to_display(), "21 business_days");

    let res_till_3 = calc.eval("bdays till end of quarter").unwrap();
    assert_eq!(res_till_3.to_display(), "21 bdays");

    // Negative countdown when target is in the past
    // Anchor: 2026-09-01, Target: 2026-08-01 -> -31 days
    let res_past = calc.eval("days until 2026-08-01").unwrap();
    assert_eq!(res_past.to_display(), "-31 d");

    // Zero countdown when target is today
    let res_same_day = calc.eval("days until 2026-09-01").unwrap();
    assert_eq!(res_same_day.to_display(), "0 d");

    // Explicit Date until Date
    let res_date_to_date = calc.eval("2026-09-01 until 2026-09-21").unwrap();
    assert_eq!(res_date_to_date.to_display(), "20 d");

    let res_date_to_event = calc.eval("2026-09-01 until christmas 2026").unwrap();
    assert_eq!(res_date_to_event.to_display(), "115 d");

    // Countdown unit conversions
    // 115 days in hours -> 115 * 24 = 2760 h
    let res_hours = calc.eval("(days until christmas) in hours").unwrap();
    assert_eq!(res_hours.to_display(), "2760 h");

    // 115 days in minutes -> 115 * 24 * 60 = 165600 min
    let res_mins = calc.eval("(days until christmas) in minutes").unwrap();
    assert_eq!(res_mins.to_display(), "165600 min");
}

#[cfg(all(feature = "units", feature = "currencies"))]
#[test]
fn test_speed_overrides_and_rate_chains() {
    let calc = Abacus::standard();

    // 60 mph * 2 hours in km -> 193.12128 km
    let res1 = calc.eval("(60 mph * 2 hours) in km").unwrap();
    assert_eq!(res1.to_display(), "193.12128 km");

    // 100 kmph * 1.5 hours in miles
    let res2 = calc.eval("(100 kmph * 1.5 hours) in miles").unwrap();
    assert_eq!(res2.to_display(), "93.2056788356001 mi");

    // 100 kmph to mph
    let res3 = calc.eval("100 kmph to mph").unwrap();
    assert_eq!(res3.to_display(), "62.1371192237334 mph");

    // 50 usd per second in 1 hour -> 50 * 3600 = $180000
    let res4 = calc.eval("50 usd per second in 1 hour").unwrap();
    assert_eq!(res4.to_display(), "$180000");

    // 10 usd a second in 2 minutes -> 10 * 120 = $1200
    let res5 = calc.eval("10 usd a second in 2 minutes").unwrap();
    assert_eq!(res5.to_display(), "$1200");

    // 50 usd an hour in 1 day (24 hours) -> 50 * 24 = $1200
    let res6 = calc.eval("50 usd an hour in 1 day").unwrap();
    assert_eq!(res6.to_display(), "$1200");
}
