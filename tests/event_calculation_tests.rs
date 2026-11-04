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

#[test]
fn test_nth_weekday_of_month() {
    let calc = Abacus::standard();

    let d1 = calc.eval_date("third thursday of november 2026").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 11);
    assert_eq!(d1.day, 19);

    let d2 = calc.eval_date("last friday of october 2026").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 10);
    assert_eq!(d2.day, 30);

    let d3 = calc.eval_date("first monday of january 2027").unwrap();
    assert_eq!(d3.year, 2027);
    assert_eq!(d3.month, 1);
    assert_eq!(d3.day, 4);

    let d4 = calc.eval_date("second tuesday in march 2026").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 3);
    assert_eq!(d4.day, 10);

    let d5 = calc.eval_date("4th thursday in november 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 11);
    assert_eq!(d5.day, 26);
}

#[test]
fn test_named_holidays() {
    let calc = Abacus::standard();

    let d1 = calc.eval_date("christmas 2026").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 12);
    assert_eq!(d1.day, 25);

    let d2 = calc.eval_date("thanksgiving 2026").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 11);
    assert_eq!(d2.day, 26);

    let d3 = calc.eval_date("black friday 2026").unwrap();
    assert_eq!(d3.year, 2026);
    assert_eq!(d3.month, 11);
    assert_eq!(d3.day, 27);

    let d4 = calc.eval_date("cyber monday 2026").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 11);
    assert_eq!(d4.day, 30);

    let d5 = calc.eval_date("halloween 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 10);
    assert_eq!(d5.day, 31);

    let d6 = calc.eval_date("new year 2027").unwrap();
    assert_eq!(d6.year, 2027);
    assert_eq!(d6.month, 1);
    assert_eq!(d6.day, 1);

    let d7 = calc.eval_date("easter 2026").unwrap();
    assert_eq!(d7.year, 2026);
    assert_eq!(d7.month, 4);
    assert_eq!(d7.day, 5);
}

#[test]
fn test_quarter_and_period_boundaries() {
    let anchor = Date::new(2026, 9, 1); // Q3
    let calc = Abacus::standard().with_now(anchor);

    let d1 = calc.eval_date("end of quarter").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 9);
    assert_eq!(d1.day, 30);

    let d2 = calc.eval_date("end of the quarter").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 9);
    assert_eq!(d2.day, 30);

    let d3 = calc.eval_date("start of quarter").unwrap();
    assert_eq!(d3.year, 2026);
    assert_eq!(d3.month, 7);
    assert_eq!(d3.day, 1);

    let d4 = calc.eval_date("end of next quarter").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 12);
    assert_eq!(d4.day, 31);

    let d5 = calc.eval_date("end of q1 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 3);
    assert_eq!(d5.day, 31);

    let d6 = calc.eval_date("end of month").unwrap();
    assert_eq!(d6.year, 2026);
    assert_eq!(d6.month, 9);
    assert_eq!(d6.day, 30);

    let d7 = calc.eval_date("end of year").unwrap();
    assert_eq!(d7.year, 2026);
    assert_eq!(d7.month, 12);
    assert_eq!(d7.day, 31);
}

#[cfg(feature = "units")]
#[test]
fn test_days_until_calculations() {
    let anchor = Date::new(2026, 9, 1);
    let calc = Abacus::standard().with_now(anchor);

    // The anchor is Sept 1, 2026.
    let res1 = calc.eval("days until christmas").unwrap();
    assert_eq!(res1.to_display(), "115 d");

    let res2 = calc.eval("days until end of quarter").unwrap();
    assert_eq!(res2.to_display(), "29 d");

    let res3 = calc.eval("business days until end of quarter").unwrap();
    assert_eq!(res3.to_display(), "21 business_days");

    let res3_b = calc.eval("bdays until end of quarter").unwrap();
    assert_eq!(res3_b.to_display(), "21 bdays");

    let res4 = calc
        .eval("days until third thursday of november 2026")
        .unwrap();
    assert_eq!(res4.to_display(), "79 d");

    let res5 = calc
        .eval("business days until third thursday of november 2026")
        .unwrap();
    assert_eq!(res5.to_display(), "57 business_days");

    // Dec 25, 2026 is a Friday.
    let res6 = calc.eval("business days until christmas").unwrap();
    assert_eq!(res6.to_display(), "83 business_days");

    // `until` alone counts days.
    let res7 = calc.eval("until christmas").unwrap();
    assert_eq!(res7.to_display(), "115 d");

    let res8 = calc.eval("days until 2026-12-25").unwrap();
    assert_eq!(res8.to_display(), "115 d");
}
