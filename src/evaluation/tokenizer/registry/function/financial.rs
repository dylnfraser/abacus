use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};
use std::sync::Arc;

/// `(1 + rate)^n`. When `1 + rate` is exactly representable (e.g. 0.5) `powf` is used as is;
/// otherwise `exp(n·ln_1p(rate))` avoids amplifying the rounding of `1 + rate` by `n`.
fn growth(rate: f64, n: f64) -> f64 {
    if rate > -1.0 && (1.0 + rate) - 1.0 != rate {
        (n * rate.ln_1p()).exp()
    } else {
        (1.0 + rate).powf(n)
    }
}

/// Annuity factor `((1 + rate)^n − 1) / rate`. When `(1 + rate)^n` is close to 1, subtracting 1
/// cancels catastrophically (`fv(1e-12, 360, -100)` came out as 36003.2 instead of
/// 36000.000006), so that regime uses `exp_m1`.
fn annuity_factor(rate: f64, n: f64) -> f64 {
    if rate > -1.0 {
        let x = n * rate.ln_1p();
        if x.abs() < 1.0 {
            return x.exp_m1() / rate;
        }
    }
    (growth(rate, n) - 1.0) / rate
}

/// pmt(rate, nper, pv) or pmt(rate, nper, pv, fv)
fn pmt_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(&args[0..2])?;
    let rate = args[0].canonical;
    let nper = args[1].canonical;
    let pv_val = &args[2];
    let fv_canonical = if args.len() == 4 {
        if !args[3].unit.is_compatible_with(&pv_val.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        args[3].canonical
    } else {
        0.0
    };

    if nper == 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let pmt_canonical = if rate == 0.0 {
        -(pv_val.canonical + fv_canonical) / nper
    } else {
        -(pv_val.canonical * growth(rate, nper) + fv_canonical) / annuity_factor(rate, nper)
    };

    Ok(Value {
        canonical: pmt_canonical,
        unit: Arc::clone(&pv_val.unit),
    })
}

/// fv(rate, nper, pmt, pv)
fn fv_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(&args[0..2])?;
    let rate = args[0].canonical;
    let nper = args[1].canonical;
    let pmt_val = &args[2];

    let pv_canonical = if args.len() == 4 {
        if !args[3].unit.is_compatible_with(&pmt_val.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        args[3].canonical
    } else {
        0.0
    };

    let fv_canonical = if rate == 0.0 {
        -(pv_canonical + pmt_val.canonical * nper)
    } else {
        -pv_canonical * growth(rate, nper) - pmt_val.canonical * annuity_factor(rate, nper)
    };

    Ok(Value {
        canonical: fv_canonical,
        unit: Arc::clone(&pmt_val.unit),
    })
}

/// pv(rate, nper, pmt, fv)
fn pv_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(&args[0..2])?;
    let rate = args[0].canonical;
    let nper = args[1].canonical;
    let pmt_val = &args[2];

    let fv_canonical = if args.len() == 4 {
        if !args[3].unit.is_compatible_with(&pmt_val.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        args[3].canonical
    } else {
        0.0
    };

    let pv_canonical = if rate == 0.0 {
        -(fv_canonical + pmt_val.canonical * nper)
    } else {
        (-fv_canonical - pmt_val.canonical * annuity_factor(rate, nper)) / growth(rate, nper)
    };

    Ok(Value {
        canonical: pv_canonical,
        unit: Arc::clone(&pmt_val.unit),
    })
}

/// npv(rate, cashflow1, cashflow2, ...)
///
/// Uses the Excel convention: the first cash flow is discounted by one period (t = 1).
/// To include an undiscounted initial outlay, add it outside: `cf0 + npv(rate, cf1, …)`.
fn npv_fn(args: &[Value]) -> Result<Value, AbacusError> {
    check_dimensionless(&args[0..1])?;
    let rate = args[0].canonical;
    let first_cashflow = &args[1];

    let mut total_npv = 0.0;
    for (i, cf) in args[1..].iter().enumerate() {
        if !cf.unit.is_compatible_with(&first_cashflow.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
        let t = (i + 1) as f64;
        total_npv += cf.canonical / growth(rate, t);
    }

    Ok(Value {
        canonical: total_npv,
        unit: Arc::clone(&first_cashflow.unit),
    })
}

/// NPV of `cfs` at rate `r`, with the first cash flow at t = 0.
fn npv_at(cfs: &[f64], r: f64) -> f64 {
    let factor = 1.0 / (1.0 + r);
    let mut discount = 1.0;
    let mut total = 0.0;
    for &cf in cfs {
        total += cf * discount;
        discount *= factor;
    }
    total
}

/// Σ |cf| / (1 + r)^t — the scale against which an NPV of "zero" is judged.
fn discounted_magnitude(cfs: &[f64], r: f64) -> f64 {
    let factor = 1.0 / (1.0 + r);
    let mut discount = 1.0;
    let mut total = 0.0;
    for &cf in cfs {
        total += cf.abs() * discount;
        discount *= factor;
    }
    total
}

/// d(NPV)/dr at rate `r`.
fn dnpv_at(cfs: &[f64], r: f64) -> f64 {
    let factor = 1.0 / (1.0 + r);
    let mut discount = factor;
    let mut total = 0.0;
    for (t, &cf) in cfs.iter().enumerate().skip(1) {
        discount *= factor;
        total -= (t as f64) * cf * discount;
    }
    total
}

/// Bisects a bracketed NPV root between rates `a` and `b`.
fn bisect_root(cfs: &[f64], a: f64, b: f64) -> f64 {
    let (mut lo, mut hi) = if a < b { (a, b) } else { (b, a) };
    let lo_sign = npv_at(cfs, lo).signum();
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if npv_at(cfs, mid).signum() == lo_sign {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// irr(cashflow0, cashflow1, cashflow2, ...) — internal rate of return.
///
/// The first cash flow is at t = 0. Requires at least one positive and one negative cash
/// flow. Newton's method is tried first from a 10% guess (Excel's default), so when several
/// rates make the NPV zero the root nearest 10% is typically returned. If Newton fails, the
/// rate axis is scanned for a sign change and the root is refined by bisection. An error is
/// returned if no root is found. Convergence is judged relative to the discounted cash-flow
/// magnitudes, so the result does not depend on the currency scale.
fn irr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_cf = &args[0];
    if args
        .iter()
        .any(|cf| !cf.unit.is_compatible_with(&first_cf.unit))
    {
        return Err(AbacusError::IncompatibleDimensions);
    }

    let cfs: Vec<f64> = args.iter().map(|v| v.canonical).collect();
    if !cfs.iter().any(|&c| c > 0.0) || !cfs.iter().any(|&c| c < 0.0) {
        // IRR needs at least one positive and one negative cash flow.
        return Err(AbacusError::IncompatibleFunctionArguments);
    }
    // Newton from 10%. Convergence is measured against the size of the *discounted* cash
    // flows at the current rate, so heavily discounted or tiny cash flows are handled alike.
    let mut r = 0.1;
    for _ in 0..100 {
        let f = npv_at(&cfs, r);
        if f == 0.0 {
            return Ok(Value::dimensionless(r));
        }
        let d = dnpv_at(&cfs, r);
        if d == 0.0 || !d.is_finite() {
            break;
        }
        let next = r - f / d;
        if !next.is_finite() || next <= -1.0 {
            break;
        }
        if (next - r).abs() <= 4.0 * f64::EPSILON * next.abs().max(1e-3) {
            if npv_at(&cfs, next).abs() <= 1e-9 * discounted_magnitude(&cfs, next) {
                return Ok(Value::dimensionless(next));
            }
            break;
        }
        r = next;
    }

    // 2. Sweep outwards from 10% — downwards towards -100% and upwards towards 1e6 — for a
    //    sign change of the NPV, then bisect it.
    let downward = std::iter::successors(Some(0.1_f64), |&x| Some(-1.0 + (x + 1.0) * 0.8))
        .take_while(|&x| x > -1.0 + 1e-12);
    let upward =
        std::iter::successors(Some(0.1_f64), |&x| Some(x * 1.25 + 0.01)).take_while(|&x| x < 1e6);
    let sweeps: [&mut dyn Iterator<Item = f64>; 2] = [&mut downward.skip(1), &mut upward.skip(1)];
    for sweep in sweeps {
        let (mut prev, mut f_prev) = (0.1, npv_at(&cfs, 0.1));
        for x in sweep {
            let f = npv_at(&cfs, x);
            if f.is_finite() && f_prev.is_finite() && f.signum() != f_prev.signum() {
                return Ok(Value::dimensionless(bisect_root(&cfs, prev, x)));
            }
            (prev, f_prev) = (x, f);
        }
    }

    Err(AbacusError::EvaluationError(
        "irr did not converge: no rate makes the NPV zero".to_string(),
    ))
}

/// compound(principal, rate, time) or compound(principal, rate, time, n)
fn compound_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let principal = &args[0];
    check_dimensionless(&args[1..=2])?;

    let rate = args[1].canonical;
    let time = args[2].canonical;
    let n = if args.len() == 4 {
        if !args[3].unit.is_dimensionless() {
            return Err(AbacusError::IncompatibleDimensions);
        }
        args[3].canonical
    } else {
        1.0 // annual compounding
    };

    if n <= 0.0 {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    let amount_canonical = principal.canonical * growth(rate / n, n * time);

    Ok(Value {
        canonical: amount_canonical,
        unit: Arc::clone(&principal.unit),
    })
}

pub fn register_financial() -> Vec<FunctionOp> {
    vec![
        FunctionOp::scalar("pmt", 3, 4, pmt_fn),
        FunctionOp::scalar("fv", 3, 4, fv_fn),
        FunctionOp::scalar("pv", 3, 4, pv_fn),
        FunctionOp::scalar("npv", 2, 255, npv_fn),
        FunctionOp::scalar("irr", 2, 255, irr_fn),
        FunctionOp::scalar("compound", 3, 4, compound_fn),
    ]
}
