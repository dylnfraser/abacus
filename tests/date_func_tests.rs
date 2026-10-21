#![cfg(feature = "date")]
use abacus::Abacus;

#[test]
fn test_date_property_functions() {
    let abacus = Abacus::standard();

    // year(17-08-2026) -> 2026
    let y = abacus.eval_scalar("year(17-08-2026)").unwrap();
    assert_eq!(y.canonical, 2026.0);

    // month(17-08-2026) -> 8
    let m = abacus.eval_scalar("month(17-08-2026)").unwrap();
    assert_eq!(m.canonical, 8.0);

    // day(17-08-2026) -> 17
    let d = abacus.eval_scalar("day(17-08-2026)").unwrap();
    assert_eq!(d.canonical, 17.0);

    // hour(2026-08-17 15:45:00) -> 15
    let h = abacus.eval_scalar("hour(17-08-2026 15:45:00)").unwrap();
    assert_eq!(h.canonical, 15.0);

    // minute(17-08-2026 15:45:00) -> 45
    let min = abacus.eval_scalar("minute(17-08-2026 15:45:00)").unwrap();
    assert_eq!(min.canonical, 45.0);

    // day_of_week(17-08-2026) -> 1 (Monday)
    let dow = abacus.eval_scalar("day_of_week(17-08-2026)").unwrap();
    assert_eq!(dow.canonical, 1.0);
}

#[test]
fn test_relative_date_property_functions() {
    let abacus = Abacus::standard();

    let m = abacus.eval_scalar("month(today)").unwrap();
    assert!(m.canonical >= 1.0 && m.canonical <= 12.0);

    let h = abacus.eval_scalar("hour(today at 12:00)").unwrap();
    assert_eq!(h.canonical, 12.0);
}
