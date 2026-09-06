use crate::{
    AbacusError, Token, TokenRegistry, UnitRegistry,
    evaluation::parser::config::EvalConfig,
    units::{eval_result::EvalResult, interval::Interval, interval::IntervalStyle, value::Value},
};

pub const MAX_RECURSION_DEPTH: usize = 64;

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
            let mut now = crate::Date::now();
            if now.timezone.is_none() {
                now.timezone = self.config.default_timezone.clone();
            }
            self.now = Some(now);
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
        if self.recursion_depth >= self.config.max_recursion_depth {
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
                    && let Ok(val) = lhs.clone().into_scalar()
                    && val.unit.dimensions != crate::units::dimensions::Dimensions::TIME
                {
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

                    let val = lhs.clone().into_scalar()?;
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

                    let diff_sec = (target_date.to_epoch_milliseconds()
                        - now.to_epoch_milliseconds()) as f64
                        / 1000.0;
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
                let ms = (val.canonical * 1000.0).round() as i64;

                match op_name {
                    "ago" => {
                        lhs =
                            EvalResult::Date(self.get_now().add_milliseconds(ms.saturating_neg()));
                        continue;
                    }
                    "from_now" => {
                        lhs = EvalResult::Date(self.get_now().add_milliseconds(ms));
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
                        lhs = EvalResult::Date(ref_date.add_milliseconds(ms.saturating_neg()));
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
                        lhs = EvalResult::Date(ref_date.add_milliseconds(ms));
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
                let prop = match self.next_token() {
                    Some(Token::DotProperty(p)) => p,
                    _ => unreachable!(),
                };
                lhs = match lhs {
                    EvalResult::Hash(hash) => {
                        let val = hash
                            .get(prop)
                            .or_else(|| match prop.as_str() {
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
        if let (EvalResult::Scalar(l), EvalResult::Scalar(r)) = (&lhs, &rhs) {
            match name {
                "+" => return (l + r).map(EvalResult::Scalar),
                "-" => return (l - r).map(EvalResult::Scalar),
                "*" => return (l * r).map(EvalResult::Scalar),
                "/" => return (l / r).map(EvalResult::Scalar),
                _ => {}
            }
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

    /// Returns (`left_bp`, `right_bp`) for an infix binary operator.
    pub(crate) fn infix_bp(&self, name: &str) -> (u8, u8) {
        match name {
            "+" | "-" => (2, 3),
            "*" | "/" | "%" | "of" => (4, 5),
            "^" => (9, 8),
            _ => {
                if let Some(op) = self.token_registry.binary_operators.get(name) {
                    let base = (op.precedence * 2) + 2;
                    if op.right_associative {
                        (base + 1, base)
                    } else {
                        (base, base + 1)
                    }
                } else {
                    (0, 0)
                }
            }
        }
    }

    /// Returns the binding power for a prefix unary operator.
    pub(crate) fn prefix_bp(&self, name: &str) -> u8 {
        match name {
            "-" | "sqrt" => 6,
            "++" | "--" => 2,
            _ => {
                if let Some(op) = self.token_registry.unary_operators.get(name) {
                    (op.precedence * 2) + 2
                } else {
                    10
                }
            }
        }
    }

    /// Returns the binding power for a postfix unary operator.
    pub(crate) fn postfix_bp(&self, name: &str) -> u8 {
        match name {
            "!" => 10,
            _ => {
                if let Some(op) = self.token_registry.unary_operators.get(name) {
                    (op.precedence * 2) + 2
                } else {
                    10
                }
            }
        }
    }
}
