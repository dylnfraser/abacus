use abacus::eval;

#[test]
fn test_basic_number_scales() {
    assert_eq!(eval("3 million").unwrap().to_display(), "3000000");
    assert_eq!(eval("3 billion").unwrap().to_display(), "3000000000");
    assert_eq!(eval("10 thousand").unwrap().to_display(), "10000");
    assert_eq!(eval("2.5 trillion").unwrap().to_display(), "2500000000000");
    assert_eq!(eval("500 hundred").unwrap().to_display(), "50000");
    assert_eq!(eval("2 dozen").unwrap().to_display(), "24");
    assert_eq!(eval("5 gross").unwrap().to_display(), "720");
    assert_eq!(eval("3 myriad").unwrap().to_display(), "30000");
}

#[test]
fn test_plural_number_scales() {
    assert_eq!(eval("3 millions").unwrap().to_display(), "3000000");
    assert_eq!(eval("5 billions").unwrap().to_display(), "5000000000");
    assert_eq!(eval("2 thousands").unwrap().to_display(), "2000");
    assert_eq!(eval("4 dozens").unwrap().to_display(), "48");
}

#[test]
fn test_unspaced_number_scales() {
    assert_eq!(eval("3million").unwrap().to_display(), "3000000");
    assert_eq!(eval("5billion").unwrap().to_display(), "5000000000");
    assert_eq!(eval("10thousand").unwrap().to_display(), "10000");
    assert_eq!(eval("2dozen").unwrap().to_display(), "24");
}

#[test]
fn test_number_scale_arithmetic() {
    // Addition
    assert_eq!(
        eval("3 million + 500 thousand").unwrap().to_display(),
        "3500000"
    );

    // Division
    assert_eq!(eval("5 billion / 2 million").unwrap().to_display(), "2500");

    // Multiplication
    assert_eq!(eval("3 million * 4").unwrap().to_display(), "12000000");

    // Percentage of scale
    assert_eq!(eval("10% of 2 million").unwrap().to_display(), "200000");

    // Chained scales
    assert_eq!(
        eval("100 thousand million").unwrap().to_display(),
        "100000000000"
    );
}

#[test]
fn test_number_scales_with_physical_units() {
    // 3 million km
    let dist = eval("3 million km").unwrap();
    assert_eq!(dist.to_display(), "3000000 km");

    // Conversion of scaled physical units
    let in_meters = eval("3 million km to m").unwrap();
    assert_eq!(in_meters.to_display(), "3000000000 m");

    let speed = eval("10 thousand m / 2 s").unwrap();
    assert_eq!(speed.to_display(), "5000 m/s");

    // Unspaced scale with unit
    let unspaced_dist = eval("3million km").unwrap();
    assert_eq!(unspaced_dist.to_display(), "3000000 km");
}

#[test]
fn test_conversions_to_and_from_scales() {
    // 3 million in thousand
    assert_eq!(
        eval("3 million in thousand").unwrap().to_display(),
        "3000 thousand"
    );

    // 5000000 to million
    assert_eq!(
        eval("5000000 to million").unwrap().to_display(),
        "5 million"
    );

    // 24 to dozen
    assert_eq!(eval("24 to dozen").unwrap().to_display(), "2 dozen");

    // 3.5 billion in million
    assert_eq!(
        eval("3.5 billion in million").unwrap().to_display(),
        "3500 million"
    );

    // (3 million + 500 thousand) in million
    assert_eq!(
        eval("(3 million + 500 thousand) in million")
            .unwrap()
            .to_display(),
        "3.5 million"
    );
}

#[test]
fn test_standalone_scale_words() {
    // Standalone million evaluates to unit million (canonical 1,000,000)
    let five_million = eval("million * 5").unwrap();
    assert_eq!(five_million.to_display(), "5 million");
    assert_eq!(five_million.into_scalar().unwrap().canonical, 5000000.0);

    let three_dozen = eval("dozen * 3").unwrap();
    assert_eq!(three_dozen.to_display(), "3 dozen");
    assert_eq!(three_dozen.into_scalar().unwrap().canonical, 36.0);

    let sqrt_million = eval("sqrt(million)").unwrap();
    assert_eq!(sqrt_million.to_display(), "1000");

    let inv_million = eval("1 / million").unwrap();
    assert!((inv_million.into_scalar().unwrap().canonical - 1e-6).abs() < 1e-12);
}

#[test]
fn test_higher_order_scales() {
    assert_eq!(
        eval("2 quadrillion").unwrap().to_display(),
        "2000000000000000"
    );
    assert_eq!(
        eval("1 quintillion").unwrap().to_display(),
        "1000000000000000000"
    );

    let googol = eval("1 googol").unwrap();
    assert_eq!(googol.into_scalar().unwrap().canonical, 1e100);
}

#[test]
fn test_number_scales_config_option() {
    use abacus::Abacus;

    // Enabled by default
    let default_calc = Abacus::standard();
    assert_eq!(
        default_calc.eval("3 million").unwrap().to_display(),
        "3000000"
    );

    // Disabled via builder
    let disabled_calc = Abacus::standard().with_number_scales(false);
    // When disabled, "3 million" is treated as Value(3, unit: million) rather than scalar 3000000
    let res = disabled_calc.eval("3 million").unwrap();
    assert_eq!(res.to_display(), "3 million");

    // Re-enabled in place
    let mut mutable_calc = disabled_calc;
    mutable_calc.set_scales(true);
    assert_eq!(
        mutable_calc.eval("3 million").unwrap().to_display(),
        "3000000"
    );
}
