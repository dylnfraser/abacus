#![warn(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod evaluation;

pub mod error;
pub mod registry;
pub mod units;

use std::collections::HashMap;
use std::sync::Arc;

pub use error::AbacusError;
pub use registry::unit_registry::UnitRegistry;
pub use units::{
    date::{Date, DateFormat, DayOfWeek, Time, TimeZone, WeekendDays},
    dimensions::Dimensions,
    eval_result::EvalResult,
    hash::Hash,
    interval::{Interval, IntervalStyle},
    unit::Unit,
    value::Value,
};

pub use evaluation::tokenizer::registry::{
    binary::operators::BinaryOp, function::operators::FunctionOp,
    function::operators::FunctionTarget, token_registry::TokenRegistry, unary::operators::UnaryOp,
};

pub use evaluation::tokenizer::sig_figs::{
    count_significant_figures, min_significant_figures_in_expr,
};
pub use evaluation::tokenizer::tokens::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AngleMode {
    #[default]
    Radians,
    Degrees,
    Gradians,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Notation {
    #[default]
    Standard,
    Scientific,
    Engineering,
}

use crate::evaluation::parser::config::EvalConfig;

pub struct Abacus {
    pub units: UnitRegistry,
    pub tokens: TokenRegistry,
    pub date_format: DateFormat,
    pub significant_figures: Option<usize>,
    pub follow_significant_figures: bool,
    pub auto_derived_units: bool,
    pub angle_mode: AngleMode,
    pub strict_dimensions: bool,
    pub decimal_places: Option<usize>,
    pub default_interval_style: Option<IntervalStyle>,
    pub notation: Notation,
    pub default_timezone: Option<TimeZone>,
    pub weekend: WeekendDays,
    pub max_recursion_depth: usize,
    pub max_exponent: f64,
    pub implicit_multiplication: bool,
    pub number_scales: bool,
    pub currencies: bool,
    pub live_rates: bool,
    pub currency_cache_path: Option<std::path::PathBuf>,
    pub unit_display_overrides: HashMap<String, String>,
    pub variables: HashMap<String, EvalResult>,
    pub anchor_date: Option<Date>,
}

impl Abacus {
    /// Returns the standard mathematical constants (`pi`, `PI`, `e`, `E`, `tau`, `TAU`, `phi`, `PHI`).
    #[must_use]
    pub fn standard_variables() -> HashMap<String, EvalResult> {
        crate::evaluation::tokenizer::registry::constants::standard_variables_map()
    }

    /// Initialize a new `Abacus` instance with default empty unit and token registries.
    #[must_use]
    pub fn new() -> Self {
        Self::from_registry(UnitRegistry::new(), TokenRegistry::new())
    }

    /// Initialize a new `Abacus` instance with custom units and tokens.
    pub fn from_registry(units: UnitRegistry, tokens: TokenRegistry) -> Self {
        Self {
            units,
            tokens,
            date_format: DateFormat::default(),
            significant_figures: None,
            follow_significant_figures: false,
            auto_derived_units: true,
            angle_mode: AngleMode::default(),
            strict_dimensions: false,
            decimal_places: None,
            default_interval_style: None,
            notation: Notation::default(),
            default_timezone: None,
            weekend: WeekendDays::default(),
            max_recursion_depth: 64,
            max_exponent: 1_000.0,
            implicit_multiplication: true,
            number_scales: cfg!(feature = "number-scales"),
            currencies: cfg!(feature = "currencies"),
            live_rates: false,
            currency_cache_path: {
                #[cfg(feature = "currencies")]
                {
                    Some(crate::registry::units::currency_units::default_currency_cache_path())
                }
                #[cfg(not(feature = "currencies"))]
                {
                    None
                }
            },
            unit_display_overrides: HashMap::new(),
            variables: HashMap::new(),
            anchor_date: None,
        }
    }

    /// Initialize an `Abacus` instance populated with standard physical units, operator registries, and mathematical constants (`pi`, `e`, `tau`, `phi`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use abacus::Abacus;
    ///
    /// let abacus = Abacus::standard();
    /// let val = abacus.eval_scalar("2 * 5 m").unwrap();
    /// assert_eq!(val.to_display(), "10 m");
    /// ```
    #[must_use]
    pub fn standard() -> Self {
        let mut abacus = Self::from_registry(UnitRegistry::standard(), TokenRegistry::standard());
        abacus.variables = Self::standard_variables();
        abacus
    }

    // Set the date format for this `Abacus` instance
    pub fn set_date_format(mut self, format: DateFormat) -> Self {
        self.date_format = format;
        self
    }

    /// Sets the significant figures for output rounding and formatting.
    #[must_use]
    pub fn set_significant_figures(mut self, sig_figs: Option<usize>) -> Self {
        self.significant_figures = sig_figs;
        self
    }

    /// Builder method to configure a specific number of significant figures.
    #[must_use]
    pub fn with_significant_figures(mut self, sig_figs: usize) -> Self {
        self.significant_figures = Some(sig_figs);
        self
    }

    /// Sets whether the evaluator should follow the minimum significant figures
    /// of the input expression's numeric literals.
    #[must_use]
    pub fn set_follow_significant_figures(mut self, follow: bool) -> Self {
        self.follow_significant_figures = follow;
        self
    }

    /// Builder method to follow the expression's input significant figures.
    #[must_use]
    pub fn with_follow_significant_figures(mut self, follow: bool) -> Self {
        self.follow_significant_figures = follow;
        self
    }

    /// Sets whether automatic derived unit reduction (e.g. N*m -> J) is enabled.
    #[must_use]
    pub fn set_auto_derived_units(mut self, enabled: bool) -> Self {
        self.auto_derived_units = enabled;
        self
    }

    /// Builder method to enable or disable automatic derived unit reduction.
    #[must_use]
    pub fn with_auto_derived_units(mut self, enabled: bool) -> Self {
        self.auto_derived_units = enabled;
        self
    }

    /// In-place setter for significant figures.
    pub fn set_sig_figs(&mut self, sig_figs: Option<usize>) {
        self.significant_figures = sig_figs;
    }

    /// In-place setter for following input significant figures.
    pub fn set_follow_sig_figs(&mut self, follow: bool) {
        self.follow_significant_figures = follow;
    }

    /// In-place setter for automatic derived unit reduction.
    pub fn set_auto_derive(&mut self, enabled: bool) {
        self.auto_derived_units = enabled;
    }

    /// Sets the angle mode for trigonometric functions (Radians, Degrees, Gradians).
    #[must_use]
    pub fn set_angle_mode(mut self, mode: AngleMode) -> Self {
        self.angle_mode = mode;
        self
    }

    /// Builder method to configure the angle mode.
    #[must_use]
    pub fn with_angle_mode(mut self, mode: AngleMode) -> Self {
        self.angle_mode = mode;
        self
    }

    /// In-place setter for angle mode.
    pub fn set_angle(&mut self, mode: AngleMode) {
        self.angle_mode = mode;
    }

    /// Sets whether strict dimension checking is enabled (disallows dimensionless promotion).
    #[must_use]
    pub fn set_strict_dimensions(mut self, strict: bool) -> Self {
        self.strict_dimensions = strict;
        self
    }

    /// Builder method to toggle strict dimension checking.
    #[must_use]
    pub fn with_strict_dimensions(mut self, strict: bool) -> Self {
        self.strict_dimensions = strict;
        self
    }

    /// Builder method to set an anchor date for relative date and event evaluations.
    #[must_use]
    pub fn with_now(mut self, now: Date) -> Self {
        self.anchor_date = Some(now);
        self
    }

    /// In-place setter for strict dimension checking.
    pub fn set_strict_dims(&mut self, strict: bool) {
        self.strict_dimensions = strict;
    }

    /// Sets the number of decimal places for rounding and formatting.
    #[must_use]
    pub fn set_decimal_places(mut self, places: Option<usize>) -> Self {
        self.decimal_places = places;
        self
    }

    /// Builder method to configure fixed decimal places.
    #[must_use]
    pub fn with_decimal_places(mut self, places: usize) -> Self {
        self.decimal_places = Some(places);
        self
    }

    /// In-place setter for decimal places.
    pub fn set_dec_places(&mut self, places: Option<usize>) {
        self.decimal_places = places;
    }

    /// Sets the default interval display style (Bracket [a, b] or Range a..b).
    #[must_use]
    pub fn set_interval_style(mut self, style: Option<IntervalStyle>) -> Self {
        self.default_interval_style = style;
        self
    }

    /// Builder method to configure interval display style.
    #[must_use]
    pub fn with_interval_style(mut self, style: IntervalStyle) -> Self {
        self.default_interval_style = Some(style);
        self
    }

    /// In-place setter for interval display style.
    pub fn set_int_style(&mut self, style: Option<IntervalStyle>) {
        self.default_interval_style = style;
    }

    /// Sets the numerical notation mode (Standard, Scientific, Engineering).
    #[must_use]
    pub fn set_notation(mut self, notation: Notation) -> Self {
        self.notation = notation;
        self
    }

    /// Builder method to configure notation mode.
    #[must_use]
    pub fn with_notation(mut self, notation: Notation) -> Self {
        self.notation = notation;
        self
    }

    /// In-place setter for notation mode.
    pub fn set_note_mode(&mut self, notation: Notation) {
        self.notation = notation;
    }

    /// Sets the default timezone for unqualified date and time evaluations.
    #[must_use]
    pub fn set_default_timezone(mut self, tz: Option<TimeZone>) -> Self {
        self.default_timezone = tz;
        self
    }

    /// Builder method to configure default timezone.
    #[must_use]
    pub fn with_default_timezone(mut self, tz: TimeZone) -> Self {
        self.default_timezone = Some(tz);
        self
    }

    /// In-place setter for default timezone.
    pub fn set_def_tz(&mut self, tz: Option<TimeZone>) {
        self.default_timezone = tz;
    }

    /// Sets the weekend days definition for business day calculations.
    #[must_use]
    pub fn set_weekend(mut self, weekend: WeekendDays) -> Self {
        self.weekend = weekend;
        self
    }

    /// Builder method to configure weekend days.
    #[must_use]
    pub fn with_weekend(mut self, weekend: WeekendDays) -> Self {
        self.weekend = weekend;
        self
    }

    /// In-place setter for weekend days.
    pub fn set_wknd(&mut self, weekend: WeekendDays) {
        self.weekend = weekend;
    }

    /// Sets the maximum expression recursion depth limit.
    #[must_use]
    pub fn set_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Builder method to configure maximum recursion depth.
    #[must_use]
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// In-place setter for maximum recursion depth.
    pub fn set_max_depth(&mut self, depth: usize) {
        self.max_recursion_depth = depth;
    }

    /// Sets the maximum exponent limit to prevent freezes or resource exhaustion.
    #[must_use]
    pub fn set_max_exponent(mut self, max: f64) -> Self {
        self.max_exponent = max;
        self.units.set_max_exponent(max);
        self
    }

    /// Builder method to configure the maximum exponent limit.
    #[must_use]
    pub fn with_max_exponent(mut self, max: f64) -> Self {
        self.max_exponent = max;
        self.units.set_max_exponent(max);
        self
    }

    /// In-place setter for maximum exponent limit.
    pub fn set_max_exp(&mut self, max: f64) {
        self.max_exponent = max;
        self.units.set_max_exponent(max);
    }

    /// Sets whether implicit multiplication (e.g. `2(3)`) is allowed.
    #[must_use]
    pub fn set_implicit_multiplication(mut self, enabled: bool) -> Self {
        self.implicit_multiplication = enabled;
        self
    }

    /// Builder method to toggle implicit multiplication.
    #[must_use]
    pub fn with_implicit_multiplication(mut self, enabled: bool) -> Self {
        self.implicit_multiplication = enabled;
        self
    }

    /// In-place setter for implicit multiplication.
    pub fn set_implicit_mul(&mut self, enabled: bool) {
        self.implicit_multiplication = enabled;
    }

    /// Sets whether natural number scale words (e.g. `million`, `billion`, `thousand`, `dozen`) are recognized as scale multipliers.
    #[must_use]
    pub fn set_number_scales(mut self, enabled: bool) -> Self {
        self.number_scales = enabled;
        self
    }

    /// Builder method to toggle natural number scale words.
    #[must_use]
    pub fn with_number_scales(mut self, enabled: bool) -> Self {
        self.number_scales = enabled;
        self
    }

    /// In-place setter for natural number scale words.
    pub fn set_scales(&mut self, enabled: bool) {
        self.number_scales = enabled;
    }

    /// Builder method to enable or disable currency support.
    #[must_use]
    pub fn with_currencies(mut self, enabled: bool) -> Self {
        self.set_currencies(enabled);
        self
    }

    /// In-place setter to enable or disable currency support.
    pub fn set_currencies(&mut self, enabled: bool) {
        self.currencies = enabled;
        #[cfg(feature = "currencies")]
        {
            self.units.set_currencies_enabled(enabled);
        }
    }

    /// Configures a custom file path for caching exchange rates.
    pub fn set_currency_cache(&mut self, path: impl Into<std::path::PathBuf>) {
        self.currency_cache_path = Some(path.into());
    }

    /// Builder method to configure a custom file path for caching exchange rates.
    #[must_use]
    pub fn with_currency_cache(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.set_currency_cache(path);
        self
    }

    /// Enables or disables live exchange rates fetched from Frankfurter API (https://api.frankfurter.dev/v1/latest?base=USD).
    #[must_use]
    pub fn with_live_rates(mut self, enabled: bool) -> Self {
        self.set_live_rates(enabled);
        self
    }

    /// Sets whether live exchange rates should be fetched.
    pub fn set_live_rates(&mut self, enabled: bool) {
        self.live_rates = enabled;
        if enabled {
            let _ = self.update_daily_rates();
        }
    }

    /// Updates currency rates according to daily cache policy:
    /// Loads from disk if cache is fresh (< 24 hours); otherwise fetches live rates and updates cache.
    pub fn update_daily_rates(&mut self) -> Result<(), AbacusError> {
        #[cfg(feature = "currencies")]
        {
            let cache_path = self.currency_cache_path.clone().unwrap_or_else(
                crate::registry::units::currency_units::default_currency_cache_path,
            );
            let rates =
                crate::registry::units::currency_units::get_or_update_daily_rates(&cache_path)?;
            self.units.update_currency_rates(&rates);
            Ok(())
        }
        #[cfg(not(feature = "currencies"))]
        {
            Err(AbacusError::EvaluationError(
                "currencies feature is not enabled".to_string(),
            ))
        }
    }

    /// Fetches live exchange rates from https://api.frankfurter.dev/v1/latest?base=USD,
    /// saves them to the configured cache file, and updates the unit registry.
    pub fn fetch_live_rates(&mut self) -> Result<(), AbacusError> {
        #[cfg(feature = "currencies")]
        {
            let cache_path = self.currency_cache_path.clone().unwrap_or_else(
                crate::registry::units::currency_units::default_currency_cache_path,
            );
            let rates = crate::registry::units::currency_units::fetch_and_cache_rates(&cache_path)?;
            self.units.update_currency_rates(&rates);
            Ok(())
        }
        #[cfg(not(feature = "currencies"))]
        {
            Err(AbacusError::EvaluationError(
                "currencies feature is not enabled".to_string(),
            ))
        }
    }

    /// Updates currency exchange rates from a JSON string (Frankfurter response format)
    /// and saves the response into the cache file if configured.
    pub fn update_rates_from_json(&mut self, _json_str: &str) -> Result<(), AbacusError> {
        #[cfg(feature = "currencies")]
        {
            let rates = crate::registry::units::currency_units::parse_frankfurter_json(_json_str)?;
            self.units.update_currency_rates(&rates);
            if let Some(ref path) = self.currency_cache_path {
                let _ =
                    crate::registry::units::currency_units::save_rates_to_cache(_json_str, path);
            }
            Ok(())
        }
        #[cfg(not(feature = "currencies"))]
        {
            Err(AbacusError::EvaluationError(
                "currencies feature is not enabled".to_string(),
            ))
        }
    }

    /// Sets an exchange rate manually for a specific currency code (e.g. "EUR", 0.86281).
    pub fn set_currency_rate(&mut self, _code: &str, _rate_per_usd: f64) {
        #[cfg(feature = "currencies")]
        {
            self.units.set_currency_rate(_code, _rate_per_usd);
        }
    }

    /// Builder method to set an exchange rate for a currency code.
    #[must_use]
    pub fn with_currency_rate(mut self, code: &str, rate_per_usd: f64) -> Self {
        self.set_currency_rate(code, rate_per_usd);
        self
    }
}

/// Checks whether the specified exchange rates cache file is fresh (modified < 24 hours ago).
#[must_use]
pub fn is_currency_cache_fresh(cache_path: &std::path::Path) -> bool {
    #[cfg(feature = "currencies")]
    {
        crate::registry::units::currency_units::is_cache_fresh(cache_path)
    }
    #[cfg(not(feature = "currencies"))]
    {
        let _ = cache_path;
        false
    }
}

/// Returns the default file path where exchange rates are cached.
#[must_use]
pub fn default_currency_cache_path() -> std::path::PathBuf {
    #[cfg(feature = "currencies")]
    {
        crate::registry::units::currency_units::default_currency_cache_path()
    }
    #[cfg(not(feature = "currencies"))]
    {
        std::env::temp_dir().join("abacus_currency_rates.json")
    }
}

impl Abacus {
    // Tokenize an expression into a vector of `Token`s
    pub fn tokenize<'a>(&self, expr: &'a str) -> Result<Vec<Token<'a>>, AbacusError> {
        crate::evaluation::tokenizer::tokenize_string_full(
            &self.tokens,
            &self.units,
            Some(&self.variables),
            expr,
            self.implicit_multiplication,
            self.number_scales,
        )
    }

    /// Define a variable on this `Abacus` instance (builder pattern).
    #[must_use]
    pub fn with_variable(mut self, name: impl Into<String>, value: impl Into<EvalResult>) -> Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// Define a variable in-place on this `Abacus` instance.
    pub fn set_variable(
        &mut self,
        name: impl Into<String>,
        value: impl Into<EvalResult>,
    ) -> &mut Self {
        self.variables.insert(name.into(), value.into());
        self
    }

    /// Define a variable by evaluating an expression string on this `Abacus` instance.
    pub fn set_variable_expr(
        &mut self,
        name: impl Into<String>,
        expr: &str,
    ) -> Result<EvalResult, AbacusError> {
        let val = self.eval(expr)?;
        self.variables.insert(name.into(), val.clone());
        Ok(val)
    }

    /// Retrieve a reference to a defined variable.
    #[must_use]
    pub fn get_variable(&self, name: &str) -> Option<&EvalResult> {
        self.variables.get(name)
    }

    /// Check if a variable is defined.
    #[must_use]
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Remove a defined variable.
    pub fn remove_variable(&mut self, name: &str) -> Option<EvalResult> {
        self.variables.remove(name)
    }

    /// Clear all defined variables.
    pub fn clear_variables(&mut self) {
        self.variables.clear();
    }

    /// Reset variables back to standard mathematical constants (`pi`, `e`, `tau`, `phi`).
    pub fn reset_standard_variables(&mut self) {
        self.variables = Self::standard_variables();
    }

    /// Returns the standard common speed unit display overrides ("mi/h" -> "mph", "km/h" -> "kmph").
    #[must_use]
    pub fn standard_unit_display_overrides() -> HashMap<String, String> {
        crate::units::speed_overrides::standard_speed_overrides()
    }

    /// Adds a display override for a unit string (e.g. `"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`).
    #[must_use]
    pub fn with_unit_display_override(
        mut self,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.set_unit_display_override(pattern, replacement);
        self
    }

    /// Sets a display override for a unit string in-place.
    pub fn set_unit_display_override(
        &mut self,
        pattern: impl Into<String>,
        replacement: impl Into<String>,
    ) {
        self.unit_display_overrides
            .insert(pattern.into(), replacement.into());
    }

    /// Builder method to configure multiple unit display overrides.
    #[must_use]
    pub fn with_unit_display_overrides(mut self, overrides: HashMap<String, String>) -> Self {
        self.set_unit_display_overrides(overrides);
        self
    }

    /// Sets multiple unit display overrides in-place.
    pub fn set_unit_display_overrides(&mut self, overrides: HashMap<String, String>) {
        self.unit_display_overrides.extend(overrides);
    }

    /// Enables common speed overrides (`"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`).
    #[must_use]
    pub fn with_common_speed_overrides(mut self) -> Self {
        self.enable_common_speed_overrides();
        self
    }

    /// Enables common speed overrides (`"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`) in-place.
    pub fn enable_common_speed_overrides(&mut self) {
        self.set_unit_display_overrides(Self::standard_unit_display_overrides());
    }

    /// Removes a unit display override.
    pub fn remove_unit_display_override(&mut self, pattern: &str) -> Option<String> {
        self.unit_display_overrides.remove(pattern)
    }

    /// Clears all unit display overrides.
    pub fn clear_unit_display_overrides(&mut self) {
        self.unit_display_overrides.clear();
    }

    /// Checks if a display override is configured for a pattern.
    #[must_use]
    pub fn has_unit_display_override(&self, pattern: &str) -> bool {
        self.unit_display_overrides.contains_key(pattern)
    }

    /// Evaluate an expression or variable assignment (`name = expr`).
    /// If the expression is an assignment, updates or creates the variable on `self` and returns its evaluated value.
    /// Otherwise, evaluates the expression normally.
    pub fn eval_mut(&mut self, expr: &str) -> Result<EvalResult, AbacusError> {
        let trimmed = expr.trim();
        // Check for simple assignment: <ident> = <rhs>
        if let Some((ident, rhs)) = trimmed.split_once('=') {
            let ident = ident.trim();
            let is_ident = !ident.is_empty()
                && (ident.starts_with(|c: char| c.is_alphabetic() || c == '_'))
                && ident.chars().all(|c| c.is_alphanumeric() || c == '_');
            if is_ident && !rhs.starts_with('=') {
                let res = self.eval(rhs)?;
                self.variables.insert(ident.to_string(), res.clone());
                return Ok(res);
            }
        }
        self.eval(expr)
    }

    // Evaluate expressions, returning the result as an `EvalResult`
    // Evaluated dates are automatically formatted
    // Returns an error if the expression is invalid or cannot be evaluated.
    pub fn eval(&self, expr: &str) -> Result<EvalResult, AbacusError> {
        let config = EvalConfig::from(self);
        let mut res = crate::evaluation::parser::evaluate_with_variables(
            &self.tokens,
            &self.units,
            Some(&self.variables),
            expr,
            config,
        )?;
        if let EvalResult::Date(ref mut d) = res {
            d.format = self.date_format;
            if d.timezone.is_none() {
                d.timezone = self.default_timezone.clone();
            }
        }

        if let Some(style) = self.default_interval_style {
            res = res.with_interval_style(style);
        }

        if let Some(dec) = self.decimal_places {
            res = res.round_to_decimals(dec);
        } else if self.significant_figures.is_none() && !self.follow_significant_figures {
            match &res {
                EvalResult::Scalar(v)
                    if v.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY =>
                {
                    let unit_str = v.unit.display.render();
                    let dec = crate::units::value::currency_decimal_places(&unit_str);
                    res = res.round_to_decimals(dec);
                }
                EvalResult::Interval(inv)
                    if inv.lo.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY =>
                {
                    let unit_str = inv.lo.unit.display.render();
                    let dec = crate::units::value::currency_decimal_places(&unit_str);
                    res = res.round_to_decimals(dec);
                }
                _ => {}
            }
        }

        let effective_sig_figs = self.significant_figures.or_else(|| {
            if self.follow_significant_figures {
                min_significant_figures_in_expr(expr)
            } else {
                None
            }
        });

        if let Some(sig) = effective_sig_figs {
            res = res.round_to_sig_figs(sig);
        }

        if !self.unit_display_overrides.is_empty() {
            res = res.with_display_override(&self.unit_display_overrides);
        }

        Ok(res)
    }

    /// Formats an `EvalResult` according to this `Abacus` instance's configuration
    /// (date format, significant figures, decimal places, notation, etc.).
    #[must_use]
    pub fn format_result(&self, res: &EvalResult) -> String {
        let mut res = res.clone();
        if let Some(style) = self.default_interval_style {
            res = res.with_interval_style(style);
        }
        if !self.unit_display_overrides.is_empty() {
            res = res.with_display_override(&self.unit_display_overrides);
        }
        match self.notation {
            Notation::Scientific => res.to_display_scientific(),
            Notation::Engineering => res.to_display_engineering(),
            Notation::Standard => {
                if let Some(decimals) = self.decimal_places {
                    res.to_display_with_decimals(decimals)
                } else if let Some(sig_figs) = self.significant_figures {
                    res.to_display_with_sig_figs(sig_figs)
                } else {
                    res.to_display()
                }
            }
        }
    }

    /// Evaluate an expression, returning only scalar results.
    /// Returns an error if the result is an interval or hash.
    pub fn eval_scalar(&self, expr: &str) -> Result<Value, AbacusError> {
        self.eval(expr)?.into_scalar()
    }

    /// Evaluate an expression, returning only Hash results.
    /// Returns an error if the result is not a Hash.
    pub fn eval_hash(&self, expr: &str) -> Result<Hash, AbacusError> {
        self.eval(expr)?.into_hash()
    }

    /// Evaluate an expression, returning only Date results.
    /// Returns an error if the result is not a Date.
    pub fn eval_date(&self, expr: &str) -> Result<Date, AbacusError> {
        self.eval(expr)?.into_date()
    }

    // Format dates using configured styles
    pub fn format_date(&self, date: &Date) -> String {
        date.format_with_style(self.date_format)
    }

    pub fn format_date_with_style(&self, date: &Date, style: DateFormat) -> String {
        date.format_with_style(style)
    }

    // Registry functions for units & tokens
    pub fn register_unit(&mut self, alias: &str, unit: Unit) {
        self.units.insert_unit(alias, Arc::from(unit));
    }

    pub fn register_binop_token(&mut self, op: BinaryOp) {
        self.tokens.register_binary_operator(op.alias, op);
    }

    pub fn register_unop_token(&mut self, op: UnaryOp) {
        self.tokens.register_unary_operator(op.alias, op);
    }

    pub fn register_function_token(&mut self, op: FunctionOp) {
        self.tokens.register_function_operator(op.name, op);
    }

    /// Returns a reference to the globally shared standard `Abacus` instance.
    #[must_use]
    pub fn shared() -> &'static Self {
        &STANDARD_ABACUS
    }
}

pub static STANDARD_ABACUS: std::sync::LazyLock<Abacus> =
    std::sync::LazyLock::new(Abacus::standard);
pub use STANDARD_ABACUS as STANDARD;
pub use STANDARD_ABACUS as STANDARD_CALC;

impl Default for Abacus {
    fn default() -> Self {
        Abacus::new()
    }
}

/// Evaluates a physical unit or mathematical expression using the standard unit and token registry.
///
/// # Examples
///
/// ```rust
/// use abacus::eval;
///
/// let res = eval("10 N * 5 m").unwrap();
/// assert_eq!(res.to_display(), "50 J");
///
/// let converted = eval("1 km in m").unwrap();
/// assert_eq!(converted.to_display(), "1000 m");
/// ```
pub fn eval(expr: &str) -> Result<EvalResult, AbacusError> {
    STANDARD_ABACUS.eval(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn global_units() -> &'static UnitRegistry {
        &Abacus::shared().units
    }

    pub fn unit(symbol: &str) -> Result<std::sync::Arc<Unit>, AbacusError> {
        global_units().unit(symbol)
    }

    pub fn value(amount: f64, symbol: &str) -> Result<Value, AbacusError> {
        Ok(Value::new(amount, unit(symbol)?))
    }

    #[test]
    fn looks_up_units_by_symbol() {
        assert_eq!(unit("km").unwrap().display.render(), "km");
        assert_eq!(unit("in").unwrap().display.render(), "in");
        assert_eq!(unit("h").unwrap().display.render(), "h");
    }

    #[test]
    fn looks_up_units_by_name() {
        assert_eq!(unit("hour").unwrap().display.render(), "h");
    }

    #[test]
    fn returns_an_error_for_unknown_units() {
        assert_eq!(
            unit("wat").unwrap_err(),
            AbacusError::UnknownUnit("wat".to_string())
        );
        assert_eq!(
            unit("").unwrap_err(),
            AbacusError::UnknownUnit("".to_string())
        );
        assert_eq!(
            unit("KM").unwrap_err(),
            AbacusError::UnknownUnit("KM".to_string())
        );
    }

    #[test]
    fn global_units_is_initialized_once() {
        let first = global_units() as *const _;
        let second = global_units() as *const _;

        assert_eq!(first, second);
        assert!(!global_units().is_empty());
    }

    #[test]
    fn global_units_contains_expected_metric_units() {
        let units = global_units();

        assert!(units.contains("km"));
        assert!(units.contains("in"));
        assert!(units.contains("h"));
        assert!(units.contains("hour"));
    }

    #[test]
    fn repeated_unit_lookups_return_equivalent_units() {
        let first = unit("km").unwrap();
        let second = unit("km").unwrap();

        assert_eq!(first.display.render(), second.display.render());
        assert_eq!(first.dimensions, second.dimensions);
    }

    #[test]
    fn repeated_unit_lookups_share_the_same_unit() {
        let first = unit("km").unwrap();
        let second = unit("km").unwrap();

        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn constructs_values_from_amount_and_unit_symbol() {
        let distance = value(5.0, "km").unwrap();

        assert_eq!(distance.to_display(), "5 km");
        assert_eq!(distance.unit.display.render(), "km");
        assert_eq!(distance.unit.dimensions, Dimensions::LENGTH);
    }

    #[test]
    fn constructs_zero_values() {
        let distance = value(0.0, "km").unwrap();

        assert_eq!(distance.to_display(), "0 km");
    }

    #[test]
    fn constructs_negative_values() {
        let distance = value(-5.0, "km").unwrap();

        assert_eq!(distance.to_display(), "-5 km");
    }

    #[test]
    fn value_propagates_unknown_unit_errors() {
        assert_eq!(
            value(5.0, "wat").unwrap_err(),
            AbacusError::UnknownUnit("wat".to_string())
        );
    }

    #[test]
    fn divides_values_with_different_dimensions() {
        let distance = value(5.0, "km").unwrap();
        let duration = value(1.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "5 km/h");
        assert_eq!(speed.unit.dimensions, Dimensions::LENGTH - Dimensions::TIME);
    }

    #[test]
    fn division_preserves_fractional_amounts() {
        let distance = value(10.0, "km").unwrap();
        let duration = value(4.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "2.5 km/h");
    }

    #[test]
    fn division_by_zero_returns_infinity() {
        let distance = value(5.0, "km").unwrap();
        let zero = value(0.0, "h").unwrap();

        assert_eq!((distance / zero).unwrap().canonical, f64::INFINITY);
    }

    #[test]
    fn division_by_negative_value_is_supported() {
        let distance = value(5.0, "km").unwrap();
        let duration = value(-2.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "-2.5 km/h");
    }

    #[test]
    fn division_with_unknown_left_unit_fails() {
        let invalid = value(5.0, "wat");
        let duration = value(1.0, "h").unwrap();

        assert!(invalid.is_err());
        assert!(invalid.and_then(|distance| distance / duration).is_err());
    }

    #[test]
    fn division_with_unknown_right_unit_fails() {
        let distance = value(5.0, "km").unwrap();
        let invalid = value(1.0, "wat");

        assert!(invalid.is_err());
        assert!(invalid.and_then(|duration| distance / duration).is_err());
    }
}
