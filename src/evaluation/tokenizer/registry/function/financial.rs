use crate::{
    AbacusError, Value,
    evaluation::tokenizer::registry::function::{check_dimensionless, operators::FunctionOp},
};
use std::sync::Arc;

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
        let compound = (1.0 + rate).powf(nper);
        -(rate * (pv_val.canonical * compound + fv_canonical)) / (compound - 1.0)
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
        let compound = (1.0 + rate).powf(nper);
        -pv_canonical * compound - pmt_val.canonical * ((compound - 1.0) / rate)
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
        let compound = (1.0 + rate).powf(nper);
        (-fv_canonical - pmt_val.canonical * ((compound - 1.0) / rate)) / compound
    };

    Ok(Value {
        canonical: pv_canonical,
        unit: Arc::clone(&pmt_val.unit),
    })
}

/// npv(rate, cashflow1, cashflow2, ...)
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
        total_npv += cf.canonical / (1.0 + rate).powf(t);
    }

    Ok(Value {
        canonical: total_npv,
        unit: Arc::clone(&first_cashflow.unit),
    })
}

/// irr(cashflow0, cashflow1, cashflow2, ...)
fn irr_fn(args: &[Value]) -> Result<Value, AbacusError> {
    let first_cf = &args[0];
    for cf in args {
        if !cf.unit.is_compatible_with(&first_cf.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }
    }

    let cfs: Vec<f64> = args.iter().map(|v| v.canonical).collect();

    // Helper to compute NPV at rate r
    let npv_at = |r: f64| -> f64 {
        cfs.iter()
            .enumerate()
            .fold(0.0, |acc, (t, &cf)| acc + cf / (1.0 + r).powi(t as i32))
    };

    // Helper to compute derivative d(NPV)/dr
    let dnpv_at = |r: f64| -> f64 {
        cfs.iter().enumerate().skip(1).fold(0.0, |acc, (t, &cf)| {
            acc - (t as f64) * cf / (1.0 + r).powi((t + 1) as i32)
        })
    };

    let mut r = 0.1; // initial guess 10%
    let mut converged = false;
    for _ in 0..100 {
        let npv_val = npv_at(r);
        if npv_val.abs() < 1e-10 {
            converged = true;
            break;
        }
        let dnpv_val = dnpv_at(r);
        if dnpv_val.abs() < 1e-12 {
            break;
        }
        let next_r = r - npv_val / dnpv_val;
        if !next_r.is_finite() {
            break;
        }
        if (next_r - r).abs() < 1e-12 {
            r = next_r;
            if npv_at(r).abs() < 1e-6 {
                converged = true;
            }
            break;
        }
        r = next_r;
    }

    if !converged && (!r.is_finite() || npv_at(r).abs() >= 1e-6) {
        return Err(AbacusError::IncompatibleFunctionArguments);
    }

    Ok(Value::dimensionless(r))
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

    let amount_canonical = principal.canonical * (1.0 + rate / n).powf(n * time);

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
