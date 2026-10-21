use crate::{
    AbacusError, Token,
    evaluation::parser::{pratt::Parser, range::RangeSeq},
    units::{
        dimensions::Dimensions,
        eval_result::EvalResult,
        unit::{Unit, UnitExpr},
        value::Value,
    },
};
use std::sync::Arc;

impl<'a> Parser<'a> {
    /// Rejects range endpoints such as `max(1..2*3)` whose top-level operators bind looser than
    /// `..`. At the top level `1..2*3` means `(1..2)*3`, so silently reading it as `1..(2*3)`
    /// inside a function call would give a different answer; parentheses are required instead.
    fn check_range_endpoint(&self, start: usize) -> Result<(), AbacusError> {
        const RANGE_BP: u8 = 6;
        let mut depth = 0usize;
        let mut prev_is_operand = false;
        for tok in &self.tokens[start..self.pos] {
            match tok {
                Token::OpenParen | Token::OpenBracket => {
                    depth += 1;
                    prev_is_operand = false;
                }
                Token::CloseParen | Token::CloseBracket => {
                    depth = depth.saturating_sub(1);
                    prev_is_operand = true;
                }
                Token::BinaryOp(name) => {
                    if depth == 0 && prev_is_operand && self.infix_bp(name).0 < RANGE_BP {
                        return Err(AbacusError::UnexpectedToken(format!(
                            "'{name}' in a range endpoint; parenthesize it, e.g. sum(1..(n * 2))"
                        )));
                    }
                    prev_is_operand = false;
                }
                Token::ConversionOp | Token::RelTimeOp(_) if depth == 0 => {
                    return Err(AbacusError::UnexpectedToken(
                        "conversion in a range endpoint; parenthesize it".to_string(),
                    ));
                }
                Token::UnaryOp(name) => {
                    // Postfix operators (`!`) keep the operand; prefix ones start a new one.
                    prev_is_operand = prev_is_operand
                        && self
                            .token_registry
                            .unary_operators
                            .get(*name)
                            .is_some_and(|op| !op.prefix);
                }
                Token::Function(_) | Token::Comma => prev_is_operand = false,
                _ => prev_is_operand = true,
            }
        }
        Ok(())
    }

    pub(crate) fn parse_function_call(
        &mut self,
        name: &'static str,
    ) -> Result<EvalResult, AbacusError> {
        let func = self
            .token_registry
            .function_operators
            .get(name)
            .ok_or_else(|| AbacusError::UnexpectedToken(name.to_string()))?;

        let mut raw_args = Vec::new();

        if self.peek() == Some(&Token::OpenParen) {
            self.advance(); // consume '('

            self.function_arg_depth += 1;
            let parse_args = (|| -> Result<(), AbacusError> {
                // Handle empty argument list: func()
                if self.peek() != Some(&Token::CloseParen) {
                    loop {
                        let arg_start = self.pos;
                        let arg_result = self.parse_expr(0)?;

                        if self.peek() == Some(&Token::Range) {
                            self.check_range_endpoint(arg_start)?;
                            let arg = arg_result
                                .into_scalar()
                                .map_err(|_| AbacusError::IntervalInFunction)?;
                            self.advance(); // consume `..`
                            let end_start = self.pos;
                            let end_result = self.parse_expr(0)?;
                            self.check_range_endpoint(end_start)?;
                            let end = end_result
                                .into_scalar()
                                .map_err(|_| AbacusError::IntervalInFunction)?;
                            let custom_step = if self.peek() == Some(&Token::Range) {
                                self.advance(); // consume second `..`
                                let step_start = self.pos;
                                let step_result = self.parse_expr(0)?;
                                self.check_range_endpoint(step_start)?;
                                Some(
                                    step_result
                                        .into_scalar()
                                        .map_err(|_| AbacusError::IntervalInFunction)?,
                                )
                            } else {
                                None
                            };
                            let seq = RangeSeq::new(arg, end, custom_step)?;
                            // The element limit applies to the whole evaluation, not to each
                            // range, so `sum(1..99999, 1..99999, ...)` cannot exhaust memory.
                            self.range_elements = self.range_elements.saturating_add(seq.count);
                            if self.range_elements > RangeSeq::MAX_RANGE_ELEMENTS {
                                return Err(AbacusError::IncompatibleFunctionArguments);
                            }
                            raw_args.extend(seq.iter().map(EvalResult::Scalar));
                        } else {
                            raw_args.push(arg_result);
                        }

                        if self.peek() == Some(&Token::Comma) {
                            self.advance(); // consume ','
                        } else {
                            break;
                        }
                    }
                }
                Ok(())
            })();
            self.function_arg_depth -= 1;
            parse_args?;

            self.expect(&Token::CloseParen)
                .map_err(|_| AbacusError::UnclosedParen)?;
        } else {
            // Single-parameter unparenthesized function call (e.g. `sin 13deg`, `cos 45 deg`)
            let bp = 8;
            let arg_result = self.parse_expr(bp)?;
            raw_args.push(arg_result);
        }

        if matches!(name, "is_workday" | "is_business_day") {
            if let Some(EvalResult::Date(d)) = raw_args.first() {
                let is_bday = d.is_business_day_with(self.config.weekend);
                return Ok(EvalResult::Scalar(Value::dimensionless(if is_bday {
                    1.0
                } else {
                    0.0
                })));
            }
        } else if name == "is_weekend" {
            if let Some(EvalResult::Date(d)) = raw_args.first() {
                let is_wknd = d.is_weekend_with(self.config.weekend);
                return Ok(EvalResult::Scalar(Value::dimensionless(if is_wknd {
                    1.0
                } else {
                    0.0
                })));
            }
        } else if matches!(name, "business_days" | "workdays")
            && raw_args.len() == 2
            && let (EvalResult::Date(d1), EvalResult::Date(d2)) = (&raw_args[0], &raw_args[1])
        {
            let bdays = d1.business_days_between_with(d2, self.config.weekend) as f64;
            let unit = Arc::new(Unit {
                scalar: 86400.0,
                offset: 0.0,
                dimensions: Dimensions::TIME,
                display: UnitExpr::single("workdays"),
            });
            return Ok(EvalResult::Scalar(Value::new(bdays, unit)));
        }

        if matches!(name, "sin" | "cos" | "tan") {
            for arg in &mut raw_args {
                if let EvalResult::Scalar(v) = arg
                    && v.unit.is_dimensionless()
                    && v.unit.display.is_empty()
                {
                    let scale = match self.config.angle_mode {
                        crate::AngleMode::Degrees => std::f64::consts::PI / 180.0,
                        crate::AngleMode::Gradians => std::f64::consts::PI / 200.0,
                        crate::AngleMode::Radians => 1.0,
                    };
                    v.canonical *= scale;
                }
            }
        }

        let mut res = func.apply(&raw_args)?;

        if let EvalResult::Date(ref mut d) = res
            && d.timezone.is_none()
        {
            d.timezone = self.config.default_timezone.clone();
        }

        if matches!(name, "asin" | "acos" | "atan" | "atan2")
            && let EvalResult::Scalar(ref v) = res
        {
            let target_unit = match self.config.angle_mode {
                crate::AngleMode::Degrees => Some(self.unit_registry.unit("deg")?),
                crate::AngleMode::Gradians => Some(self.unit_registry.unit("grad")?),
                crate::AngleMode::Radians => None,
            };
            if let Some(target) = target_unit {
                res = EvalResult::Scalar(v.convert_to(target)?);
            }
        }

        Ok(res)
    }
}
