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

    // "third thursday of november 2026" -> 2026-11-19
    let d1 = calc.eval_date("third thursday of november 2026").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 11);
    assert_eq!(d1.day, 19);

    // "last friday of october 2026" -> 2026-10-30
    let d2 = calc.eval_date("last friday of october 2026").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 10);
    assert_eq!(d2.day, 30);

    // "first monday of january 2027" -> 2027-01-04
    let d3 = calc.eval_date("first monday of january 2027").unwrap();
    assert_eq!(d3.year, 2027);
    assert_eq!(d3.month, 1);
    assert_eq!(d3.day, 4);

    // "second tuesday in march 2026" -> 2026-03-10
    let d4 = calc.eval_date("second tuesday in march 2026").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 3);
    assert_eq!(d4.day, 10);

    // "4th thursday in november 2026" -> 2026-11-26
    let d5 = calc.eval_date("4th thursday in november 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 11);
    assert_eq!(d5.day, 26);
}

#[test]
fn test_named_holidays() {
    let calc = Abacus::standard();

    // "christmas 2026" -> 2026-12-25
    let d1 = calc.eval_date("christmas 2026").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 12);
    assert_eq!(d1.day, 25);

    // "thanksgiving 2026" -> 2026-11-26
    let d2 = calc.eval_date("thanksgiving 2026").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 11);
    assert_eq!(d2.day, 26);

    // "black friday 2026" -> 2026-11-27
    let d3 = calc.eval_date("black friday 2026").unwrap();
    assert_eq!(d3.year, 2026);
    assert_eq!(d3.month, 11);
    assert_eq!(d3.day, 27);

    // "cyber monday 2026" -> 2026-11-30
    let d4 = calc.eval_date("cyber monday 2026").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 11);
    assert_eq!(d4.day, 30);

    // "halloween 2026" -> 2026-10-31
    let d5 = calc.eval_date("halloween 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 10);
    assert_eq!(d5.day, 31);

    // "new year 2027" -> 2027-01-01
    let d6 = calc.eval_date("new year 2027").unwrap();
    assert_eq!(d6.year, 2027);
    assert_eq!(d6.month, 1);
    assert_eq!(d6.day, 1);

    // "easter 2026" -> 2026-04-05
    let d7 = calc.eval_date("easter 2026").unwrap();
    assert_eq!(d7.year, 2026);
    assert_eq!(d7.month, 4);
    assert_eq!(d7.day, 5);
}

#[test]
fn test_quarter_and_period_boundaries() {
    let anchor = Date::new(2026, 9, 1); // Q3
    let calc = Abacus::standard().with_now(anchor);

    // "end of quarter" -> 2026-09-30
    let d1 = calc.eval_date("end of quarter").unwrap();
    assert_eq!(d1.year, 2026);
    assert_eq!(d1.month, 9);
    assert_eq!(d1.day, 30);

    // "end of the quarter" -> 2026-09-30
    let d2 = calc.eval_date("end of the quarter").unwrap();
    assert_eq!(d2.year, 2026);
    assert_eq!(d2.month, 9);
    assert_eq!(d2.day, 30);

    // "start of quarter" -> 2026-07-01
    let d3 = calc.eval_date("start of quarter").unwrap();
    assert_eq!(d3.year, 2026);
    assert_eq!(d3.month, 7);
    assert_eq!(d3.day, 1);

    // "end of next quarter" -> 2026-12-31 (Q4)
    let d4 = calc.eval_date("end of next quarter").unwrap();
    assert_eq!(d4.year, 2026);
    assert_eq!(d4.month, 12);
    assert_eq!(d4.day, 31);

    // "end of q1 2026" -> 2026-03-31
    let d5 = calc.eval_date("end of q1 2026").unwrap();
    assert_eq!(d5.year, 2026);
    assert_eq!(d5.month, 3);
    assert_eq!(d5.day, 31);

    // "end of month" -> 2026-09-30
    let d6 = calc.eval_date("end of month").unwrap();
    assert_eq!(d6.year, 2026);
    assert_eq!(d6.month, 9);
    assert_eq!(d6.day, 30);

    // "end of year" -> 2026-12-31
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

    // "days until christmas" -> 115 d (Sept 1 to Dec 25)
    let res1 = calc.eval("days until christmas").unwrap();
    assert_eq!(res1.to_display(), "115 d");

    // "days until end of quarter" -> 29 d (Sept 1 to Sept 30)
    let res2 = calc.eval("days until end of quarter").unwrap();
    assert_eq!(res2.to_display(), "29 d");

    // "business days until end of quarter" -> 21 business_days
    let res3 = calc.eval("business days until end of quarter").unwrap();
    assert_eq!(res3.to_display(), "21 business_days");

    // "bdays until end of quarter" -> 21 bdays
    let res3_b = calc.eval("bdays until end of quarter").unwrap();
    assert_eq!(res3_b.to_display(), "21 bdays");

    // "days until third thursday of november 2026" -> 79 d
    let res4 = calc
        .eval("days until third thursday of november 2026")
        .unwrap();
    assert_eq!(res4.to_display(), "79 d");

    // "business days until third thursday of november 2026" -> 57 business_days
    let res5 = calc
        .eval("business days until third thursday of november 2026")
        .unwrap();
    assert_eq!(res5.to_display(), "57 business_days");

    // "business days until christmas" (Dec 25 is Friday)
    let res6 = calc.eval("business days until christmas").unwrap();
    // Sept 1 to Dec 25: 83 business days
    assert_eq!(res6.to_display(), "83 business_days");

    // "until christmas" defaults to days
    let res7 = calc.eval("until christmas").unwrap();
    assert_eq!(res7.to_display(), "115 d");

    // "days until 2026-12-25" -> 115 d
    let res8 = calc.eval("days until 2026-12-25").unwrap();
    assert_eq!(res8.to_display(), "115 d");
}
