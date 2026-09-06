use crate::{
    AbacusError, Token,
    evaluation::parser::pratt::Parser,
    units::{eval_result::EvalResult, interval::Interval, interval::IntervalStyle, value::Value},
};

impl<'a> Parser<'a> {
    /// Parse a prefix expression (NUD in Pratt terminology).
    pub(crate) fn parse_prefix(&mut self) -> Result<EvalResult, AbacusError> {
        match self.next_token() {
            Some(Token::Val(val)) => Ok(EvalResult::Scalar(val.clone())),

            Some(Token::Ident(name)) => {
                if let Some(vars) = self.variables
                    && let Some(val) = vars.get(*name)
                {
                    Ok(val.clone())
                } else if let Some(val) = Self::get_standard_constant(name) {
                    Ok(val)
                } else {
                    Err(AbacusError::UndefinedVariable(name.to_string()))
                }
            }

            Some(Token::Date(d)) => {
                let mut d = d.clone();
                if d.timezone.is_none() {
                    d.timezone = self.config.default_timezone.clone();
                }
                Ok(EvalResult::Date(d))
            }

            // Relative time / conversion in prefix position (e.g. `in 3 hours`, `before 07-08-2026`)
            Some(Token::ConversionOp) => {
                let rhs = self.parse_expr(4)?;
                match rhs {
                    EvalResult::Scalar(v)
                        if v.unit.dimensions == crate::units::dimensions::Dimensions::TIME =>
                    {
                        let ms = (v.canonical * 1000.0).round() as i64;
                        Ok(EvalResult::Date(self.get_now().add_milliseconds(ms)))
                    }
                    EvalResult::Date(d) => Ok(EvalResult::Date(d)),
                    other => Ok(other),
                }
            }

            Some(Token::RelTimeOp(name)) => {
                let name = *name;
                if name == "until" {
                    let rhs = self.parse_expr(4)?;
                    let target_date = match rhs {
                        EvalResult::Date(d) => d,
                        _ => {
                            return Err(AbacusError::EvaluationError(
                                "expected a date after 'until'".to_string(),
                            ));
                        }
                    };
                    let now = self.get_now().clone();
                    let days_diff = target_date.to_epoch_days() - now.to_epoch_days();
                    let day_unit = self.unit_registry.unit("d")?;
                    return Ok(EvalResult::Scalar(Value::new(days_diff as f64, day_unit)));
                }

                let rhs = self.parse_expr(4)?;
                match rhs {
                    EvalResult::Scalar(v)
                        if v.unit.dimensions == crate::units::dimensions::Dimensions::TIME =>
                    {
                        let ms = (v.canonical * 1000.0).round() as i64;
                        if name == "before" || name == "ago" {
                            Ok(EvalResult::Date(
                                self.get_now().add_milliseconds(ms.saturating_neg()),
                            ))
                        } else {
                            Ok(EvalResult::Date(self.get_now().add_milliseconds(ms)))
                        }
                    }
                    EvalResult::Date(d) => Ok(EvalResult::Date(d)),
                    other => Ok(other),
                }
            }

            Some(Token::PercentChangeFrom) => {
                let start_res = self.parse_expr(2)?;
                if let Some(Token::ConversionOp) = self.peek() {
                    self.advance();
                } else {
                    return Err(AbacusError::EvaluationError(
                        "expected 'to' in percentage change expression".to_string(),
                    ));
                }
                let end_res = self.parse_expr(2)?;

                let start_val = start_res.into_scalar()?;
                let end_val = end_res.into_scalar()?;

                if !start_val.unit.is_compatible_with(&end_val.unit) {
                    return Err(AbacusError::IncompatibleDimensions);
                }

                if start_val.canonical == 0.0 {
                    return Err(AbacusError::EvaluationError(
                        "division by zero in percentage change".to_string(),
                    ));
                }

                let change = (end_val.canonical - start_val.canonical) / start_val.canonical;
                let pct_unit = std::sync::Arc::new(crate::units::unit::Unit {
                    scalar: 0.01,
                    offset: 0.0,
                    dimensions: crate::units::dimensions::Dimensions::DIMENSIONLESS,
                    display: crate::units::unit::UnitExpr::single("+%"),
                });

                Ok(EvalResult::Scalar(Value {
                    canonical: change,
                    unit: pct_unit,
                }))
            }

            Some(Token::Unit(unit_sym)) => {
                let unit = self.unit_registry.unit(unit_sym)?;
                Ok(EvalResult::Scalar(Value::new(1.0, unit)))
            }

            Some(Token::Float(num)) => {
                // A bare float without a unit — wrap in dimensionless Value
                Ok(EvalResult::Scalar(Value::dimensionless(*num)))
            }

            // Grouped expression: ( expr )
            Some(Token::OpenParen) => {
                let prev_depth = self.function_arg_depth;
                self.function_arg_depth = 0;
                let val = self.parse_expr(0);
                self.function_arg_depth = prev_depth;
                let val = val?;
                self.expect(&Token::CloseParen)
                    .map_err(|_| AbacusError::UnclosedParen)?;
                Ok(val)
            }

            // Bracket interval syntax: [ lo , hi ]
            Some(Token::OpenBracket) => {
                let prev_depth = self.function_arg_depth;
                self.function_arg_depth = 0;

                let lo_result = self.parse_expr(0)?;
                let lo = lo_result.into_scalar().map_err(|_| {
                    AbacusError::UnexpectedToken(
                        "interval endpoints must be scalar values".to_string(),
                    )
                })?;

                self.expect(&Token::Comma).map_err(|_| {
                    AbacusError::UnexpectedToken("expected ',' between interval bounds".to_string())
                })?;

                let hi_result = self.parse_expr(0)?;
                let hi = hi_result.into_scalar().map_err(|_| {
                    AbacusError::UnexpectedToken(
                        "interval endpoints must be scalar values".to_string(),
                    )
                })?;

                self.function_arg_depth = prev_depth;

                self.expect(&Token::CloseBracket)
                    .map_err(|_| AbacusError::UnclosedBracket)?;

                if self.config.strict_dimensions {
                    let lo_dimless = lo.unit.is_dimensionless();
                    let hi_dimless = hi.unit.is_dimensionless();
                    if lo_dimless != hi_dimless {
                        return Err(AbacusError::IncompatibleDimensions);
                    }
                }

                let style = self
                    .config
                    .default_interval_style
                    .unwrap_or(IntervalStyle::Bracket);
                Ok(EvalResult::Interval(Interval::new_with_style(
                    lo, hi, style,
                )?))
            }

            Some(Token::BinaryOp("-")) => {
                let op = self
                    .token_registry
                    .unary_operators
                    .get("-")
                    .ok_or_else(|| AbacusError::UnexpectedToken("-".to_string()))?;
                let bp = self.prefix_bp("-");
                let operand = self.parse_expr(bp)?;
                operand.apply_unary(op)
            }

            Some(Token::UnaryOp(name)) => {
                let name = *name;
                let bp = self.prefix_bp(name);
                let operand = self.parse_expr(bp)?;
                if name == "sqrt"
                    && let EvalResult::Scalar(ref v) = operand
                    && v.unit.is_dimensionless()
                {
                    return Ok(EvalResult::Scalar(Value::dimensionless(v.canonical.sqrt())));
                }
                let op = self
                    .token_registry
                    .unary_operators
                    .get(name)
                    .ok_or_else(|| AbacusError::UnexpectedToken(name.to_string()))?;
                operand.apply_unary(op)
            }

            // Function call: name(arg1, arg2, ...) or unparenthesized single argument: name arg
            Some(Token::Function(name)) => {
                let name = *name;
                self.parse_function_call(name)
            }

            Some(tok) => Err(AbacusError::UnexpectedToken(format!("{tok:?}"))),
            None => Err(AbacusError::UnexpectedEnd),
        }
    }
}
