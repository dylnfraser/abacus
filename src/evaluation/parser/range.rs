use crate::{AbacusError, Value};
use std::sync::Arc;

/// A streaming sequence generator for arithmetic ranges (e.g. `1..10` or `1..5..2`).
pub struct RangeSeq {
    pub start: f64,
    pub step: f64,
    pub count: usize,
    pub unit: Arc<crate::units::unit::Unit>,
}

impl RangeSeq {
    /// Maximum number of values permitted in range step expansion.
    pub const MAX_RANGE_ELEMENTS: usize = 100_000;

    pub fn new(start: Value, end: Value, custom_step: Option<Value>) -> Result<Self, AbacusError> {
        if !start.unit.is_compatible_with(&end.unit) {
            return Err(AbacusError::IncompatibleDimensions);
        }

        let start_val = start.canonical;
        let end_val = end.canonical;

        let step_abs = match custom_step {
            Some(step_val) => {
                if step_val.unit.is_compatible_with(&start.unit) {
                    step_val.canonical.abs()
                } else if step_val.unit.is_dimensionless() && !start.unit.is_dimensionless() {
                    let amount = step_val.amount();
                    (amount * start.unit.scalar).abs()
                } else {
                    return Err(AbacusError::IncompatibleDimensions);
                }
            }
            None => start.unit.scalar.abs(),
        };

        if !start_val.is_finite() || !end_val.is_finite() || step_abs.is_nan() || step_abs <= 0.0 {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }

        let diff = (end_val - start_val).abs();
        let epsilon = 1e-12 * step_abs.max(1.0);
        let count_f64 = ((diff + epsilon) / step_abs).floor();
        if !count_f64.is_finite()
            || count_f64 < 0.0
            || count_f64 > (Self::MAX_RANGE_ELEMENTS - 1) as f64
        {
            return Err(AbacusError::IncompatibleFunctionArguments);
        }
        let count = count_f64 as usize + 1;
        let step = if start_val <= end_val {
            step_abs
        } else {
            -step_abs
        };

        Ok(Self {
            start: start_val,
            step,
            count,
            unit: start.unit,
        })
    }

    /// Stream `Value` instances from this range on demand.
    pub fn iter(&self) -> impl Iterator<Item = Value> + '_ {
        let unit = &self.unit;
        let start = self.start;
        let step = self.step;
        (0..self.count).map(move |i| Value {
            canonical: start + (i as f64) * step,
            unit: Arc::clone(unit),
        })
    }
}
