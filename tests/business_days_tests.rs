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
use abacus::Abacus;

#[cfg(all(feature = "units", feature = "date"))]
#[test]
fn test_section7_business_days_expanded() {
    let calc = Abacus::standard();

    // 07-08-2026 is a Friday.
    assert_eq!(
        calc.eval("07-08-2026 + 5 business days")
            .unwrap()
            .to_display(),
        "August 14, 2026"
    );

    assert_eq!(
        calc.eval("07-08-2026 + 3 work days").unwrap().to_display(),
        "August 12, 2026"
    );

    assert_eq!(
        calc.eval("workdays(07-08-2026, 14-08-2026)")
            .unwrap()
            .to_display(),
        "5 workdays"
    );

    assert_eq!(
        calc.eval("is_weekend(07-08-2026)").unwrap().to_display(),
        "0"
    );
}

#[cfg(feature = "units")]
#[test]
fn test_business_days_addition_across_weekend() {
    let abacus = Abacus::standard();

    // Friday Aug 7, 2026 + 5 business days is Friday Aug 14 (skipping the weekend of Aug 8-9).
    let d1 = abacus.eval_date("07-08-2026 + 5 business days").unwrap();
    assert_eq!(d1.day, 14);
    assert_eq!(d1.month, 8);
    assert_eq!(d1.year, 2026);

    // Friday + 3 work days is Wednesday Aug 12.
    let d2 = abacus.eval_date("07-08-2026 + 3 work days").unwrap();
    assert_eq!(d2.day, 12);

    // Friday + 1 workday is Monday Aug 10.
    let d3 = abacus.eval_date("07-08-2026 + 1 workday").unwrap();
    assert_eq!(d3.day, 10);
}

#[cfg(feature = "units")]
#[test]
fn test_business_days_subtraction() {
    let abacus = Abacus::standard();

    // Friday Aug 7, 2026 - 2 workdays is Wednesday Aug 5.
    let d1 = abacus.eval_date("07-08-2026 - 2 workdays").unwrap();
    assert_eq!(d1.day, 5);

    // Monday Aug 10 - 1 business day is Friday Aug 7, skipping the weekend.
    let d2 = abacus.eval_date("10-08-2026 - 1 business day").unwrap();
    assert_eq!(d2.day, 7);
}

#[cfg(feature = "date")]
#[test]
fn test_workdays_functions() {
    let abacus = Abacus::standard();

    let w1 = abacus.eval("workdays(07-08-2026, 14-08-2026)").unwrap();
    assert_eq!(w1.to_display(), "5 workdays");

    // 07-08-2026 is a Friday and 08-08-2026 a Saturday.
    let is_wknd = abacus.eval_scalar("is_weekend(07-08-2026)").unwrap();
    assert_eq!(is_wknd.canonical, 0.0);

    let is_work = abacus.eval_scalar("is_workday(07-08-2026)").unwrap();
    assert_eq!(is_work.canonical, 1.0);

    let sat_is_wknd = abacus.eval_scalar("is_weekend(08-08-2026)").unwrap();
    assert_eq!(sat_is_wknd.canonical, 1.0);
}
