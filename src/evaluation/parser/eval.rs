//! Evaluates the syntax tree built by [`super::Parser`].

use std::{collections::HashMap, sync::Arc};

use crate::{
    AbacusError, BinaryOp, Date, UnitRegistry,
    evaluation::{
        parser::{
            ast::{
                After, Arg, Ast, At, Convert, Kind, Led, LedKind, Located, NodeId, RelPrefix,
                Slice, Target,
            },
            config::EvalConfig,
            pratt::{STACK_BUDGET_BYTES, stack_position},
            range::RangeSeq,
        },
        tokenizer::registry::{
            binary::arithmetic::{OpKind, check_overflow, op_kind},
            constants::get_standard_constant,
            function::operators::FunctionContext,
        },
    },
    units::{
        dimensions::Dimensions,
        eval_result::EvalResult,
        interval::{Interval, IntervalStyle},
        unit::{Unit, UnitExpr},
        value::Value,
    },
};

/// Walks a syntax tree, producing its value.
pub(crate) struct Evaluator<'a> {
    ast: &'a Ast,
    units: &'a UnitRegistry,
    variables: Option<&'a HashMap<String, EvalResult>>,
    config: &'a EvalConfig,
    /// The reference "now", resolved at most once per evaluation.
    now: Option<Date>,
    /// Values expanded from `a..b` ranges in function arguments so far.
    range_elements: usize,
    /// Stack position at the outermost `eval` call (0 until evaluation starts).
    stack_base: usize,
}

impl<'a> Evaluator<'a> {
    pub(crate) fn new(
        ast: &'a Ast,
        units: &'a UnitRegistry,
        variables: Option<&'a HashMap<String, EvalResult>>,
        config: &'a EvalConfig,
    ) -> Self {
        Self {
            ast,
            units,
            variables,
            config,
            now: config.anchor_date.clone(),
            range_elements: 0,
            stack_base: 0,
        }
    }

    fn now(&mut self) -> &Date {
        let tz = self.config.default_timezone.as_ref();
        self.now
            .get_or_insert_with(|| Date::reference_now(None, tz))
    }

    fn weekend(&self) -> crate::WeekendDays {
        self.config.weekend
    }

    /// Evaluates `node`, attaching its span to errors that don't have a more precise one.
    pub(crate) fn eval(&mut self, id: NodeId) -> Result<EvalResult, Located> {
        let node = self.ast.node(id);
        let here = stack_position();
        if self.stack_base == 0 {
            self.stack_base = here;
        } else if self.stack_base.abs_diff(here) > STACK_BUDGET_BYTES {
            return Err(AbacusError::RecursionLimitExceeded).at(node.span);
        }
        self.eval_kind(&node.kind).at(node.span)
    }

    fn span(&self, id: NodeId) -> crate::Span {
        self.ast.node(id).span
    }

    fn eval_kind(&mut self, kind: &'a Kind) -> Result<EvalResult, Located> {
        Ok(match kind {
            Kind::Value(v) => EvalResult::Scalar(v.clone()),
            Kind::Number(n) => EvalResult::Scalar(Value::dimensionless(*n)),
            Kind::Unit(unit) => EvalResult::Scalar(Value::new(1.0, Arc::clone(unit))),
            Kind::Ident(name) => {
                if let Some(val) = self.variables.and_then(|vars| vars.get(&**name)) {
                    val.clone()
                } else if let Some(val) = get_standard_constant(name) {
                    val
                } else {
                    return Err(AbacusError::UndefinedVariable(name.to_string()).into());
                }
            }
            Kind::Date(d) => {
                let mut d = (**d).clone();
                if d.timezone.is_none() {
                    d.timezone = self.config.default_timezone.clone();
                }
                EvalResult::Date(d)
            }
            Kind::Interval { lo, hi } => {
                let endpoint = |res: EvalResult| {
                    res.into_scalar().map_err(|_| {
                        AbacusError::UnexpectedToken(
                            "interval endpoints must be scalar values".to_string(),
                        )
                    })
                };
                let lo_val = endpoint(self.eval(*lo)?).at(self.span(*lo))?;
                let hi_val = endpoint(self.eval(*hi)?).at(self.span(*hi))?;
                self.check_strict_endpoints(&lo_val, &hi_val)?;
                let style = self
                    .config
                    .default_interval_style
                    .unwrap_or(IntervalStyle::Bracket);
                EvalResult::Interval(Interval::new_with_style(lo_val, hi_val, style)?)
            }
            Kind::Prefix { op, operand } => self.eval(*operand)?.apply_unary(op)?,
            Kind::Call { func, args } => self.eval_call(func, *args)?,
            Kind::PercentChange { from, to } => {
                let start = self.eval(*from)?.into_scalar().at(self.span(*from))?;
                let end = self.eval(*to)?.into_scalar().at(self.span(*to))?;
                if !start.unit.is_compatible_with(&end.unit) {
                    return Err(AbacusError::IncompatibleDimensions.into());
                }
                if start.canonical == 0.0 {
                    return Err(AbacusError::EvaluationError(
                        "division by zero in percentage change".to_string(),
                    )
                    .into());
                }
                let change = (end.canonical - start.canonical) / start.canonical;
                let pct_unit = Arc::new(Unit {
                    scalar: 0.01,
                    offset: 0.0,
                    dimensions: Dimensions::DIMENSIONLESS,
                    display: UnitExpr::single("+%"),
                });
                EvalResult::Scalar(Value {
                    canonical: change,
                    unit: pct_unit,
                })
            }
            Kind::RelPrefix { op, operand } => self.eval_rel_prefix(*op, *operand)?,
            Kind::Chain { head, ops } => {
                let mut acc = self.eval(*head)?;
                for led in self.ast.leds(*ops) {
                    acc = self.apply_led(acc, led).at(led.span)?;
                }
                acc
            }
        })
    }

    fn check_strict_endpoints(&self, lo: &Value, hi: &Value) -> Result<(), Located> {
        if self.config.strict_dimensions && lo.unit.is_dimensionless() != hi.unit.is_dimensionless()
        {
            return Err(AbacusError::IncompatibleDimensions.into());
        }
        Ok(())
    }

    // ── Prefix forms ──

    fn eval_rel_prefix(&mut self, op: RelPrefix, operand: NodeId) -> Result<EvalResult, Located> {
        let rhs = self.eval(operand)?;
        if op == RelPrefix::Until {
            let EvalResult::Date(target) = rhs else {
                return Err(AbacusError::EvaluationError(
                    "expected a date after 'until'".to_string(),
                ))
                .at(self.span(operand));
            };
            let days = target.to_epoch_days() - self.now().to_epoch_days();
            let day = self.units.unit("d")?;
            return Ok(EvalResult::Scalar(Value::new(days as f64, day)));
        }
        Ok(match rhs {
            EvalResult::Scalar(v) if v.unit.dimensions == Dimensions::TIME => {
                let sign = if op == RelPrefix::Backward { -1 } else { 1 };
                let weekend = self.weekend();
                EvalResult::Date(self.now().apply_time_value_with(&v, sign, weekend)?)
            }
            other => other,
        })
    }

    // ── Function calls ──

    fn eval_call(&mut self, func: &crate::FunctionOp, args: Slice) -> Result<EvalResult, Located> {
        let args = self.ast.args(args);
        let mut raw_args = Vec::with_capacity(args.len());
        for arg in args {
            match *arg {
                Arg::Expr(node) => raw_args.push(self.eval(node)?),
                Arg::Range { start, end, step } => {
                    let mut scalar = |node: NodeId| -> Result<Value, Located> {
                        let span = self.span(node);
                        self.eval(node)?
                            .into_scalar()
                            .map_err(|_| AbacusError::IntervalInFunction)
                            .at(span)
                    };
                    let start_val = scalar(start)?;
                    let end_val = scalar(end)?;
                    let step_val = step.map(&mut scalar).transpose()?;
                    let seq = RangeSeq::new(start_val, end_val, step_val).at(self.span(start))?;
                    // The element limit applies to the whole evaluation, not to each range, so
                    // `sum(1..99999, 1..99999, ...)` cannot exhaust memory.
                    self.range_elements = self.range_elements.saturating_add(seq.count);
                    if self.range_elements > RangeSeq::MAX_RANGE_ELEMENTS {
                        return Err(AbacusError::IncompatibleFunctionArguments)
                            .at(self.span(start));
                    }
                    raw_args.extend(seq.iter().map(EvalResult::Scalar));
                }
            }
        }

        let ctx = FunctionContext::new(self.units)
            .with_angle_mode(self.config.angle_mode)
            .with_weekend(self.config.weekend)
            .with_default_timezone(self.config.default_timezone.as_ref());
        let mut res = func.apply_with(&ctx, &raw_args)?;
        if let EvalResult::Date(ref mut d) = res
            && d.timezone.is_none()
        {
            d.timezone = self.config.default_timezone.clone();
        }
        Ok(res)
    }

    // ── Infix and postfix operations ──

    fn apply_led(&mut self, acc: EvalResult, led: &'a Led) -> Result<EvalResult, Located> {
        Ok(match &led.kind {
            LedKind::PercentTag(tag) => {
                // A tag is only meaningful after a percentage (`20% off`); `5 off` is an error
                // rather than silently ignoring the word.
                if !acc.as_scalar().is_ok_and(|v| v.unit.is_percent()) {
                    return Err(AbacusError::UnexpectedToken((*tag).to_string()).into());
                }
                acc
            }
            LedKind::Postfix(op) => acc.apply_unary(op)?,
            LedKind::Binary { op, rhs } => {
                let rhs_val = self.eval(*rhs)?;
                self.eval_binary(op, acc, rhs_val)?
            }
            LedKind::Range { hi } => {
                let endpoint = |res: EvalResult| {
                    res.into_scalar().map_err(|_| {
                        AbacusError::UnexpectedToken(
                            "range endpoints must be scalar values".to_string(),
                        )
                    })
                };
                let hi_res = self.eval(*hi)?;
                let lo = endpoint(acc)?;
                let hi_val = endpoint(hi_res).at(self.span(*hi))?;
                self.check_strict_endpoints(&lo, &hi_val)?;
                let style = self
                    .config
                    .default_interval_style
                    .unwrap_or(IntervalStyle::Range);
                EvalResult::Interval(Interval::new_with_style(lo, hi_val, style)?)
            }
            LedKind::Convert(convert) => self.eval_conversion(acc, convert)?,
            LedKind::Ago => {
                let val = time_value(acc)?;
                let weekend = self.weekend();
                EvalResult::Date(self.now().apply_time_value_with(&val, -1, weekend)?)
            }
            LedKind::FromNow => {
                let val = time_value(acc)?;
                let weekend = self.weekend();
                EvalResult::Date(self.now().apply_time_value_with(&val, 1, weekend)?)
            }
            LedKind::Before { date } => {
                let val = time_value(acc)?;
                let reference = self.reference_date(*date)?;
                EvalResult::Date(reference.apply_time_value_with(&val, -1, self.weekend())?)
            }
            LedKind::After(after) => self.eval_after(acc, after)?,
            LedKind::Until { target } => self.eval_until(acc, *target)?,
            LedKind::Property(prop) => property(acc, prop, self.weekend())?,
        })
    }

    /// The date `node` evaluates to, or now if there is none or it isn't a date.
    fn reference_date(&mut self, node: Option<NodeId>) -> Result<Date, Located> {
        if let Some(node) = node
            && let EvalResult::Date(d) = self.eval(node)?
        {
            return Ok(d);
        }
        Ok(self.now().clone())
    }

    fn eval_binary(
        &self,
        op: &BinaryOp,
        lhs: EvalResult,
        rhs: EvalResult,
    ) -> Result<EvalResult, Located> {
        let kind = op_kind(op);

        if self.config.strict_dimensions
            && matches!(kind, Some(OpKind::Add | OpKind::Sub))
            && let (Ok(l), Ok(r)) = (lhs.as_scalar(), rhs.as_scalar())
            && l.unit.is_dimensionless() != r.unit.is_dimensionless()
            && !r.unit.is_percent()
        {
            return Err(AbacusError::IncompatibleDimensions.into());
        }

        // Fast path for built-in scalar arithmetic (custom operators never take it, even if
        // registered under `+`).
        if let Some(kind @ (OpKind::Add | OpKind::Sub | OpKind::Mul | OpKind::Div)) = kind
            && let (EvalResult::Scalar(l), EvalResult::Scalar(r)) = (&lhs, &rhs)
        {
            let (x, y) = (l.canonical, r.canonical);
            let snap = self.config.snap_cancellation;
            let result = match kind {
                OpKind::Add => l.add_with(r, snap),
                OpKind::Sub => l.sub_with(r, snap),
                OpKind::Mul => l * r,
                _ => l / r,
            }
            .map_err(|e| with_units_note(e, op, &lhs, &rhs))?;
            // Division by zero yields ±∞ by design; only genuine overflow is an error.
            if kind == OpKind::Div && y == 0.0 {
                return Ok(EvalResult::Scalar(result));
            }
            return check_overflow(result, x, y)
                .map(EvalResult::Scalar)
                .map_err(Located::from);
        }

        if kind == Some(OpKind::Pow) {
            let max = self.config.max_exponent;
            let too_big = match &rhs {
                EvalResult::Scalar(r) => r.canonical.abs() > max,
                EvalResult::Interval(i) => i.lo.canonical.abs() > max || i.hi.canonical.abs() > max,
                _ => false,
            };
            if too_big {
                return Err(AbacusError::ExponentLimitExceeded.into());
            }
        }

        let note_units = (lhs.unit().clone(), rhs.unit().clone());
        lhs.apply_binary_with_weekend(op, rhs, self.config.weekend)
            .map_err(|error| units_note(error, op, &note_units.0, &note_units.1))
    }

    // ── Relative time ──

    /// `x after y`: date arithmetic when `x` is a duration, otherwise a percentage increase
    /// (`$50 after 15% tax`) or decrease (`$50 after 20% off`).
    fn eval_after(&mut self, acc: EvalResult, after: &After) -> Result<EvalResult, Located> {
        let is_amount = acc
            .as_scalar()
            .is_ok_and(|v| v.unit.dimensions != Dimensions::TIME);
        if is_amount {
            let val = acc.into_scalar()?;
            let Some(rhs) = after.rhs else {
                return Err(AbacusError::UnexpectedEnd.into());
            };
            let pct = self.eval(rhs)?.into_scalar().at(self.span(rhs))?;
            if !pct.unit.is_percent() {
                return Err(AbacusError::IncompatibleDimensions).at(self.span(rhs));
            }
            let tag = after
                .tag
                .map(|(tag, _)| tag)
                .or_else(|| self.ast.last_percent_tag(rhs));
            let factor = if matches!(tag, Some("discount" | "off")) {
                1.0 - pct.canonical
            } else {
                1.0 + pct.canonical
            };
            return Ok(EvalResult::Scalar(Value {
                canonical: val.canonical * factor,
                unit: val.unit,
            }));
        }

        let val = time_value(acc)?;
        if let Some((tag, span)) = after.tag {
            return Err(AbacusError::UnexpectedToken(tag.to_string())).at(span);
        }
        let reference = match after.rhs {
            Some(rhs) if after.rhs_is_date => self.reference_date(Some(rhs))?,
            Some(rhs) => {
                return Err(AbacusError::UnexpectedToken(
                    "expected a date after 'after'".to_string(),
                ))
                .at(self.span(rhs));
            }
            None => self.now().clone(),
        };
        Ok(EvalResult::Date(reference.apply_time_value_with(
            &val,
            1,
            self.weekend(),
        )?))
    }

    /// `x until <date>`: the time from now (or from `x`, if it's a date) to the date, in `x`'s
    /// unit.
    fn eval_until(&mut self, acc: EvalResult, target: NodeId) -> Result<EvalResult, Located> {
        let EvalResult::Date(target_date) = self.eval(target)? else {
            return Err(AbacusError::EvaluationError(
                "expected a date after 'until'".to_string(),
            ))
            .at(self.span(target));
        };

        if let EvalResult::Date(from) = acc {
            let days = target_date.to_epoch_days() - from.to_epoch_days();
            let day = self.units.unit("d")?;
            return Ok(EvalResult::Scalar(Value::new(days as f64, day)));
        }

        let val = acc.into_scalar()?;
        if val.unit.dimensions != Dimensions::TIME {
            return Err(AbacusError::IncompatibleDimensions.into());
        }
        let now = self.now().clone();
        if val.unit.is_business_day_unit() {
            let bdays = now.business_days_between_with(&target_date, self.weekend());
            return Ok(EvalResult::Scalar(Value::new(
                bdays as f64,
                Arc::clone(&val.unit),
            )));
        }
        let unit_str = val.unit.display.render();
        if matches!(unit_str.as_str(), "d" | "days" | "day") {
            let days = target_date.to_epoch_days() - now.to_epoch_days();
            return Ok(EvalResult::Scalar(Value::new(
                days as f64,
                Arc::clone(&val.unit),
            )));
        }
        let seconds = now.milliseconds_between(&target_date) as f64 / 1000.0;
        let in_seconds = Value::new(seconds, self.units.unit("s")?);
        Ok(EvalResult::Scalar(
            in_seconds.convert_to(Arc::clone(&val.unit))?,
        ))
    }

    // ── Conversions ──

    fn eval_conversion(
        &mut self,
        lhs: EvalResult,
        convert: &'a Convert,
    ) -> Result<EvalResult, Located> {
        if let Target::PercentOf(base) = &convert.target {
            let base_val = self.eval(*base)?.into_scalar().at(self.span(*base))?;
            let lhs_val = lhs.into_scalar()?;
            return Ok(EvalResult::Scalar(Value {
                canonical: lhs_val.canonical / base_val.canonical,
                unit: self.units.unit("%")?,
            }));
        }

        if let EvalResult::Date(d1) = &lhs {
            return self.convert_date(d1, &convert.target);
        }

        let (target, has_magnitude) = match &convert.target {
            Target::PercentOf(_) => unreachable!("handled above"),
            Target::BareHour(h) => (EvalResult::Scalar(Value::dimensionless(*h)), true),
            Target::Date(d) => (EvalResult::Date((**d).clone()), false),
            Target::Zone { unit, .. } => {
                (EvalResult::Scalar(Value::new(1.0, unit.clone()?)), false)
            }
            Target::Expr {
                node,
                has_magnitude,
            } => (self.eval(*node)?, *has_magnitude),
        };

        // Rate accumulation over an explicit duration (e.g. "5 usd per second in 20 days" ->
        // $8640000). A bare unit target (`5 m/s in s`, `1 kHz in ms`) is a plain conversion and
        // must fail on incompatible dimensions rather than silently multiplying.
        if has_magnitude
            && target.unit().dimensions == Dimensions::TIME
            && lhs.unit().dimensions.0[2] < 0
        {
            return self.accumulate_rate_over_time(&lhs, &target);
        }

        let target_unit = Arc::clone(target.unit());
        // Only a unitless expression may take on the target unit (`15 to kg`, `x in h`). When
        // the units were written and cancelled (`10 m / 5 m in kg`, `sin(30 deg) to m`), the
        // conversion makes no dimensional sense.
        if convert.lhs_has_unit
            && matches!(lhs, EvalResult::Scalar(_) | EvalResult::Interval(_))
            && !lhs.unit().is_compatible_with(&target_unit)
            && lhs.unit().is_dimensionless()
        {
            return Err(AbacusError::IncompatibleDimensions.into());
        }
        Ok(lhs.convert_to(target_unit)?)
    }

    /// Conversions of a date: to another date or time (the span between them), a bare hour on
    /// the same day, or a timezone.
    fn convert_date(&mut self, d1: &Date, target: &'a Target) -> Result<EvalResult, Located> {
        match target {
            Target::PercentOf(_) => unreachable!("handled by the caller"),
            // A bare hour after a date-time (`today at 12 to 1`) is a time on the same date.
            Target::BareHour(h) => {
                let hour = *h as u32;
                let mut d2 = d1.start_of_day();
                d2.time = crate::units::date::Time::new(hour % 24, 0, 0, 0);
                d2.flags = crate::units::date::FLAG_TIME_ONLY;
                if hour == 24 {
                    d2 = d2.add_days(1);
                }
                Ok(EvalResult::Scalar(time_span(d1, d2)))
            }
            Target::Date(d2) => {
                let mut d2 = (**d2).clone();
                if d2.timezone.is_none() {
                    d2.timezone = if d2.flags & crate::units::date::FLAG_TIME_ONLY != 0 {
                        d1.timezone.clone()
                    } else {
                        self.config.default_timezone.clone()
                    };
                }
                Ok(EvalResult::Scalar(time_span(d1, d2)))
            }
            Target::Zone { tz, .. } => Ok(EvalResult::Date(d1.to_timezone(tz))),
            Target::Expr { node, .. } => match self.eval(*node)? {
                EvalResult::Date(ref d2) => Ok(EvalResult::Scalar(d2 - d1)),
                other => {
                    let target_str = other.unit().display.render();
                    let tz = crate::units::date::TimeZone::parse(&target_str).map_err(|_| {
                        AbacusError::EvaluationError(format!(
                            "cannot convert a date to '{target_str}': expected a timezone or a \
                             date (subtract dates to get a duration)"
                        ))
                    });
                    Ok(EvalResult::Date(d1.to_timezone(&tz.at(self.span(*node))?)))
                }
            },
        }
    }

    fn accumulate_rate_over_time(
        &self,
        lhs: &EvalResult,
        target: &EvalResult,
    ) -> Result<EvalResult, Located> {
        if let (EvalResult::Scalar(rate), EvalResult::Scalar(duration)) = (lhs, target) {
            return Ok(EvalResult::Scalar(
                self.accumulate_rate_scalar(rate, duration)?,
            ));
        }
        let (rate_lo, rate_hi, rate_style) = match lhs {
            EvalResult::Scalar(r) => (r, r, None),
            EvalResult::Interval(inv) => (&inv.lo, &inv.hi, Some(inv.style)),
            _ => return Err(AbacusError::IncompatibleDimensions.into()),
        };
        let (dur_lo, dur_hi, dur_style) = match target {
            EvalResult::Scalar(d) => (d, d, None),
            EvalResult::Interval(inv) => (&inv.lo, &inv.hi, Some(inv.style)),
            _ => return Err(AbacusError::IncompatibleDimensions.into()),
        };
        let lo = self.accumulate_rate_scalar(rate_lo, dur_lo)?;
        let hi = self.accumulate_rate_scalar(rate_hi, dur_hi)?;
        Ok(EvalResult::Interval(Interval {
            lo,
            hi,
            style: rate_style.or(dur_style).unwrap_or_default(),
        }))
    }

    fn accumulate_rate_scalar(&self, rate: &Value, duration: &Value) -> Result<Value, AbacusError> {
        if duration.unit.dimensions != Dimensions::TIME || rate.unit.dimensions.0[2] >= 0 {
            return Err(AbacusError::IncompatibleDimensions);
        }

        let total_canonical = rate.canonical * duration.canonical;
        let new_dimensions = rate.unit.dimensions + duration.unit.dimensions;

        let time_denom_idx = rate.unit.display.denominator.iter().position(|sym| {
            self.units
                .unit(sym)
                .is_ok_and(|u| u.dimensions == Dimensions::TIME)
        });

        // Without a single time unit to cancel in the denominator (e.g. `kHz`, `m/s^2`), fall
        // back to plain multiplication so the resulting display always matches its dimensions.
        let Some(idx) = time_denom_idx else {
            return rate * duration;
        };
        let mut new_denom = rate.unit.display.denominator.clone();
        let removed_sym = new_denom.remove(idx);
        let time_scalar = self.units.unit(&removed_sym).map_or(1.0, |u| u.scalar);
        let remaining_scalar = rate.unit.scalar * time_scalar;

        let composite = |numerator, denominator| {
            Arc::new(Unit {
                scalar: remaining_scalar,
                offset: 0.0,
                dimensions: new_dimensions,
                display: UnitExpr {
                    numerator,
                    denominator,
                },
            })
        };
        let unit = if new_denom.is_empty() && rate.unit.display.numerator.len() == 1 {
            let sym = &rate.unit.display.numerator[0];
            match self.units.unit(sym) {
                Ok(reg_unit)
                    if reg_unit.dimensions == new_dimensions
                        && (reg_unit.scalar - remaining_scalar).abs() < 1e-9 =>
                {
                    reg_unit
                }
                _ => composite(rate.unit.display.numerator.clone(), new_denom),
            }
        } else if new_denom.is_empty() && rate.unit.display.numerator.is_empty() {
            Unit::dimensionless_arc()
        } else {
            composite(rate.unit.display.numerator.clone(), new_denom)
        };

        Ok(Value {
            canonical: total_canonical,
            unit,
        })
    }
}

/// `acc` as a duration (for `ago`, `before`, ...).
fn time_value(acc: EvalResult) -> Result<Value, AbacusError> {
    let val = acc.into_scalar()?;
    if val.unit.dimensions != Dimensions::TIME {
        return Err(AbacusError::IncompatibleDimensions);
    }
    Ok(val)
}

/// `.prop` on a hash (`linreg(...).slope`) or a date (`today.year`).
fn property(
    acc: EvalResult,
    prop: &str,
    weekend: crate::WeekendDays,
) -> Result<EvalResult, AbacusError> {
    match acc {
        EvalResult::Hash(hash) => {
            let val = hash
                .get(prop)
                .or_else(|| match prop {
                    "m" => hash.get("slope"),
                    "b" => hash.get("intercept"),
                    "R2" | "r_squared" => hash.get("r2"),
                    "R" => hash.get("r"),
                    "SE" | "std_err" => hash.get("se"),
                    "x" | "x_mean" => hash.get("mean_x"),
                    "y" | "y_mean" => hash.get("mean_y"),
                    _ => None,
                })
                .ok_or_else(|| {
                    AbacusError::UnexpectedToken(format!(
                        "unknown property '.{prop}' on Hash result"
                    ))
                })?;
            Ok(EvalResult::Scalar(val.clone()))
        }
        EvalResult::Date(d) => {
            let num = d.get_property_with(prop, weekend).ok_or_else(|| {
                AbacusError::UnexpectedToken(format!("unknown property '.{prop}' on Date"))
            })?;
            Ok(EvalResult::Scalar(Value::dimensionless(num)))
        }
        _ => Err(AbacusError::UnexpectedToken(format!(
            "cannot access property '.{prop}' on this result type"
        ))),
    }
}

/// Adds a note naming both sides' units to a dimension mismatch (`m + s`).
fn units_note(error: AbacusError, op: &BinaryOp, lhs: &Unit, rhs: &Unit) -> Located {
    let note = matches!(error, AbacusError::IncompatibleDimensions).then(|| {
        let name = |u: &Unit| {
            let s = u.display.render();
            if s.is_empty() {
                "a number".to_string()
            } else {
                s
            }
        };
        format!("{} {} {}", name(lhs), op.alias, name(rhs))
    });
    Located::new(error, None, note)
}

fn with_units_note(
    error: AbacusError,
    op: &BinaryOp,
    lhs: &EvalResult,
    rhs: &EvalResult,
) -> Located {
    units_note(error, op, lhs.unit(), rhs.unit())
}

/// Duration of the span `start to end`. When `end` is a bare time of day (e.g. `3:00`, `1am`) that
/// would fall before `start` on the same date, the span rolls forward: if neither side used AM/PM
/// and both read as 12-hour clock times (e.g. `12:00 to 3:00`), `end` moves 12 hours later;
/// otherwise it moves to the next day (`11pm to 1am` = 2 h, `13:00 to 09:00` = 20 h).
fn time_span(start: &Date, mut end: Date) -> Value {
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
