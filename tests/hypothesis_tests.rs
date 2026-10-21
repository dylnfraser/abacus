use abacus::{Abacus, AbacusError, EvalResult, Hash, Value};

fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    Abacus::standard().eval(expr)
}

fn eval_scalar(expr: &str) -> Result<Value, AbacusError> {
    Abacus::standard().eval_scalar(expr)
}

fn eval_hash(expr: &str) -> Result<Hash, AbacusError> {
    Abacus::standard().eval_hash(expr)
}

// ── 1. ZTest ──

#[test]
fn test_section4_hypothesis_testing_expanded() {
    let calc = Abacus::standard();

    // ZTest(100 m, 105 m, 15 m, 50) and dot property access .p_value
    let p_val = calc
        .eval_scalar("ZTest(100 m, 105 m, 15 m, 50).p_value")
        .unwrap();
    assert!((p_val.canonical - 0.018_422_125_454_099_01).abs() < 1e-14);

    // TTest(100 m, 105 m, 15 m, 25)
    let t_hash = calc.eval_hash("TTest(100 m, 105 m, 15 m, 25)").unwrap();
    assert_eq!(t_hash.get("df").unwrap().canonical, 24.0);

    // 2-SampTTest(100 m, 15 m, 25, 90 m, 10 m, 30)
    let samp2 = calc
        .eval_hash("2-SampTTest(100 m, 15 m, 25, 90 m, 10 m, 30)")
        .unwrap();
    assert!(samp2.get("p_value").unwrap().canonical < 0.01);

    // Chi2Test(15, 25, 10, 30).p_value
    let chi2_p = calc
        .eval_scalar("Chi2Test(15, 25, 10, 30).p_value")
        .unwrap();
    assert!((chi2_p.canonical - 0.067_889_154_861_829_02).abs() < 1e-14);
}

#[test]
fn test_z_test_summary_stats() {
    // ZTest(mu0: 100 m, xbar: 105 m, sigma: 15 m, n: 50)
    let hash = eval_hash("ZTest(100 m, 105 m, 15 m, 50)").unwrap();
    let z = hash.get("z").unwrap().canonical;
    let p = hash.get("p").unwrap().canonical;

    // z = (105 - 100) / (15 / sqrt(50)) = 5 / 2.12132 = 2.35702
    assert!(
        (z - 2.357_022_603_955_158).abs() < 1e-12,
        "Expected z ≈ 2.357, got {}",
        z
    );
    assert!(
        (p - 0.018_422_125_454_099_01).abs() < 1e-14,
        "Expected p ≈ 0.0184, got {}",
        p
    );

    // Test dot property access on ZTest
    let z_prop = eval_scalar("ZTest(100 m, 105 m, 15 m, 50).z").unwrap();
    assert!((z_prop.canonical - 2.357_022_603_955_158).abs() < 1e-12);
}

#[test]
fn test_z_test_data_list() {
    // ZTestData(mu0: 10 m, sigma: 2 m, data: 12 m, 14 m, 10 m, 16 m)
    let hash = eval_hash("ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)").unwrap();
    let mean = hash.get("mean").unwrap();
    assert_eq!(mean.to_display(), "13 m");
    let z = hash.get("z").unwrap().canonical;
    // mean = 13, n = 4, SE = 2 / 2 = 1.0, z = (13 - 10) / 1 = 3.0
    assert!((z - 3.0).abs() < 1e-5);
}

// ── 2. TTest ──

#[test]
fn test_t_test_summary_stats() {
    // TTest(mu0: 100 m, xbar: 105 m, s: 15 m, n: 25)
    let hash = eval_hash("TTest(100 m, 105 m, 15 m, 25)").unwrap();
    let t = hash.get("t").unwrap().canonical;
    let p = hash.get("p").unwrap().canonical;
    let df = hash.get("df").unwrap().canonical;

    // t = (105 - 100) / (15 / 5) = 5 / 3 = 1.66667
    assert!(
        (t - 5.0 / 3.0).abs() < 1e-12,
        "Expected t ≈ 1.6667, got {}",
        t
    );
    assert_eq!(df, 24.0);
    assert!(
        (p - 0.108_580_123_024_722_97).abs() < 1e-13,
        "Expected p ≈ 0.1086, got {}",
        p
    );
}

#[test]
fn test_t_test_data_list() {
    // TTestData(mu0: 10 m, data: 12 m, 14 m, 10 m, 16 m)
    let hash = eval_hash("TTestData(10 m, 12 m, 14 m, 10 m, 16 m)").unwrap();
    let t = hash.get("t").unwrap().canonical;
    assert!((t - 2.323_790_007_724_45).abs() < 1e-12);
    let p = hash.get("p").unwrap().canonical;
    assert!((p - 0.102_728_078_858_399).abs() < 1e-13);
}

// ── 3. 1-PropZTest ──

#[test]
fn test_1_prop_z_test() {
    // 1-PropZTest(p0: 0.5, x: 45, n: 100)
    let hash = eval_hash("1-PropZTest(0.5, 45, 100)").unwrap();
    let z = hash.get("z").unwrap().canonical;
    let phat = hash.get("phat").unwrap().canonical;

    // phat = 0.45, SE = sqrt(0.25 / 100) = 0.05, z = -0.05 / 0.05 = -1.0
    assert!((z - (-1.0)).abs() < 1e-5);
    assert!((phat - 0.45).abs() < 1e-5);

    // Test alias 1PropZTest and propztest
    let z_alias = eval_scalar("propztest(0.5, 45, 100).z").unwrap();
    assert!((z_alias.canonical - (-1.0)).abs() < 1e-5);
}

// ── 4. 2-SampZTest ──

#[test]
fn test_2_samp_z_test() {
    // 2-SampZTest(sigma1: 15 m, sigma2: 10 m, xbar1: 100 m, n1: 50, xbar2: 90 m, n2: 50)
    let hash = eval_hash("2-SampZTest(15 m, 10 m, 100 m, 50, 90 m, 50)").unwrap();
    let z = hash.get("z").unwrap().canonical;
    let diff = hash.get("diff").unwrap();

    // diff = 10 m, SE = sqrt(225/50 + 100/50) = sqrt(6.5) ≈ 2.5495, z ≈ 3.9223
    assert_eq!(diff.to_display(), "10 m");
    assert!((z - 10.0 / 6.5_f64.sqrt()).abs() < 1e-12);
}

// ── 5. 2-SampTTest ──

#[test]
fn test_2_samp_t_test() {
    // 2-SampTTest(xbar1: 100 m, s1: 15 m, n1: 25, xbar2: 90 m, s2: 10 m, n2: 30)
    let hash = eval_hash("2-SampTTest(100 m, 15 m, 25, 90 m, 10 m, 30)").unwrap();
    let t = hash.get("t").unwrap().canonical;
    let df = hash.get("df").unwrap().canonical;

    // t ≈ 2.846, Welch df ≈ 40.475
    assert!(
        (t - 2.847_473_987_257_497).abs() < 1e-12,
        "Expected t ≈ 2.847, got {}",
        t
    );
    assert!(
        (df - 40.475_086_020_135_08).abs() < 1e-9,
        "Expected df ≈ 40.475, got {}",
        df
    );
    let p = hash.get("p").unwrap().canonical;
    assert!((p - 0.006_891_426_705_032_224).abs() < 1e-14);
}

// ── 6. 2-PropZTest ──

#[test]
fn test_2_prop_z_test() {
    // 2-PropZTest(x1: 45, n1: 100, x2: 30, n2: 100)
    let hash = eval_hash("2-PropZTest(45, 100, 30, 100)").unwrap();
    let z = hash.get("z").unwrap().canonical;
    let diff = hash.get("diff").unwrap().canonical;

    // p1 = 0.45, p2 = 0.30, diff = 0.15, p_pooled = 75/200 = 0.375, SE ≈ 0.06846, z ≈ 2.191
    assert!((diff - 0.15).abs() < 1e-5);
    assert!((z - 2.191).abs() < 0.02, "Expected z ≈ 2.191, got {}", z);
}

// ── 7. Chi2Test ──

#[test]
fn test_chi2_test() {
    // Chi2Test(obs1: 15, obs2: 25, exp1: 10, exp2: 30) — observed counts first, then expected
    let hash = eval_hash("Chi2Test(15, 25, 10, 30)").unwrap();
    let chi2 = hash.get("chi2").unwrap().canonical;
    let df = hash.get("df").unwrap().canonical;

    // chi2 = (15-10)^2/10 + (25-30)^2/30 = 25/10 + 25/30 = 2.5 + 0.8333 = 3.3333
    assert!((chi2 - 10.0 / 3.0).abs() < 1e-12);
    assert_eq!(df, 1.0);

    let p_val = eval_scalar("Chi2Test(15, 25, 10, 30).p_value").unwrap();
    assert!((p_val.canonical - 0.067_889_154_861_829_02).abs() < 1e-14);
}

// ── Error handling ──

#[test]
fn test_hypothesis_error_cases() {
    // Incompatible unit dimensions
    assert!(eval("ZTest(100 m, 105 s, 15 m, 50)").is_err());
    // Zero sample size or negative standard deviation
    assert!(eval("TTest(100 m, 105 m, -5 m, 25)").is_err());
}
