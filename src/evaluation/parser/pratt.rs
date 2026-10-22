use crate::{
    AbacusError, Token, TokenRegistry, UnitRegistry,
    evaluation::parser::config::EvalConfig,
    units::{eval_result::EvalResult, interval::Interval, interval::IntervalStyle, value::Value},
};

/// Default maximum parser recursion depth.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// Hard upper bound accepted by [`crate::Abacus::with_max_recursion_depth`]. Deeper limits are
/// clamped so that hostile input cannot overflow the stack.
pub const MAX_RECURSION_DEPTH_CAP: usize = 256;

/// Maximum stack the parser may consume for one evaluation. Frames are much larger in debug
/// builds, so in addition to the depth limit, recursion stops once this much stack has been used.
/// 1 MiB leaves headroom on the default 2 MiB stack of spawned threads.
const STACK_BUDGET_BYTES: usize = 1024 * 1024;

/// Approximate current stack position (address of a local).
#[inline(always)]
fn stack_position() -> usize {
    let marker = 0u8;
    std::ptr::from_ref(std::hint::black_box(&marker)) as usize
}

/// Pratt expression parser for mathematical, dimensional, and date expressions.
pub struct Parser<'a> {
    pub(crate) tokens: &'a [Token<'a>],
    pub(crate) pos: usize,
    pub(crate) token_registry: &'a TokenRegistry,
    pub(crate) unit_registry: &'a UnitRegistry,
    pub has_explicit_conversion: bool,
    pub(crate) function_arg_depth: usize,
    pub(crate) recursion_depth: usize,
    pub(crate) now: Option<crate::Date>,
    pub(crate) variables: Option<&'a std::collections::HashMap<String, EvalResult>>,
    pub(crate) last_percent_tag: Option<&'static str>,
    /// Total number of values expanded from `a..b` ranges in function arguments so far.
    pub(crate) range_elements: usize,
    /// Stack position at the outermost `parse_expr` call (0 until parsing starts).
    stack_base: usize,
    pub config: EvalConfig,
}

impl<'a> Parser<'a> {
    #[must_use]
    pub fn new(
        tokens: &'a [Token<'a>],
        token_registry: &'a TokenRegistry,
        unit_registry: &'a UnitRegistry,
    ) -> Self {
        Self::new_with_config(tokens, token_registry, unit_registry, EvalConfig::default())
    }

    #[must_use]
    pub fn new_with_config(
        tokens: &'a [Token<'a>],
        token_registry: &'a TokenRegistry,
        unit_registry: &'a UnitRegistry,
        config: EvalConfig,
    ) -> Self {
        Self::new_with_variables(tokens, token_registry, unit_registry, None, config)
    }

    #[must_use]
    pub fn new_with_variables(
        tokens: &'a [Token<'a>],
        token_registry: &'a TokenRegistry,
        unit_registry: &'a UnitRegistry,
        variables: Option<&'a std::collections::HashMap<String, EvalResult>>,
        config: EvalConfig,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            token_registry,
            unit_registry,
            has_explicit_conversion: false,
            function_arg_depth: 0,
            recursion_depth: 0,
            now: config.anchor_date.clone(),
            variables,
            last_percent_tag: None,
            range_elements: 0,
            stack_base: 0,
            config,
        }
    }

    /// Retrieve value for standard mathematical constants.
    pub(crate) fn get_standard_constant(name: &str) -> Option<EvalResult> {
        crate::evaluation::tokenizer::registry::constants::get_standard_constant(name)
    }

    /// Lazy accessor for anchor date/time. Only evaluated if relative dates are used.
    pub(crate) fn get_now(&mut self) -> &crate::Date {
        if self.now.is_none() {
            self.now = Some(crate::Date::reference_now(
                None,
                self.config.default_timezone.as_ref(),
            ));
        }
        self.now.as_ref().expect("now initialized")
    }

    /// Peek at the current token without consuming it.
    #[inline(always)]
    pub(crate) fn peek(&self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.pos)
    }

    /// Peek at the next token without consuming it.
    #[inline(always)]
    pub(crate) fn peek_next(&self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.pos + 1)
    }

    /// Consume the current token and advance.
    #[inline(always)]
    pub(crate) fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    /// Consume and return the current token.
    #[inline(always)]
    pub(crate) fn next_token(&mut self) -> Option<&'a Token<'a>> {
        if self.pos < self.tokens.len() {
            let tok = &self.tokens[self.pos];
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    /// Expect and consume a specific token, or return an error.
    pub(crate) fn expect(&mut self, expected: &Token<'a>) -> Result<(), AbacusError> {
        match self.peek() {
            Some(tok) if tok == expected => {
                self.advance();
                Ok(())
            }
            Some(tok) => Err(AbacusError::UnexpectedToken(format!("{tok:?}"))),
            None => Err(AbacusError::UnexpectedEnd),
        }
    }

    /// Check if the next token can start a date expression.
    pub(crate) fn can_start_date_expr(&self) -> bool {
        match self.peek() {
            Some(Token::Date(_)) | Some(Token::OpenParen) => true,
            Some(Token::Function(name)) => {
                matches!(*name, "date" | "today" | "tomorrow" | "yesterday" | "now")
            }
            _ => false,
        }
    }

    /// Entry point: parse the full expression at minimum binding power 0.
    pub fn parse(&mut self) -> Result<EvalResult, AbacusError> {
        let result = self.parse_expr(0)?;

        // Ensure all tokens were consumed
        if self.pos < self.tokens.len() {
            if let Some(tok) = self.peek() {
                return Err(AbacusError::UnexpectedToken(format!("{tok:?}")));
            }
            return Err(AbacusError::UnexpectedEnd);
        }

        Ok(result)
    }

    /// Core Pratt expression parser.
    pub(crate) fn parse_expr(&mut self, min_bp: u8) -> Result<EvalResult, AbacusError> {
        let limit = self.config.max_recursion_depth.min(MAX_RECURSION_DEPTH_CAP);
        if self.recursion_depth >= limit {
            return Err(AbacusError::RecursionLimitExceeded);
        }
        let here = stack_position();
        if self.recursion_depth == 0 {
            self.stack_base = here;
        } else if self.stack_base.abs_diff(here) > STACK_BUDGET_BYTES {
            return Err(AbacusError::RecursionLimitExceeded);
        }
        self.recursion_depth += 1;
        let res = self.parse_expr_inner(min_bp);
        self.recursion_depth -= 1;
        res
    }

    fn parse_expr_inner(&mut self, min_bp: u8) -> Result<EvalResult, AbacusError> {
        // ── NUD (prefix / atom) ──
        let mut lhs = self.parse_prefix()?;

        // ── LED (infix / postfix) ──
        loop {
            // Consume trailing percentage descriptor tags (e.g. "18% tip", "15% tax")
            if let Some(Token::PercentTag(tag)) = self.peek() {
                // A tag is only meaningful after a percentage (`20% off`); `5 off` is an error
                // rather than silently ignoring the word.
                if !lhs.as_scalar().is_ok_and(|v| v.unit.is_percent()) {
                    return Err(AbacusError::UnexpectedToken((*tag).to_string()));
                }
                self.last_percent_tag = Some(*tag);
                self.advance();
                continue;
            }

            // Check for postfix operators (e.g. `!`)
            if let Some(Token::UnaryOp(name)) = self.peek() {
                let name = *name;
                if let Some(op) = self.token_registry.unary_operators.get(name)
                    && (!op.prefix || op.alias == "++" || op.alias == "--")
                {
                    let bp = self.postfix_bp(name);
                    if bp < min_bp {
                        break;
                    }
                    self.advance();
                    lhs = lhs.apply_unary(op)?;
                    continue;
                }
            }

            // Check for infix binary operators
            if let Some(Token::BinaryOp(name)) = self.peek() {
                let name = *name;
                let (l_bp, r_bp) = self.infix_bp(name);
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs = self.parse_expr(r_bp)?;
                if self.config.strict_dimensions
                    && matches!(name, "+" | "-")
                    && let (Ok(l_val), Ok(r_val)) = (lhs.as_scalar(), rhs.as_scalar())
                {
                    let l_dimless = l_val.unit.is_dimensionless();
                    let r_dimless = r_val.unit.is_dimensionless();
                    if l_dimless != r_dimless && !r_val.unit.is_percent() {
                        return Err(AbacusError::IncompatibleDimensions);
                    }
                }
                lhs = self.eval_binary_op(name, lhs, rhs)?;
                continue;
            }

            // Check for range operator `..` (constructing an Interval) outside function arguments
            if self.function_arg_depth == 0 && self.peek() == Some(&Token::Range) {
                let l_bp = 5;
                let r_bp = 6;
                if l_bp < min_bp {
                    break;
                }
                self.advance();
                let rhs_res = self.parse_expr(r_bp)?;
                let lo = lhs.into_scalar().map_err(|_| {
                    AbacusError::UnexpectedToken(
                        "range endpoints must be scalar values".to_string(),
                    )
                })?;
                let hi = rhs_res.into_scalar().map_err(|_| {
                    AbacusError::UnexpectedToken(
                        "range endpoints must be scalar values".to_string(),
                    )
                })?;
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
                    .unwrap_or(IntervalStyle::Range);
                lhs = EvalResult::Interval(Interval::new_with_style(lo, hi, style)?);
                continue;
            }

            // Check for conversion operator (`in`, `to`, `as`)
            if let Some(Token::ConversionOp) = self.peek() {
                let l_bp = 1; // lowest infix precedence
                if l_bp < min_bp {
                    break;
                }
                lhs = self.parse_conversion(lhs)?;
                continue;
            }

            // Check for relative time operators (`ago`, `from_now`, `before`, `after`)
            if let Some(Token::RelTimeOp(op_name)) = self.peek() {
                let op_name = *op_name;
                let l_bp = 5;
                if l_bp < min_bp {
                    break;
                }

                // If op_name is "after" and lhs is NOT a time duration, check for conversational percentage (e.g. "$50 after 15% tax")
                if op_name == "after"
                    && lhs.as_scalar().is_ok_and(|v| {
                        v.unit.dimensions != crate::units::dimensions::Dimensions::TIME
                    })
                {
                    let val = lhs.into_scalar()?;
                    self.advance();
                    self.last_percent_tag = None;
                    let rhs = self.parse_expr(l_bp)?;
                    let rhs_val = rhs.into_scalar()?;
                    if rhs_val.unit.is_percent() {
                        let is_discount = self.last_percent_tag == Some("discount")
                            || self.last_percent_tag == Some("off")
                            || matches!(self.peek(), Some(Token::PercentTag("discount" | "off")));
                        if matches!(self.peek(), Some(Token::PercentTag(_))) {
                            self.advance();
                        }
                        let factor = if is_discount {
                            1.0 - rhs_val.canonical
                        } else {
                            1.0 + rhs_val.canonical
                        };
                        lhs = EvalResult::Scalar(Value {
                            canonical: val.canonical * factor,
                            unit: std::sync::Arc::clone(&val.unit),
                        });
                        continue;
                    }
                    return Err(AbacusError::IncompatibleDimensions);
                }

                if op_name == "until" {
                    self.advance();
                    let rhs = self.parse_expr(l_bp)?;
                    let target_date = match rhs {
                        EvalResult::Date(d) => d,
                        _ => {
                            return Err(AbacusError::EvaluationError(
                                "expected a date after 'until'".to_string(),
                            ));
                        }
                    };
                    let now = self.get_now().clone();

                    if let EvalResult::Date(from_date) = lhs {
                        let diff_days = target_date.to_epoch_days() - from_date.to_epoch_days();
                        let day_unit = self.unit_registry.unit("d")?;
                        lhs = EvalResult::Scalar(Value::new(diff_days as f64, day_unit));
                        continue;
                    }

                    let val = lhs.into_scalar()?;
                    if val.unit.dimensions != crate::units::dimensions::Dimensions::TIME {
                        return Err(AbacusError::IncompatibleDimensions);
                    }

                    if val.unit.is_business_day_unit() {
                        let bdays =
                            now.business_days_between_with(&target_date, self.config.weekend);
                        lhs = EvalResult::Scalar(Value::new(
                            bdays as f64,
                            std::sync::Arc::clone(&val.unit),
                        ));
                        continue;
                    }

                    let unit_str = val.unit.display.render();
                    if unit_str == "d" || unit_str == "days" || unit_str == "day" {
                        let days_diff = target_date.to_epoch_days() - now.to_epoch_days();
                        lhs = EvalResult::Scalar(Value::new(
                            days_diff as f64,
                            std::sync::Arc::clone(&val.unit),
                        ));
                        continue;
                    }

                    let diff_sec = now.milliseconds_between(&target_date) as f64 / 1000.0;
                    let sec_val = Value::new(diff_sec, self.unit_registry.unit("s")?);
                    let converted = sec_val.convert_to(std::sync::Arc::clone(&val.unit))?;
                    lhs = EvalResult::Scalar(converted);
                    continue;
                }

                self.advance();

                let val = lhs.clone().into_scalar()?;
                if val.unit.dimensions != crate::units::dimensions::Dimensions::TIME {
                    return Err(AbacusError::IncompatibleDimensions);
                }
                let weekend = self.config.weekend;

                match op_name {
                    "ago" => {
                        lhs = EvalResult::Date(
                            self.get_now().apply_time_value_with(&val, -1, weekend)?,
                        );
                        continue;
                    }
                    "from_now" => {
                        lhs = EvalResult::Date(
                            self.get_now().apply_time_value_with(&val, 1, weekend)?,
                        );
                        continue;
                    }
                    "before" => {
                        let ref_date = if self.can_start_date_expr() {
                            let rhs = self.parse_expr(5)?;
                            match rhs {
                                EvalResult::Date(d) => d,
                                _ => self.get_now().clone(),
                            }
                        } else {
                            self.get_now().clone()
                        };
                        lhs = EvalResult::Date(ref_date.apply_time_value_with(&val, -1, weekend)?);
                        continue;
                    }
                    "after" => {
                        let ref_date = if self.can_start_date_expr() {
                            let rhs = self.parse_expr(5)?;
                            match rhs {
                                EvalResult::Date(d) => d,
                                _ => self.get_now().clone(),
                            }
                        } else {
                            self.get_now().clone()
                        };
                        lhs = EvalResult::Date(ref_date.apply_time_value_with(&val, 1, weekend)?);
                        continue;
                    }
                    _ => {}
                }
            }

            // Check for property access operator (`.prop`)
            if let Some(Token::DotProperty(_)) = self.peek() {
                let bp = 100; // high precedence for dot member access
                if bp < min_bp {
                    break;
                }
                let Some(Token::DotProperty(prop)) = self.next_token() else {
                    break;
                };
                let prop = *prop;
                lhs = match lhs {
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
                        EvalResult::Scalar(val.clone())
                    }
                    EvalResult::Date(d) => {
                        let num =
                            d.get_property_with(prop, self.config.weekend)
                                .ok_or_else(|| {
                                    AbacusError::UnexpectedToken(format!(
                                        "unknown property '.{prop}' on Date"
                                    ))
                                })?;
                        EvalResult::Scalar(Value::dimensionless(num))
                    }
                    _ => {
                        return Err(AbacusError::UnexpectedToken(format!(
                            "cannot access property '.{prop}' on this result type"
                        )));
                    }
                };
                continue;
            }

            // No more infix/postfix operators at this precedence level
            break;
        }

        Ok(lhs)
    }

    /// Fast-path binary operator evaluation for standard operators.
    #[inline]
    fn eval_binary_op(
        &self,
        name: &str,
        lhs: EvalResult,
        rhs: EvalResult,
    ) -> Result<EvalResult, AbacusError> {
        // Fast-path scalar-to-scalar standard arithmetic
        if matches!(name, "+" | "-" | "*" | "/")
            && let (EvalResult::Scalar(l), EvalResult::Scalar(r)) = (&lhs, &rhs)
        {
            use crate::evaluation::tokenizer::registry::binary::arithmetic::check_overflow;
            let (x, y) = (l.canonical, r.canonical);
            let result = match name {
                "+" => l + r,
                "-" => l - r,
                "*" => l * r,
                // Division by zero yields ±∞ by design; only genuine overflow is an error.
                _ if y == 0.0 => return (l / r).map(EvalResult::Scalar),
                _ => l / r,
            };
            return check_overflow(result?, x, y).map(EvalResult::Scalar);
        }

        if name == "^" {
            match &rhs {
                EvalResult::Scalar(r) if r.canonical.abs() > self.config.max_exponent => {
                    return Err(AbacusError::ExponentLimitExceeded);
                }
                EvalResult::Interval(i)
                    if i.lo.canonical.abs() > self.config.max_exponent
                        || i.hi.canonical.abs() > self.config.max_exponent =>
                {
                    return Err(AbacusError::ExponentLimitExceeded);
                }
                _ => {}
            }
        }

        let op = self
            .token_registry
            .binary_operators
            .get(name)
            .ok_or_else(|| AbacusError::UnexpectedToken(name.to_string()))?;
        lhs.apply_binary_with_weekend(op, rhs, self.config.weekend)
    }

    /// Returns (`left_bp`, `right_bp`) for an infix binary operator, derived from the registered
    /// precedence (`+ -` = 0 -> (2, 3), `* / % of` = 1 -> (4, 5), `^` = 3 right-assoc -> (9, 8)).
    pub(crate) fn infix_bp(&self, name: &str) -> (u8, u8) {
        self.token_registry
            .binary_operators
            .get(name)
            .map_or((0, 0), |op| {
                let base = op.precedence.saturating_mul(2).saturating_add(2);
                if op.right_associative {
                    (base.saturating_add(1), base)
                } else {
                    (base, base.saturating_add(1))
                }
            })
    }

    /// Returns the binding power for a prefix unary operator, derived from the registered
    /// precedence (`-`, `sqrt`, `++`, `--` = 2 -> 6, binding tighter than `*` but looser than `^`).
    pub(crate) fn prefix_bp(&self, name: &str) -> u8 {
        self.unary_bp(name)
    }

    /// Returns the binding power for a postfix unary operator (`!` = 4 -> 10, tighter than `^`).
    pub(crate) fn postfix_bp(&self, name: &str) -> u8 {
        self.unary_bp(name)
    }

    fn unary_bp(&self, name: &str) -> u8 {
        self.token_registry
            .unary_operators
            .get(name)
            .map_or(10, |op| op.precedence.saturating_mul(2).saturating_add(2))
    }
}
