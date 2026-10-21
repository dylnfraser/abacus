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
use abacus::{Abacus, AbacusError, EvalResult};

fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    Abacus::standard().eval(expr)
}

fn eval_scalar(expr: &str) -> Result<abacus::Value, AbacusError> {
    Abacus::standard().eval_scalar(expr)
}

// ── TInterval (TI-84 Option 8) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_section3_confidence_intervals_expanded() {
    let calc = Abacus::standard();

    // TInterval(10 m, 12 m, 11 m, 14 m)
    let t_int = calc.eval("TInterval(10 m, 12 m, 11 m, 14 m)").unwrap();
    assert!(t_int.to_display().contains("m"));

    // ZInterval(100 m, 15 m, 100) -> [97.06 m, 102.94 m]; z* = 1.959963984540054
    let z_int = calc.eval("ZInterval(100 m, 15 m, 100)").unwrap();
    assert_eq!(
        z_int.to_display(),
        "[97.06005402318992 m, 102.93994597681008 m]"
    );

    // 1-PropZInt(45, 100) — Wald interval, as on the TI-84
    let prop_int = calc.eval("1-PropZInt(45, 100)").unwrap();
    if let EvalResult::Interval(interval) = prop_int {
        assert!((interval.lo.canonical - 0.352_493_022_910_060_6).abs() < 1e-14);
        assert!((interval.hi.canonical - 0.547_506_977_089_939_5).abs() < 1e-14);
    } else {
        panic!("Expected interval result");
    }

    // 2-SampTInt(100 m, 15 m, 25, 90 m, 10 m, 30)
    let samp2_int = calc
        .eval("2-SampTInt(100 m, 15 m, 25, 90 m, 10 m, 30)")
        .unwrap();
    assert!(samp2_int.to_display().contains("m"));
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_t_interval_sample_data() {
    // Tests TInterval with sample data: TInterval(10 m, 12 m, 11 m, 14 m)
    let result = eval("TInterval(10 m, 12 m, 11 m, 14 m)").unwrap();
    let display = result.to_display();
    assert!(
        display.starts_with("[") && display.ends_with("]"),
        "Got display: {}",
        display
    );

    if let EvalResult::Interval(interval) = result {
        let lo = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        assert!(
            (lo - 9.032_469_116_203_982).abs() < 1e-12,
            "lo should be ~9.03 m, got {}",
            lo
        );
        assert!(
            (hi - 14.467_530_883_796_018).abs() < 1e-12,
            "hi should be ~14.47 m, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_t_interval_summary_stats() {
    // Tests TIntervalStats with summary statistics: TIntervalStats(100 m, 15 m, 25)
    let result = eval("TIntervalStats(100 m, 15 m, 25)").unwrap();
    let display = result.to_display();
    assert!(
        display.contains("m"),
        "Expected meters in display: {}",
        display
    );

    if let EvalResult::Interval(interval) = result {
        let lo = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        assert!(
            (lo - 93.808_304_315_115_92).abs() < 1e-11,
            "lo should be ~93.81, got {}",
            lo
        );
        assert!(
            (hi - 106.191_695_684_884_08).abs() < 1e-11,
            "hi should be ~106.19, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── ZInterval (TI-84 Option 7) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_z_interval_summary_stats() {
    // Tests ZInterval with summary statistics: ZInterval(100 m, 15 m, 100)
    let result = eval("ZInterval(100 m, 15 m, 100)").unwrap();
    if let EvalResult::Interval(interval) = result {
        let lo = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        assert!(
            (lo - 97.060_054_023_189_92).abs() < 1e-11,
            "lo should be ~97.06, got {}",
            lo
        );
        assert!(
            (hi - 102.939_945_976_810_08).abs() < 1e-11,
            "hi should be ~102.94, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── 1-PropZInt (TI-84 Option A) ──

#[cfg(feature = "stats")]
#[test]
fn test_ti84_1_prop_z_int() {
    // Tests 1-PropZInt(45, 100) and 1_PropZInt(45, 100)
    let result1 = eval("1-PropZInt(45, 100)").unwrap();
    let result2 = eval("1_PropZInt(45, 100)").unwrap();
    assert_eq!(result1.to_display(), result2.to_display());

    if let EvalResult::Interval(interval) = result1 {
        let lo = interval.lo.canonical;
        let hi = interval.hi.canonical;
        assert!(
            (lo - 0.352_493_022_910_060_6).abs() < 1e-14,
            "lo should be ~0.3525, got {}",
            lo
        );
        assert!(
            (hi - 0.547_506_977_089_939_5).abs() < 1e-14,
            "hi should be ~0.5475, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── 2-SampTInt (TI-84 Option 0) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_2_samp_t_int() {
    // Tests 2-SampTInt(100 m, 15 m, 25, 90 m, 10 m, 30)
    let result = eval("2-SampTInt(100 m, 15 m, 25, 90 m, 10 m, 30)").unwrap();
    let display = result.to_display();
    assert!(
        display.starts_with("[") && display.contains("m"),
        "Got display: {}",
        display
    );

    if let EvalResult::Interval(interval) = result {
        let lo = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        // mean diff = 10 m, Welch df ≈ 41.3, SE ≈ 3.51 m, t*(95%) ≈ 2.02, MoE ≈ 7.09 m -> [2.91 m, 17.09 m]
        assert!(
            (lo - 2.904_810_776_916_694).abs() < 1e-11,
            "lo should be ~2.90 m, got {}",
            lo
        );
        assert!(
            (hi - 17.095_189_223_083_306).abs() < 1e-11,
            "hi should be ~17.09 m, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── 2-SampZInt (TI-84 Option 9) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_2_samp_z_int() {
    // Tests 2-SampZInt(100 m, 15 m, 50, 90 m, 10 m, 50)
    let result = eval("2-SampZInt(100 m, 15 m, 50, 90 m, 10 m, 50)").unwrap();
    if let EvalResult::Interval(interval) = result {
        let lo = (interval.lo.canonical - interval.lo.unit.offset) / interval.lo.unit.scalar;
        let hi = (interval.hi.canonical - interval.hi.unit.offset) / interval.hi.unit.scalar;
        // mean diff = 10 m, SE = sqrt(2.25 + 2.0) ≈ 2.55 m, z*(95%) ≈ 1.96, MoE ≈ 5.0 m -> [5.0 m, 15.0 m]
        assert!(
            (lo - 5.003_052_698_445_598).abs() < 1e-11,
            "lo should be ~5.0 m, got {}",
            lo
        );
        assert!(
            (hi - 14.996_947_301_554_401).abs() < 1e-11,
            "hi should be ~15.0 m, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── 2-PropZInt (TI-84 Option B) ──

#[cfg(feature = "stats")]
#[test]
fn test_ti84_2_prop_z_int() {
    // Tests 2-PropZInt(45, 100, 30, 100)
    let result = eval("2-PropZInt(45, 100, 30, 100)").unwrap();
    if let EvalResult::Interval(interval) = result {
        let lo = interval.lo.canonical;
        let hi = interval.hi.canonical;
        // p1 = 0.45, p2 = 0.30, diff = 0.15, SE ≈ 0.0676, z*(95%) ≈ 1.96, MoE ≈ 0.132 -> [0.018, 0.282]
        assert!(
            (lo - 0.017_430_493_307_564_77).abs() < 1e-14,
            "lo should be ~0.0174, got {}",
            lo
        );
        assert!(
            (hi - 0.282_569_506_692_435_2).abs() < 1e-14,
            "hi should be ~0.2826, got {}",
            hi
        );
    } else {
        panic!("Expected interval result");
    }
}

// ── Margin of Error (moe, tmoe, zmoe) ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_moe_scalar_margin_of_error() {
    let moe_val = eval_scalar("moe(10 m, 12 m, 11 m, 14 m)").unwrap();
    assert_eq!(moe_val.unit.display.render(), "m");
    assert!((moe_val.canonical - 2.717_530_883_796_017).abs() < 1e-12);

    let tmoe_val = eval_scalar("tmoe(15 m, 25)").unwrap();
    assert_eq!(tmoe_val.unit.display.render(), "m");
    assert!((tmoe_val.canonical - 6.191_695_684_884_078).abs() < 1e-12);

    let zmoe_val = eval_scalar("zmoe(15 m, 100)").unwrap();
    assert_eq!(zmoe_val.unit.display.render(), "m");
    assert!((zmoe_val.canonical - 2.939_945_976_810_081).abs() < 1e-13);
}

// ── Interval arithmetic with TI-84 Confidence Intervals ──

#[cfg(all(feature = "units", feature = "stats"))]
#[test]
fn test_ti84_interval_with_arithmetic() {
    // TInterval(10 m, 12 m, 11 m, 14 m) * 2
    let result = eval("TInterval(10 m, 12 m, 11 m, 14 m) * 2").unwrap();
    let display = result.to_display();
    assert!(
        display.starts_with("[") && display.contains("m"),
        "Got display: {}",
        display
    );
}
