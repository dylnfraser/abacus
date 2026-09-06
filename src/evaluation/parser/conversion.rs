use crate::{
    AbacusError, Token,
    evaluation::parser::pratt::Parser,
    units::{eval_result::EvalResult, unit::Unit, unit::UnitExpr, value::Value},
};
use std::sync::Arc;

impl<'a> Parser<'a> {
    pub(crate) fn hour_unit(&self) -> Arc<Unit> {
        self.unit_registry
            .unit("h")
            .or_else(|_| self.unit_registry.unit("hour"))
            .unwrap_or_else(|_| {
                Arc::new(Unit {
                    scalar: 3600.0,
                    offset: 0.0,
                    dimensions: crate::units::dimensions::Dimensions::TIME,
                    display: UnitExpr::single("h"),
                })
            })
    }

    pub(crate) fn parse_conversion(&mut self, lhs: EvalResult) -> Result<EvalResult, AbacusError> {
        self.advance();
        self.has_explicit_conversion = true;

        // Check for "% of <base>" or "a % of <base>"
        let is_pct_of = match (self.peek(), self.peek_next()) {
            (Some(Token::Unit(u)), Some(&Token::BinaryOp("of"))) => {
                let sym = *u;
                sym == "%" || sym == "percent" || sym == "pct"
            }
            (Some(Token::Val(v)), Some(&Token::BinaryOp("of"))) => v.unit.is_percent(),
            _ => false,
        };

        if is_pct_of {
            self.advance();
            self.advance();
            let base_result = self.parse_expr(1)?;
            let base_val = base_result.into_scalar()?;
            let lhs_val = lhs.into_scalar()?;
            let pct_unit = self.unit_registry.unit("%")?;
            let ratio = lhs_val.canonical / base_val.canonical;
            return Ok(EvalResult::Scalar(Value {
                canonical: ratio,
                unit: pct_unit,
            }));
        }

        if let EvalResult::Date(ref d1) = lhs {
            match self.peek() {
                Some(Token::Date(d2)) => {
                    let mut d2 = d2.clone();
                    self.advance();

                    // Infer 12-hour clock rollover if d2.time < d1.time on same date (e.g. 12:00 to 1:00 -> 12:00 to 13:00)
                    if d2.year == d1.year
                        && d2.month == d1.month
                        && d2.day == d1.day
                        && d2.time.hour < d1.time.hour
                        && d2.time.hour < 12
                    {
                        d2.time.hour += 12;
                    }

                    let unit_h = self.hour_unit();
                    let diff_val = (&d2 - d1).convert_to(unit_h)?;
                    return Ok(EvalResult::Scalar(diff_val));
                }
                Some(Token::Unit(u)) => {
                    let sym = *u;
                    if let Ok(tz) = crate::units::date::TimeZone::parse(sym) {
                        self.advance();
                        return Ok(EvalResult::Date(d1.to_timezone(&tz)));
                    }
                }
                _ => {}
            }
            let rhs_result = self.parse_expr(1)?;
            return match rhs_result {
                EvalResult::Date(ref d2) => {
                    let unit_h = self.hour_unit();
                    let diff_val = (d2 - d1).convert_to(unit_h)?;
                    Ok(EvalResult::Scalar(diff_val))
                }
                other => {
                    let target_str = other.unit().display.render();
                    let tz = crate::units::date::TimeZone::parse(&target_str)?;
                    Ok(EvalResult::Date(d1.to_timezone(&tz)))
                }
            };
        }

        // The RHS of a conversion must be a scalar unit expression
        let target_result = self.parse_expr(1)?;

        // Check for rate accumulation over a time duration (e.g. "5 usd per second in 20 days" -> $8640000)
        if target_result.unit().dimensions == crate::units::dimensions::Dimensions::TIME
            && lhs.unit().dimensions.0[2] < 0
        {
            return self.accumulate_rate_over_time(&lhs, &target_result);
        }

        let target_unit = target_result.unit().clone();
        lhs.convert_to(target_unit)
    }

    pub(crate) fn accumulate_rate_over_time(
        &self,
        lhs: &EvalResult,
        target_result: &EvalResult,
    ) -> Result<EvalResult, AbacusError> {
        if let (EvalResult::Scalar(rate), EvalResult::Scalar(duration)) = (lhs, target_result) {
            let res = self.accumulate_rate_scalar(rate, duration)?;
            return Ok(EvalResult::Scalar(res));
        }

        let (rate_lo, rate_hi, rate_style) = match lhs {
            EvalResult::Scalar(r) => (r, r, None),
            EvalResult::Interval(inv) => (&inv.lo, &inv.hi, Some(inv.style)),
            _ => return Err(AbacusError::IncompatibleDimensions),
        };
        let (dur_lo, dur_hi, dur_style) = match target_result {
            EvalResult::Scalar(d) => (d, d, None),
            EvalResult::Interval(inv) => (&inv.lo, &inv.hi, Some(inv.style)),
            _ => return Err(AbacusError::IncompatibleDimensions),
        };

        let lo = self.accumulate_rate_scalar(rate_lo, dur_lo)?;
        let hi = self.accumulate_rate_scalar(rate_hi, dur_hi)?;
        Ok(EvalResult::Interval(crate::units::interval::Interval {
            lo,
            hi,
            style: rate_style.or(dur_style).unwrap_or_default(),
        }))
    }

    pub(crate) fn accumulate_rate_scalar(
        &self,
        rate: &Value,
        duration: &Value,
    ) -> Result<Value, AbacusError> {
        if duration.unit.dimensions != crate::units::dimensions::Dimensions::TIME
            || rate.unit.dimensions.0[2] >= 0
        {
            return Err(AbacusError::IncompatibleDimensions);
        }

        let total_canonical = rate.canonical * duration.canonical;
        let new_dimensions = rate.unit.dimensions + duration.unit.dimensions;

        let time_denom_idx = rate.unit.display.denominator.iter().position(|sym| {
            self.unit_registry
                .unit(sym)
                .map(|u| u.dimensions == crate::units::dimensions::Dimensions::TIME)
                .unwrap_or(false)
        });

        let mut new_denom = rate.unit.display.denominator.clone();
        let time_scalar = if let Some(idx) = time_denom_idx {
            let removed_sym = new_denom.remove(idx);
            self.unit_registry
                .unit(&removed_sym)
                .map(|u| u.scalar)
                .unwrap_or(1.0)
        } else {
            1.0
        };

        let remaining_scalar = rate.unit.scalar * time_scalar;

        let unit = if new_denom.is_empty() && rate.unit.display.numerator.len() == 1 {
            let sym = &rate.unit.display.numerator[0];
            if let Ok(reg_unit) = self.unit_registry.unit(sym) {
                if reg_unit.dimensions == new_dimensions
                    && (reg_unit.scalar - remaining_scalar).abs() < 1e-9
                {
                    reg_unit
                } else {
                    Arc::new(Unit {
                        scalar: remaining_scalar,
                        offset: 0.0,
                        dimensions: new_dimensions,
                        display: UnitExpr {
                            numerator: rate.unit.display.numerator.clone(),
                            denominator: new_denom,
                        },
                    })
                }
            } else {
                Arc::new(Unit {
                    scalar: remaining_scalar,
                    offset: 0.0,
                    dimensions: new_dimensions,
                    display: UnitExpr {
                        numerator: rate.unit.display.numerator.clone(),
                        denominator: new_denom,
                    },
                })
            }
        } else if new_denom.is_empty() && rate.unit.display.numerator.is_empty() {
            Unit::dimensionless_arc()
        } else {
            Arc::new(Unit {
                scalar: remaining_scalar,
                offset: 0.0,
                dimensions: new_dimensions,
                display: UnitExpr {
                    numerator: rate.unit.display.numerator.clone(),
                    denominator: new_denom,
                },
            })
        };

        Ok(Value {
            canonical: total_canonical,
            unit,
        })
    }
}
