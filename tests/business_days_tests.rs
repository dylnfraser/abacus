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

    // 07-08-2026 + 5 business days = 14-08-2026
    assert_eq!(
        calc.eval("07-08-2026 + 5 business days")
            .unwrap()
            .to_display(),
        "August 14, 2026"
    );

    // 07-08-2026 + 3 work days = 12-08-2026
    assert_eq!(
        calc.eval("07-08-2026 + 3 work days").unwrap().to_display(),
        "August 12, 2026"
    );

    // workdays(07-08-2026, 14-08-2026) = 5 workdays
    assert_eq!(
        calc.eval("workdays(07-08-2026, 14-08-2026)")
            .unwrap()
            .to_display(),
        "5 workdays"
    );

    // is_weekend(07-08-2026) = 0
    assert_eq!(
        calc.eval("is_weekend(07-08-2026)").unwrap().to_display(),
        "0"
    );
}

#[cfg(feature = "units")]
#[test]
fn test_business_days_addition_across_weekend() {
    let abacus = Abacus::standard();

    // Friday Aug 7, 2026 + 5 business days -> Friday Aug 14, 2026 (skips Aug 8 Saturday & Aug 9 Sunday)
    let d1 = abacus.eval_date("07-08-2026 + 5 business days").unwrap();
    assert_eq!(d1.day, 14);
    assert_eq!(d1.month, 8);
    assert_eq!(d1.year, 2026);

    // Friday Aug 7, 2026 + 3 work days -> Wednesday Aug 12, 2026
    let d2 = abacus.eval_date("07-08-2026 + 3 work days").unwrap();
    assert_eq!(d2.day, 12);

    // Friday Aug 7, 2026 + 1 workday -> Monday Aug 10, 2026
    let d3 = abacus.eval_date("07-08-2026 + 1 workday").unwrap();
    assert_eq!(d3.day, 10);
}

#[cfg(feature = "units")]
#[test]
fn test_business_days_subtraction() {
    let abacus = Abacus::standard();

    // Friday Aug 7, 2026 - 2 workdays -> Wednesday Aug 5, 2026
    let d1 = abacus.eval_date("07-08-2026 - 2 workdays").unwrap();
    assert_eq!(d1.day, 5);

    // Monday Aug 10, 2026 - 1 business day -> Friday Aug 7, 2026 (skips Sunday & Saturday)
    let d2 = abacus.eval_date("10-08-2026 - 1 business day").unwrap();
    assert_eq!(d2.day, 7);
}

#[cfg(feature = "date")]
#[test]
fn test_workdays_functions() {
    let abacus = Abacus::standard();

    // workdays(07-08-2026, 14-08-2026) -> 5 workdays
    let w1 = abacus.eval("workdays(07-08-2026, 14-08-2026)").unwrap();
    assert_eq!(w1.to_display(), "5 workdays");

    // is_weekend & is_workday
    let is_wknd = abacus.eval_scalar("is_weekend(07-08-2026)").unwrap();
    assert_eq!(is_wknd.canonical, 0.0);

    let is_work = abacus.eval_scalar("is_workday(07-08-2026)").unwrap();
    assert_eq!(is_work.canonical, 1.0);

    let sat_is_wknd = abacus.eval_scalar("is_weekend(08-08-2026)").unwrap();
    assert_eq!(sat_is_wknd.canonical, 1.0);
}
