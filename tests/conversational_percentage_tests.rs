use abacus::eval;

#[test]
fn test_discounts_and_reductions() {
    // "20% off $120" -> $96
    let res1 = eval("20% off $120").unwrap();
    assert_eq!(res1.to_display(), "$96");

    // "15% off 200 EUR" -> 170 EUR
    let res2 = eval("15% off 200 EUR").unwrap();
    assert_eq!(res2.to_display(), "170 EUR");

    // "30% off 50 kg" -> 35 kg
    let res3 = eval("30% off 50 kg").unwrap();
    assert_eq!(res3.to_display(), "35 kg");

    // Trailing "off" modifier: "$100 - 20% off" -> $80
    let res4 = eval("$100 - 20% off").unwrap();
    assert_eq!(res4.to_display(), "$80");

    // "($100 - 20% off) * 2" -> $160
    let res5 = eval("($100 - 20% off) * 2").unwrap();
    assert_eq!(res5.to_display(), "$160");
}

#[test]
fn test_additions_taxes_and_tips() {
    // "$85 + 18% tip" -> $100.30
    let res1 = eval("$85 + 18% tip").unwrap();
    assert_eq!(res1.to_display(), "$100.30");

    // "$50 after 15% tax" -> $57.50
    let res2 = eval("$50 after 15% tax").unwrap();
    assert_eq!(res2.to_display(), "$57.50");

    // "$50 after 15%" -> $57.50
    let res3 = eval("$50 after 15%").unwrap();
    assert_eq!(res3.to_display(), "$57.50");

    // "$100 after 20% discount" -> $80
    let res4 = eval("$100 after 20% discount").unwrap();
    assert_eq!(res4.to_display(), "$80");

    // "$100 after 20% off" -> $80
    let res5 = eval("$100 after 20% off").unwrap();
    assert_eq!(res5.to_display(), "$80");

    // "$50 + 10% fee" -> $55
    let res6 = eval("$50 + 10% fee").unwrap();
    assert_eq!(res6.to_display(), "$55");

    // "$100 + 20% vat" -> $120
    let res7 = eval("$100 + 20% vat").unwrap();
    assert_eq!(res7.to_display(), "$120");
}

#[test]
fn test_proportional_percentage_and_out_of() {
    // "40 as a % of 200" -> 20%
    let res1 = eval("40 as a % of 200").unwrap();
    assert_eq!(res1.to_display(), "20%");

    // "40 as % of 200" -> 20%
    let res2 = eval("40 as % of 200").unwrap();
    assert_eq!(res2.to_display(), "20%");

    // "40 out of 200 as %" -> 20%
    let res3 = eval("40 out of 200 as %").unwrap();
    assert_eq!(res3.to_display(), "20%");

    // "40 out of 200" -> 0.2
    let res4 = eval("40 out of 200").unwrap();
    assert_eq!(res4.to_display(), "0.2");

    // "3 out of 5 in %" -> 60%
    let res5 = eval("3 out of 5 in %").unwrap();
    assert_eq!(res5.to_display(), "60%");
}

#[test]
fn test_relative_percentage_differences() {
    // "% change from 50 to 75" -> +50%
    let res1 = eval("% change from 50 to 75").unwrap();
    assert_eq!(res1.to_display(), "+50%");

    // "% change from 100 to 80" -> -20%
    let res2 = eval("% change from 100 to 80").unwrap();
    assert_eq!(res2.to_display(), "-20%");

    // "percent change from $80 to $100" -> +25%
    let res3 = eval("percent change from $80 to $100").unwrap();
    assert_eq!(res3.to_display(), "+25%");

    // "percentage change from 2 hours to 3 hours" -> +50%
    let res4 = eval("percentage change from 2 hours to 3 hours").unwrap();
    assert_eq!(res4.to_display(), "+50%");
}

#[test]
fn test_relative_scaling_more_and_less_than() {
    // "30% more than 50 kg" -> 65 kg
    let res1 = eval("30% more than 50 kg").unwrap();
    assert_eq!(res1.to_display(), "65 kg");

    // "5 kg more than 50 kg" -> 55 kg
    let res2 = eval("5 kg more than 50 kg").unwrap();
    assert_eq!(res2.to_display(), "55 kg");

    // "15% less than 2 hours in minutes" -> 102 min
    let res3 = eval("15% less than 2 hours in minutes").unwrap();
    assert_eq!(res3.to_display(), "102 min");

    // "15% less than 2 hours" -> 1.7 h
    let res4 = eval("15% less than 2 hours").unwrap();
    assert_eq!(res4.to_display(), "1.7 h");

    // "20% more than $100" -> $120
    let res5 = eval("20% more than $100").unwrap();
    assert_eq!(res5.to_display(), "$120");
}

#[test]
fn test_adding_and_subtracting_percentages() {
    // "50% + 50%" -> 100%
    let res1 = eval("50% + 50%").unwrap();
    assert_eq!(res1.to_display(), "100%");

    // "(% change from 50 to 70) + 50%" -> 90% (40% + 50%)
    let res2 = eval("(% change from 50 to 70) + 50%").unwrap();
    assert_eq!(res2.to_display(), "90%");

    // "50% - 20%" -> 30%
    let res3 = eval("50% - 20%").unwrap();
    assert_eq!(res3.to_display(), "30%");

    // "(% change from 50 to 70) - 10%" -> 30% (40% - 10%)
    let res4 = eval("(% change from 50 to 70) - 10%").unwrap();
    assert_eq!(res4.to_display(), "30%");

    // "10% + 20% + 30%" -> 60%
    let res5 = eval("10% + 20% + 30%").unwrap();
    assert_eq!(res5.to_display(), "60%");
}
