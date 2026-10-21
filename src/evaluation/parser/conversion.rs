use crate::{
    AbacusError, Token,
    evaluation::parser::pratt::Parser,
    units::{eval_result::EvalResult, unit::Unit, unit::UnitExpr, value::Value},
};
use std::sync::Arc;

/// Duration of the span `start to end`. When `end` is a bare time of day (e.g. `3:00`, `1am`) that
/// would fall before `start` on the same date, the span rolls forward: if neither side used AM/PM
/// and both read as 12-hour clock times (e.g. `12:00 to 3:00`), `end` moves 12 hours later;
/// otherwise it moves to the next day (`11pm to 1am` = 2 h, `13:00 to 09:00` = 20 h).
fn time_span(start: &crate::Date, mut end: crate::Date) -> Value {
    use crate::units::date::{FLAG_EXPLICIT_MERIDIEM, FLAG_TIME_ONLY};
    if end.flags & FLAG_TIME_ONLY != 0 && end < *start {
        let explicit = (start.flags | end.flags) & FLAG_EXPLICIT_MERIDIEM != 0;
        let half_day = end.add_hours(12);
        end = if !explicit && start.time.hour <= 12 && end.time.hour < 12 && half_day >= *start {
            half_day
        } else {
            end.add_days(1)
        };
    }
    &end - start
}

impl<'a> Parser<'a> {
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
            // A bare hour after a date-time (`today at 12 to 1`) is a time on the same date.
            let bare_hour = match (self.peek(), self.peek_next()) {
                (
                    Some(Token::Float(h)),
                    None | Some(Token::ConversionOp | Token::CloseParen | Token::Comma),
                ) if h.fract() == 0.0 && (0.0..=24.0).contains(h) => Some(*h as u32),
                _ => None,
            };
            if let Some(hour) = bare_hour {
                self.advance();
                let mut d2 = d1.start_of_day();
                d2.time = crate::units::date::Time::new(hour % 24, 0, 0, 0);
                d2.flags = crate::units::date::FLAG_TIME_ONLY;
                if hour == 24 {
                    d2 = d2.add_days(1);
                }
                return Ok(EvalResult::Scalar(time_span(d1, d2)));
            }
            match self.peek() {
                Some(Token::Date(d2)) => {
                    let mut d2 = d2.clone();
                    self.advance();
                    if d2.timezone.is_none() {
                        d2.timezone = if d2.flags & crate::units::date::FLAG_TIME_ONLY != 0 {
                            d1.timezone.clone()
                        } else {
                            self.config.default_timezone.clone()
                        };
                    }
                    return Ok(EvalResult::Scalar(time_span(d1, d2)));
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
                EvalResult::Date(ref d2) => Ok(EvalResult::Scalar(d2 - d1)),
                other => {
                    let target_str = other.unit().display.render();
                    let tz = crate::units::date::TimeZone::parse(&target_str).map_err(|_| {
                        AbacusError::EvaluationError(format!(
                            "cannot convert a date to '{target_str}': expected a timezone or a \
                             date (subtract dates to get a duration)"
                        ))
                    })?;
                    Ok(EvalResult::Date(d1.to_timezone(&tz)))
                }
            };
        }

        // The RHS of a conversion must be a scalar unit expression
        let target_start = self.pos;
        let target_result = self.parse_expr(1)?;

        // Rate accumulation over an explicit duration (e.g. "5 usd per second in 20 days" ->
        // $8640000). A bare unit target (`5 m/s in s`, `1 kHz in ms`) is a plain conversion and
        // must fail on incompatible dimensions rather than silently multiplying.
        if self.target_has_magnitude(target_start)
            && target_result.unit().dimensions == crate::units::dimensions::Dimensions::TIME
            && lhs.unit().dimensions.0[2] < 0
        {
            return self.accumulate_rate_over_time(&lhs, &target_result);
        }

        let target_unit = target_result.unit().clone();
        lhs.convert_to(target_unit)
    }

    /// Returns true if the conversion target parsed from `start` contains an explicit number
    /// (e.g. `20 days`) rather than being a pure unit expression (`days`, `m^2/s`).
    fn target_has_magnitude(&self, start: usize) -> bool {
        let target = &self.tokens[start..self.pos];
        target.iter().enumerate().any(|(i, tok)| {
            matches!(tok, Token::Val(_))
                || (matches!(tok, Token::Float(_))
                    && !matches!(
                        i.checked_sub(1).map(|p| &target[p]),
                        Some(Token::BinaryOp("^"))
                    ))
        })
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

        // Without a single time unit to cancel in the denominator (e.g. `kHz`, `m/s^2`), fall back
        // to plain multiplication so the resulting display always matches its dimensions.
        let Some(idx) = time_denom_idx else {
            return rate * duration;
        };
        let mut new_denom = rate.unit.display.denominator.clone();
        let removed_sym = new_denom.remove(idx);
        let time_scalar = self
            .unit_registry
            .unit(&removed_sym)
            .map(|u| u.scalar)
            .unwrap_or(1.0);

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
