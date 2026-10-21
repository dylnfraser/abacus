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

/// Fixed reference instant (Wednesday 2026-10-21 09:30, floating local time) so these tests never
/// depend on the wall clock.
fn anchor() -> Date {
    Date::new_full(2026, 10, 21, 9, 30, 0, 0)
}

fn anchored_calc() -> Abacus {
    Abacus::standard().with_now(anchor())
}

#[test]
fn test_relative_date_keywords() {
    let abacus = anchored_calc();

    let d_today = abacus.eval_date("today").unwrap();
    let expected_today = anchor().start_of_day();
    assert_eq!(d_today.year, expected_today.year);
    assert_eq!(d_today.month, expected_today.month);
    assert_eq!(d_today.day, expected_today.day);

    let d_tdy = abacus.eval_date("tdy").unwrap();
    assert_eq!(d_tdy.year, expected_today.year);
    assert_eq!(d_tdy.month, expected_today.month);
    assert_eq!(d_tdy.day, expected_today.day);

    let d_tomorrow = abacus.eval_date("tomorrow").unwrap();
    let expected_tomorrow = anchor().start_of_day().add_days(1);
    assert_eq!(d_tomorrow.day, expected_tomorrow.day);

    let d_tmr = abacus.eval_date("tmr").unwrap();
    assert_eq!(d_tmr.day, expected_tomorrow.day);

    let d_yesterday = abacus.eval_date("yesterday").unwrap();
    let expected_yesterday = anchor().start_of_day().add_days(-1);
    assert_eq!(d_yesterday.day, expected_yesterday.day);
}

#[test]
fn test_relative_date_with_at_time() {
    let abacus = anchored_calc();

    let d1 = abacus.eval_date("today at 12:00").unwrap();
    assert_eq!(d1.time.hour, 12);
    assert_eq!(d1.time.minute, 0);

    let d2 = abacus.eval_date("tomorrow at 3:30 PM").unwrap();
    assert_eq!(d2.time.hour, 15);
    assert_eq!(d2.time.minute, 30);
}

#[cfg(feature = "units")]
#[test]
fn test_relative_time_interval_today_at_12_to_1() {
    let abacus = anchored_calc();

    // today at 12:00 to 1:00 in hours -> 1 h (12:00 PM to 1:00 PM)
    let res = abacus.eval("today at 12:00 to 1:00 in hours").unwrap();
    assert_eq!(res.to_display(), "1 h");

    // today at 11:30 AM to 2:15 PM in hours -> 2.75 h
    let res2 = abacus
        .eval("today at 11:30 AM to 2:15 PM in hours")
        .unwrap();
    assert_eq!(res2.to_display(), "2.75 h");
}

#[cfg(feature = "units")]
#[test]
fn test_relative_time_interval_defaults_to_hours() {
    let abacus = anchored_calc();

    // today at 12:00 to 3:00 -> 3 h by default!
    let res = abacus.eval("today at 12:00 to 3:00").unwrap();
    assert_eq!(res.to_display(), "3 h");

    // Overridden with explicit unit
    let res_min = abacus.eval("today at 12:00 to 3:00 in minutes").unwrap();
    assert_eq!(res_min.to_display(), "180 min");
}

#[test]
fn test_last_thursday_at_3pm() {
    let abacus = anchored_calc();

    let d = abacus.eval_date("last thursday at 3pm").unwrap();
    assert_eq!(d.day_of_week(), abacus::DayOfWeek::Thursday);
    assert_eq!(d.time.hour, 15);
    assert_eq!(d.time.minute, 0);

    // Verify it is in the past relative to today
    let today = anchor().start_of_day();
    assert!(d.to_epoch_days() <= today.to_epoch_days());
    assert!((today.to_epoch_days() - d.to_epoch_days()) <= 7);
}

#[cfg(feature = "units")]
#[test]
fn test_relative_date_arithmetic_with_last_thursday() {
    let abacus = anchored_calc();

    // Adding days to "last thursday at 3pm"
    let d_plus = abacus.eval_date("last thursday at 3pm + 2 days").unwrap();
    let d_orig = abacus.eval_date("last thursday at 3pm").unwrap();
    assert_eq!(d_plus, d_orig.add_days(2));
    assert_eq!(d_plus.time.hour, 15);

    // Subtracting hours
    let d_sub = abacus.eval_date("last thursday at 3pm - 3 hours").unwrap();
    assert_eq!(d_sub.time.hour, 12);

    // Property access
    let hour = abacus.eval_scalar("last thursday at 3pm.hour").unwrap();
    assert_eq!(hour.canonical, 15.0);

    let dow = abacus
        .eval_scalar("last thursday at 3pm.day_of_week")
        .unwrap();
    assert_eq!(dow.canonical, 4.0); // Thursday = 4
}

#[test]
fn test_relative_weekday_modifiers() {
    let abacus = anchored_calc();
    let today = anchor().start_of_day();

    let d_next_mon = abacus.eval_date("next monday at 10am").unwrap();
    assert_eq!(d_next_mon.day_of_week(), abacus::DayOfWeek::Monday);
    assert_eq!(d_next_mon.time.hour, 10);
    assert!(d_next_mon.to_epoch_days() > today.to_epoch_days());

    let d_this_fri = abacus.eval_date("this friday at 4:30pm").unwrap();
    assert_eq!(d_this_fri.day_of_week(), abacus::DayOfWeek::Friday);
    assert_eq!(d_this_fri.time.hour, 16);
    assert_eq!(d_this_fri.time.minute, 30);

    let d_prev_tue = abacus.eval_date("previous tuesday at 14:00").unwrap();
    assert_eq!(d_prev_tue.day_of_week(), abacus::DayOfWeek::Tuesday);
    assert_eq!(d_prev_tue.time.hour, 14);
    assert!(d_prev_tue.to_epoch_days() <= today.to_epoch_days());
}

#[test]
fn test_standalone_time_literals() {
    let abacus = anchored_calc();

    let d1 = abacus.eval_date("3pm").unwrap();
    assert_eq!(d1.year, anchor().start_of_day().year);
    assert_eq!(d1.month, anchor().start_of_day().month);
    assert_eq!(d1.day, anchor().start_of_day().day);
    assert_eq!(d1.time.hour, 15);

    let d2 = abacus.eval_date("9:15 AM").unwrap();
    assert_eq!(d2.time.hour, 9);
    assert_eq!(d2.time.minute, 15);
}

#[cfg(feature = "units")]
#[test]
fn test_relative_time_ago_before_after_in() {
    let abacus = anchored_calc();
    let now = anchor();

    // 3 hours ago
    let d_ago = abacus.eval_date("3 hours ago").unwrap();
    let diff_ms = (now.to_epoch_milliseconds() - d_ago.to_epoch_milliseconds()).abs();
    assert!((diff_ms - 3 * 3_600_000).abs() < 5000);

    // 3 hours before last thursday at 3pm
    let d_before = abacus
        .eval_date("3 hours before last thursday at 3pm")
        .unwrap();
    let d_thu = abacus.eval_date("last thursday at 3pm").unwrap();
    assert_eq!(d_before, d_thu.add_hours(-3));
    assert_eq!(d_before.time.hour, 12);

    // 3 hours after last thursday at 3pm
    let d_after = abacus
        .eval_date("3 hours after last thursday at 3pm")
        .unwrap();
    assert_eq!(d_after, d_thu.add_hours(3));
    assert_eq!(d_after.time.hour, 18);

    // in 3 hours
    let d_in = abacus.eval_date("in 3 hours").unwrap();
    let diff_in_ms = (d_in.to_epoch_milliseconds() - now.to_epoch_milliseconds()).abs();
    assert!((diff_in_ms - 3 * 3_600_000).abs() < 5000);

    // 3 hours from now
    let d_from_now = abacus.eval_date("3 hours from now").unwrap();
    let diff_fn_ms = (d_from_now.to_epoch_milliseconds() - now.to_epoch_milliseconds()).abs();
    assert!((diff_fn_ms - 3 * 3_600_000).abs() < 5000);

    // 3 hours before
    let d_before_standalone = abacus.eval_date("3 hours before").unwrap();
    let diff_bs_ms =
        (now.to_epoch_milliseconds() - d_before_standalone.to_epoch_milliseconds()).abs();
    assert!((diff_bs_ms - 3 * 3_600_000).abs() < 5000);
}

#[cfg(feature = "units")]
#[test]
fn test_more_relative_time_expressions() {
    let abacus = anchored_calc();
    let now = anchor();

    // 5 minutes ago
    let d_5min = abacus.eval_date("5 minutes ago").unwrap();
    let diff_5min = (now.to_epoch_milliseconds() - d_5min.to_epoch_milliseconds()).abs();
    assert!((diff_5min - 5 * 60_000).abs() < 5000);

    // 2 days ago
    let d_2days = abacus.eval_date("2 days ago").unwrap();
    let diff_2days = (now.to_epoch_milliseconds() - d_2days.to_epoch_milliseconds()).abs();
    assert!((diff_2days - 2 * 86_400_000).abs() < 5000);

    // 1 week ago
    let d_1wk = abacus.eval_date("1 week ago").unwrap();
    let diff_1wk = (now.to_epoch_milliseconds() - d_1wk.to_epoch_milliseconds()).abs();
    assert!((diff_1wk - 7 * 86_400_000).abs() < 5000);

    // 30 seconds ago
    let d_30sec = abacus.eval_date("30 seconds ago").unwrap();
    let diff_30sec = (now.to_epoch_milliseconds() - d_30sec.to_epoch_milliseconds()).abs();
    assert!((diff_30sec - 30_000).abs() < 5000);

    // 10 minutes before 2026-08-07 15:30:00
    let d_exact = abacus
        .eval_date("10 minutes before 2026-08-07 15:30:00")
        .unwrap();
    assert_eq!(d_exact, Date::new_with_hms(2026, 8, 7, 15, 20, 0));

    // 45 minutes after today at 12:00
    let d_after_45 = abacus.eval_date("45 minutes after today at 12:00").unwrap();
    assert_eq!(d_after_45.time.hour, 12);
    assert_eq!(d_after_45.time.minute, 45);

    // in 45 minutes
    let d_in_45 = abacus.eval_date("in 45 minutes").unwrap();
    let diff_in_45 = (d_in_45.to_epoch_milliseconds() - now.to_epoch_milliseconds()).abs();
    assert!((diff_in_45 - 45 * 60_000).abs() < 5000);

    // 10 days from now
    let d_10d_fn = abacus.eval_date("10 days from now").unwrap();
    let diff_10d = (d_10d_fn.to_epoch_milliseconds() - now.to_epoch_milliseconds()).abs();
    assert!((diff_10d - 10 * 86_400_000).abs() < 5000);
}

#[cfg(feature = "units")]
#[test]
fn test_relative_time_chained_arithmetic() {
    let abacus = anchored_calc();

    // (3 hours before today at 5pm) + 30 min -> today at 14:30
    let d1 = abacus
        .eval_date("3 hours before today at 5pm + 30 min")
        .unwrap();
    assert_eq!(d1.time.hour, 14);
    assert_eq!(d1.time.minute, 30);

    // Property access on "(3 hours before today at 5pm)"
    let hr = abacus
        .eval_scalar("(3 hours before today at 5pm).hour")
        .unwrap();
    assert_eq!(hr.canonical, 14.0);

    // Date difference interval conversion
    let res = abacus
        .eval("3 hours before today at 5pm to today at 5pm in hours")
        .unwrap();
    assert_eq!(res.to_display(), "3 h");
}
