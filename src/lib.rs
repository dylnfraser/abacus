#![warn(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod evaluation;

pub mod error;
pub mod registry;
pub mod units;

use std::collections::HashMap;
use std::sync::Arc;

pub use error::{AbacusError, Diagnostic};
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
    binary::operators::BinaryOp, function::operators::FunctionContext,
    function::operators::FunctionOp, function::operators::FunctionTarget,
    token_registry::TokenRegistry, unary::operators::UnaryOp,
};

pub use evaluation::tokenizer::sig_figs::{
    count_significant_figures, min_significant_figures_in_expr,
};
pub use evaluation::tokenizer::tokens::{Span, Token};

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

/// A configured evaluation engine: unit and operator registries, variables and settings.
///
/// Every setting has a consuming builder `with_*` and an in-place `set_*`, plus a getter of the
/// same name as the setting. Optional settings take `impl Into<Option<T>>`, so
/// `with_decimal_places(2)` sets them and `set_decimal_places(None)` clears them.
///
/// ```rust
/// # #[cfg(feature = "units")] {
/// use abacus::{Abacus, AngleMode};
///
/// let mut calc = Abacus::standard().with_angle_mode(AngleMode::Degrees);
/// calc.set_decimal_places(2);
/// assert_eq!(calc.eval("sin(30)").unwrap().to_display(), "0.5");
/// assert_eq!(calc.decimal_places(), Some(2));
/// # }
/// ```
#[derive(Debug)]
pub struct Abacus {
    units: UnitRegistry,
    tokens: TokenRegistry,
    date_format: DateFormat,
    significant_figures: Option<usize>,
    follow_significant_figures: bool,
    auto_derived_units: bool,
    angle_mode: AngleMode,
    strict_dimensions: bool,
    decimal_places: Option<usize>,
    interval_style: Option<IntervalStyle>,
    notation: Notation,
    default_timezone: Option<TimeZone>,
    weekend: WeekendDays,
    max_recursion_depth: usize,
    implicit_multiplication: bool,
    number_scales: bool,
    live_rates: bool,
    currency_cache_path: Option<std::path::PathBuf>,
    unit_display_overrides: HashMap<String, String>,
    variables: HashMap<String, EvalResult>,
    anchor_date: Option<Date>,
    /// Identifies this engine to the expressions it compiles (see [`Expr`]).
    engine_id: u64,
    /// Bumped whenever something that affects how expressions are read changes (units,
    /// operators, variable names, settings), so stale compiled expressions are re-read.
    generation: u64,
}

/// Source of [`Abacus::engine_id`]s.
static NEXT_ENGINE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Generates `with_*` / `set_*` / getter triples for plain `Copy` settings.
macro_rules! settings {
    ($( $(#[$doc:meta])* $field:ident: $ty:ty => $with:ident, $set:ident; )*) => {
        $(
            $(#[$doc])*
            #[must_use]
            pub fn $with(mut self, value: $ty) -> Self {
                self.$field = value;
                self.changed();
                self
            }

            $(#[$doc])*
            pub fn $set(&mut self, value: $ty) -> &mut Self {
                self.$field = value;
                self.changed();
                self
            }

            $(#[$doc])*
            #[must_use]
            pub fn $field(&self) -> $ty {
                self.$field
            }
        )*
    };
}

/// Generates `with_*` / `set_*` / getter triples for optional `Copy` settings.
macro_rules! optional_settings {
    ($( $(#[$doc:meta])* $field:ident: $ty:ty => $with:ident, $set:ident; )*) => {
        $(
            $(#[$doc])*
            #[must_use]
            pub fn $with(mut self, value: impl Into<Option<$ty>>) -> Self {
                self.$field = value.into();
                self.changed();
                self
            }

            $(#[$doc])*
            pub fn $set(&mut self, value: impl Into<Option<$ty>>) -> &mut Self {
                self.$field = value.into();
                self.changed();
                self
            }

            $(#[$doc])*
            #[must_use]
            pub fn $field(&self) -> Option<$ty> {
                self.$field
            }
        )*
    };
}

impl Abacus {
    /// Returns the standard mathematical constants (`pi`, `PI`, `e`, `E`, `tau`, `TAU`, `phi`, `PHI`).
    #[must_use]
    pub fn standard_variables() -> HashMap<String, EvalResult> {
        crate::evaluation::tokenizer::registry::constants::standard_variables_map()
    }

    /// The standard engine; same as [`Abacus::standard`] and [`Abacus::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::standard()
    }

    /// An engine with empty unit and operator registries and no variables: even `+` must be
    /// registered before use. Start from [`Abacus::standard`] to extend the built-ins instead.
    #[must_use]
    pub fn empty() -> Self {
        Self::from_registry(UnitRegistry::new(), TokenRegistry::new())
    }

    /// Initialize a new `Abacus` instance with custom units and tokens.
    #[must_use]
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
            interval_style: None,
            notation: Notation::default(),
            default_timezone: None,
            weekend: WeekendDays::default(),
            max_recursion_depth: crate::evaluation::parser::MAX_RECURSION_DEPTH,
            implicit_multiplication: true,
            number_scales: cfg!(feature = "number-scales"),
            live_rates: false,
            // No implicit disk writes: `update_rates_from_json` only persists when a cache path
            // is configured. Live-rate methods fall back to `default_currency_cache_path()`.
            currency_cache_path: None,
            unit_display_overrides: HashMap::new(),
            variables: HashMap::new(),
            anchor_date: None,
            engine_id: NEXT_ENGINE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            generation: 0,
        }
    }

    /// Records a change that may affect how expressions are read (see [`Expr`]).
    fn changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Defines or updates a variable. Only a new name changes how expressions are read.
    fn insert_variable(&mut self, name: String, value: EvalResult) {
        if self.variables.insert(name, value).is_none() {
            self.changed();
        }
    }

    /// Initialize an `Abacus` instance populated with standard physical units, operator registries, and mathematical constants (`pi`, `e`, `tau`, `phi`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "units")] {
    /// use abacus::Abacus;
    ///
    /// let abacus = Abacus::standard();
    /// let val = abacus.eval_scalar("2 * 5 m").unwrap();
    /// assert_eq!(val.to_display(), "10 m");
    /// # }
    /// ```
    #[must_use]
    pub fn standard() -> Self {
        let mut abacus = Self::from_registry(UnitRegistry::standard(), TokenRegistry::standard());
        abacus.variables = Self::standard_variables();
        abacus
    }

    // ── Registries ──

    /// The unit registry.
    #[must_use]
    pub fn units(&self) -> &UnitRegistry {
        &self.units
    }

    /// Mutable access to the unit registry, e.g. to register units in bulk.
    pub fn units_mut(&mut self) -> &mut UnitRegistry {
        self.changed();
        &mut self.units
    }

    /// The operator and function registry.
    #[must_use]
    pub fn tokens(&self) -> &TokenRegistry {
        &self.tokens
    }

    /// Mutable access to the operator and function registry.
    pub fn tokens_mut(&mut self) -> &mut TokenRegistry {
        self.changed();
        &mut self.tokens
    }

    // ── Settings ──

    settings! {
        /// Output date format. `MMDDYYYY` also reads ambiguous `NN-NN-YYYY` input month-first.
        date_format: DateFormat => with_date_format, set_date_format;
        /// Round results to the fewest significant figures among the input's numeric literals.
        follow_significant_figures: bool => with_follow_significant_figures, set_follow_significant_figures;
        /// Reduce composite results to derived SI units (`N*m` → `J`).
        auto_derived_units: bool => with_auto_derived_units, set_auto_derived_units;
        /// Angle unit assumed for bare numbers passed to `sin`/`cos`/`tan` and returned by the
        /// inverse functions.
        angle_mode: AngleMode => with_angle_mode, set_angle_mode;
        /// Reject adding or subtracting a bare number to a dimensioned value (`5 m + 3`).
        strict_dimensions: bool => with_strict_dimensions, set_strict_dimensions;
        /// Number notation used by [`Abacus::format_result`] and [`Abacus::eval_to_string`].
        notation: Notation => with_notation, set_notation;
        /// Days treated as the weekend in business-day calculations.
        weekend: WeekendDays => with_weekend, set_weekend;
        /// Allow implicit multiplication (`2(3)`, `2 pi`).
        implicit_multiplication: bool => with_implicit_multiplication, set_implicit_multiplication;
        /// Recognise scale words (`million`, `dozen`, ...) as multipliers.
        number_scales: bool => with_number_scales, set_number_scales;
    }

    optional_settings! {
        /// Round results to this many significant figures.
        significant_figures: usize => with_significant_figures, set_significant_figures;
        /// Round results to this many decimal places.
        decimal_places: usize => with_decimal_places, set_decimal_places;
        /// Display style for intervals (`[a, b]` or `a..b`); `None` keeps each interval's own.
        interval_style: IntervalStyle => with_interval_style, set_interval_style;
    }

    /// Timezone for dates and times written without one.
    #[must_use]
    pub fn with_default_timezone(mut self, tz: impl Into<Option<TimeZone>>) -> Self {
        self.set_default_timezone(tz);
        self
    }

    /// Timezone for dates and times written without one.
    pub fn set_default_timezone(&mut self, tz: impl Into<Option<TimeZone>>) -> &mut Self {
        self.default_timezone = tz.into();
        self.changed();
        self
    }

    /// Timezone for dates and times written without one.
    #[must_use]
    pub fn default_timezone(&self) -> Option<&TimeZone> {
        self.default_timezone.as_ref()
    }

    /// Fixes the reference "now" used by `today`, `now`, `ago`, relative weekdays and so on.
    #[must_use]
    pub fn with_now(mut self, now: impl Into<Option<Date>>) -> Self {
        self.set_now(now);
        self
    }

    /// Fixes the reference "now"; `None` goes back to the system clock.
    pub fn set_now(&mut self, now: impl Into<Option<Date>>) -> &mut Self {
        self.anchor_date = now.into();
        self.changed();
        self
    }

    /// The fixed reference "now", if one is set.
    #[must_use]
    pub fn anchor_date(&self) -> Option<&Date> {
        self.anchor_date.as_ref()
    }

    /// Maximum parser nesting depth, clamped to
    /// [`evaluation::parser::pratt::MAX_RECURSION_DEPTH_CAP`] so input cannot overflow the stack.
    #[must_use]
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.set_max_recursion_depth(depth);
        self
    }

    /// Maximum parser nesting depth, clamped to
    /// [`evaluation::parser::pratt::MAX_RECURSION_DEPTH_CAP`] so input cannot overflow the stack.
    pub fn set_max_recursion_depth(&mut self, depth: usize) -> &mut Self {
        self.max_recursion_depth =
            depth.min(crate::evaluation::parser::pratt::MAX_RECURSION_DEPTH_CAP);
        self.changed();
        self
    }

    /// Maximum parser nesting depth.
    #[must_use]
    pub fn max_recursion_depth(&self) -> usize {
        self.max_recursion_depth
    }

    /// Largest exponent magnitude accepted, both for `^` and in unit symbols such as `m^3`.
    /// Clamped to the largest exponent dimensions can represent (≈ 77 471).
    #[must_use]
    pub fn with_max_exponent(mut self, max: f64) -> Self {
        self.set_max_exponent(max);
        self
    }

    /// Largest exponent magnitude accepted (see [`Abacus::with_max_exponent`]).
    pub fn set_max_exponent(&mut self, max: f64) -> &mut Self {
        self.units.set_max_exponent(max);
        self.changed();
        self
    }

    /// Largest exponent magnitude accepted.
    #[must_use]
    pub fn max_exponent(&self) -> f64 {
        self.units.max_exponent()
    }

    /// Enables or disables currency units (`$`, `EUR`, ...).
    #[must_use]
    pub fn with_currencies(mut self, enabled: bool) -> Self {
        self.set_currencies(enabled);
        self
    }

    /// Enables or disables currency units (`$`, `EUR`, ...).
    pub fn set_currencies(&mut self, enabled: bool) -> &mut Self {
        self.changed();
        #[cfg(feature = "currencies")]
        {
            self.units.set_currencies_enabled(enabled);
        }
        #[cfg(not(feature = "currencies"))]
        let _ = enabled;
        self
    }

    /// Whether currency units are registered.
    #[must_use]
    pub fn currencies(&self) -> bool {
        #[cfg(feature = "currencies")]
        {
            self.units.currencies_enabled()
        }
        #[cfg(not(feature = "currencies"))]
        {
            false
        }
    }

    /// File used to cache exchange rates (defaults to [`default_currency_cache_path`] for the
    /// live-rate methods; [`Abacus::update_rates_from_json`] only writes when one is set).
    #[must_use]
    pub fn with_currency_cache(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.set_currency_cache(path);
        self
    }

    /// File used to cache exchange rates (see [`Abacus::with_currency_cache`]).
    pub fn set_currency_cache(&mut self, path: impl Into<std::path::PathBuf>) -> &mut Self {
        self.currency_cache_path = Some(path.into());
        self
    }

    /// File used to cache exchange rates, if one is configured.
    #[must_use]
    pub fn currency_cache_path(&self) -> Option<&std::path::Path> {
        self.currency_cache_path.as_deref()
    }

    /// Enables or disables live exchange rates fetched from the Frankfurter API
    /// (<https://api.frankfurter.dev/v1/latest?base=USD>).
    ///
    /// Fetch failures are ignored here (built-in fallback rates stay in effect); use
    /// [`Abacus::try_set_live_rates`] to observe them.
    #[must_use]
    pub fn with_live_rates(mut self, enabled: bool) -> Self {
        // Errors are intentionally ignored by this builder; see the doc comment.
        let _ = self.try_set_live_rates(enabled);
        self
    }

    /// Sets whether live exchange rates should be used, fetching (or loading cached) rates
    /// immediately when enabling. Returns the fetch error, if any; the flag is set regardless.
    pub fn try_set_live_rates(&mut self, enabled: bool) -> Result<(), AbacusError> {
        self.live_rates = enabled;
        if enabled {
            self.update_daily_rates()
        } else {
            Ok(())
        }
    }

    /// Whether live exchange rates were requested.
    #[must_use]
    pub fn live_rates(&self) -> bool {
        self.live_rates
    }

    #[cfg(feature = "currencies")]
    fn cache_path_or_default(&self) -> std::path::PathBuf {
        self.currency_cache_path
            .clone()
            .unwrap_or_else(default_currency_cache_path)
    }

    /// Updates currency rates according to daily cache policy:
    /// Loads from disk if cache is fresh (< 24 hours); otherwise fetches live rates and updates cache.
    pub fn update_daily_rates(&mut self) -> Result<(), AbacusError> {
        #[cfg(feature = "currencies")]
        {
            let rates = crate::registry::units::currency_units::get_or_update_daily_rates(
                &self.cache_path_or_default(),
            )?;
            self.units.update_currency_rates(&rates);
            self.changed();
            Ok(())
        }
        #[cfg(not(feature = "currencies"))]
        {
            Err(AbacusError::EvaluationError(
                "currencies feature is not enabled".to_string(),
            ))
        }
    }

    /// Fetches live exchange rates from <https://api.frankfurter.dev/v1/latest?base=USD>,
    /// saves them to the configured cache file, and updates the unit registry.
    pub fn fetch_live_rates(&mut self) -> Result<(), AbacusError> {
        #[cfg(feature = "currencies")]
        {
            let rates = crate::registry::units::currency_units::fetch_and_cache_rates(
                &self.cache_path_or_default(),
            )?;
            self.units.update_currency_rates(&rates);
            self.changed();
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
            self.changed();
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
    pub fn set_currency_rate(&mut self, _code: &str, _rate_per_usd: f64) -> &mut Self {
        self.changed();
        #[cfg(feature = "currencies")]
        {
            self.units.set_currency_rate(_code, _rate_per_usd);
        }
        self
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
        crate::evaluation::tokenizer::lexer::tokenize_string_with_anchor(
            &self.tokens,
            &self.units,
            Some(&self.variables),
            expr,
            self.implicit_multiplication,
            self.number_scales,
            self.anchor_date.as_ref(),
        )
    }

    /// Define a variable on this `Abacus` instance (builder pattern).
    #[must_use]
    pub fn with_variable(mut self, name: impl Into<String>, value: impl Into<EvalResult>) -> Self {
        self.insert_variable(name.into(), value.into());
        self
    }

    /// Define a variable in-place on this `Abacus` instance.
    pub fn set_variable(
        &mut self,
        name: impl Into<String>,
        value: impl Into<EvalResult>,
    ) -> &mut Self {
        self.insert_variable(name.into(), value.into());
        self
    }

    /// Define a variable by evaluating an expression string on this `Abacus` instance.
    pub fn set_variable_expr(
        &mut self,
        name: impl Into<String>,
        expr: &str,
    ) -> Result<EvalResult, AbacusError> {
        let raw = self.eval_unformatted(expr)?;
        self.insert_variable(name.into(), raw.clone());
        Ok(self.apply_output_settings(raw, expr))
    }

    /// All defined variables, including the standard constants.
    #[must_use]
    pub fn variables(&self) -> &HashMap<String, EvalResult> {
        &self.variables
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
        let removed = self.variables.remove(name);
        if removed.is_some() {
            self.changed();
        }
        removed
    }

    /// Clear all defined variables.
    pub fn clear_variables(&mut self) {
        self.variables.clear();
        self.changed();
    }

    /// Reset variables back to standard mathematical constants (`pi`, `e`, `tau`, `phi`).
    pub fn reset_standard_variables(&mut self) {
        self.variables = Self::standard_variables();
        self.changed();
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

    /// All configured unit display overrides.
    #[must_use]
    pub fn unit_display_overrides(&self) -> &HashMap<String, String> {
        &self.unit_display_overrides
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
        self.eval_mut_detailed(expr).map_err(Diagnostic::into_error)
    }

    /// [`Abacus::eval_mut`], reporting where in `expr` an error occurred (see
    /// [`Abacus::eval_detailed`]).
    pub fn eval_mut_detailed(&mut self, expr: &str) -> Result<EvalResult, Diagnostic> {
        let trimmed = expr.trim();
        // Check for simple assignment: <ident> = <rhs>
        if let Some((ident, rhs)) = trimmed.split_once('=') {
            let ident = ident.trim();
            let is_ident = !ident.is_empty()
                && (ident.starts_with(|c: char| c.is_alphabetic() || c == '_'))
                && ident.chars().all(|c| c.is_alphanumeric() || c == '_');
            if is_ident && !rhs.starts_with('=') {
                // Store the full-precision value; only the returned copy is rounded for display.
                let raw = self.eval_located(rhs).map_err(|located| {
                    // Spans are relative to `rhs`; report them relative to `expr`.
                    let offset = rhs.as_ptr() as usize - expr.as_ptr() as usize;
                    let inner = located.into_inner();
                    let span = inner
                        .span
                        .map(|s| Span::new(s.start + offset, s.end + offset));
                    Diagnostic::new(inner.error, span, expr).with_note(inner.note)
                })?;
                self.insert_variable(ident.to_string(), raw.clone());
                return Ok(self.apply_output_settings(raw, rhs));
            }
        }
        self.eval_detailed(expr)
    }

    /// Evaluates an expression without applying output settings (decimal places, significant
    /// figures, currency rounding, display overrides). Use this when the result feeds further
    /// computation; [`Abacus::eval`] is this plus [`Abacus::apply_output_settings`].
    pub fn eval_unformatted(&self, expr: &str) -> Result<EvalResult, AbacusError> {
        self.eval_located(expr)
            .map_err(|located| located.into_inner().error)
    }

    /// Evaluates an expression, reporting where in it an error occurred.
    ///
    /// ```rust
    /// # #[cfg(feature = "units")] {
    /// use abacus::{Abacus, AbacusError};
    ///
    /// let err = Abacus::standard().eval_detailed("5 m + 3 s").unwrap_err();
    /// assert_eq!(err.error(), &AbacusError::IncompatibleDimensions);
    /// assert_eq!(err.span().map(|s| s.text(err.source())), Some("+"));
    /// assert_eq!(err.note(), Some("m + s"));
    /// # }
    /// ```
    pub fn eval_detailed(&self, expr: &str) -> Result<EvalResult, Diagnostic> {
        let res = self
            .eval_located(expr)
            .map_err(|located| diagnostic(located, expr))?;
        Ok(self.apply_output_settings(res, expr))
    }

    fn eval_located(
        &self,
        expr: &str,
    ) -> Result<EvalResult, crate::evaluation::parser::ast::Located> {
        let res = crate::evaluation::parser::evaluate_located(
            &self.tokens,
            &self.units,
            Some(&self.variables),
            expr,
            EvalConfig::from(self),
        )?;
        Ok(self.finish_date(res))
    }

    /// Applies the engine's date format and timezone to a date result.
    fn finish_date(&self, mut res: EvalResult) -> EvalResult {
        if let EvalResult::Date(ref mut d) = res {
            if !d.has_explicit_format() {
                d.format = self.date_format;
            }
            if d.timezone.is_none() {
                d.timezone = self.default_timezone.clone();
            }
        }
        res
    }

    /// Parses an expression once so it can be evaluated many times with
    /// [`Abacus::eval_compiled`], e.g. while variables change:
    ///
    /// ```rust
    /// use abacus::Abacus;
    ///
    /// let mut calc = Abacus::standard().with_variable("x", 0.0);
    /// let expr = calc.compile("x^2 + 1").unwrap();
    /// let ys: Vec<String> = (0..4)
    ///     .map(|x| {
    ///         calc.set_variable("x", f64::from(x));
    ///         calc.eval_compiled(&expr).unwrap().to_display()
    ///     })
    ///     .collect();
    /// assert_eq!(ys, ["1", "2", "5", "10"]);
    /// ```
    ///
    /// Which names are variables and which units exist is decided when compiling; if the engine
    /// has changed in a way that affects that since (new variable names, units, operators or
    /// settings), or the expression contains date literals (which depend on the current date),
    /// [`Abacus::eval_compiled`] re-reads the source, so the result is always the same as
    /// [`Abacus::eval`] would give.
    pub fn compile(&self, expr: &str) -> Result<Expr, Diagnostic> {
        let config = EvalConfig::from(self);
        let lazy_now = std::cell::OnceCell::new();
        let compiled = crate::evaluation::parser::compile(
            &self.tokens,
            &self.units,
            Some(&self.variables),
            expr,
            &config,
            &lazy_now,
        )
        .map_err(|located| diagnostic(located, expr))?;
        Ok(Expr {
            source: expr.into(),
            compiled: Arc::new(compiled),
            engine_id: self.engine_id,
            generation: self.generation,
        })
    }

    /// Evaluates a compiled expression with the engine's current variables and settings and
    /// applies its output settings, exactly as [`Abacus::eval`] does for the source text.
    pub fn eval_compiled(&self, expr: &Expr) -> Result<EvalResult, AbacusError> {
        let res = self.eval_compiled_unformatted(expr)?;
        Ok(self.apply_output_settings(res, &expr.source))
    }

    /// [`Abacus::eval_compiled`] without output settings (see [`Abacus::eval_unformatted`]).
    pub fn eval_compiled_unformatted(&self, expr: &Expr) -> Result<EvalResult, AbacusError> {
        if expr.engine_id != self.engine_id
            || expr.generation != self.generation
            || expr.compiled.has_dates
        {
            return self.eval_unformatted(&expr.source);
        }
        let mut config = EvalConfig::from(self);
        if config.anchor_date.is_some() {
            config.anchor_date = Some(Date::reference_now(
                config.anchor_date.as_ref(),
                config.default_timezone.as_ref(),
            ));
        }
        let res = crate::evaluation::parser::run(
            &expr.compiled,
            &self.units,
            Some(&self.variables),
            &config,
        )
        .map_err(|located| located.into_inner().error)?;
        Ok(self.finish_date(res))
    }

    /// Evaluates an expression and applies this instance's output settings: decimal places,
    /// significant figures, default currency rounding, interval style and display overrides.
    ///
    /// The returned values are rounded; variables assigned via [`Abacus::eval_mut`] or
    /// [`Abacus::set_variable_expr`] keep full precision.
    pub fn eval(&self, expr: &str) -> Result<EvalResult, AbacusError> {
        let res = self.eval_unformatted(expr)?;
        Ok(self.apply_output_settings(res, expr))
    }

    /// Applies output rounding and display settings to a result evaluated from `expr`
    /// (`expr` is only consulted when following input significant figures).
    #[must_use]
    pub fn apply_output_settings(&self, res: EvalResult, expr: &str) -> EvalResult {
        // With default settings only currency rounding applies; keep that path small and put
        // the rarely used configuration handling out of line.
        if self.interval_style.is_none()
            && self.decimal_places.is_none()
            && self.significant_figures.is_none()
            && !self.follow_significant_figures
            && self.unit_display_overrides.is_empty()
        {
            return Self::round_currency_default(res);
        }
        self.apply_configured_output_settings(res, expr)
    }

    /// Rounds currency results to their standard number of decimal places.
    fn round_currency_default(res: EvalResult) -> EvalResult {
        let currency_unit = match &res {
            EvalResult::Scalar(v)
                if v.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY =>
            {
                &v.unit
            }
            EvalResult::Interval(inv)
                if inv.lo.unit.dimensions == crate::units::dimensions::Dimensions::CURRENCY =>
            {
                &inv.lo.unit
            }
            _ => return res,
        };
        let mut unit_str = crate::units::value::StackStr::new();
        let _ =
            std::fmt::Write::write_fmt(&mut unit_str, format_args!("{}", currency_unit.display));
        let dec = crate::units::value::currency_decimal_places(unit_str.as_str());
        res.round_to_decimals(dec)
    }

    #[cold]
    #[inline(never)]
    fn apply_configured_output_settings(&self, mut res: EvalResult, expr: &str) -> EvalResult {
        if let Some(style) = self.interval_style {
            res = res.with_interval_style(style);
        }

        if let Some(dec) = self.decimal_places {
            res = res.round_to_decimals(dec);
        } else if self.significant_figures.is_none() && !self.follow_significant_figures {
            res = Self::round_currency_default(res);
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

        res
    }

    /// Evaluates an expression and renders it with every output setting, including
    /// [`Notation`]. Equivalent to [`Abacus::eval`] followed by [`Abacus::format_result`].
    ///
    /// ```rust
    /// use abacus::{Abacus, Notation};
    ///
    /// let calc = Abacus::standard().with_notation(Notation::Scientific);
    /// assert_eq!(calc.eval_to_string("12345").unwrap(), "1.2345e4");
    /// ```
    pub fn eval_to_string(&self, expr: &str) -> Result<String, AbacusError> {
        Ok(self.format_result(&self.eval(expr)?))
    }

    /// Formats an `EvalResult` according to this `Abacus` instance's configuration
    /// (date format, significant figures, decimal places, notation, etc.).
    ///
    /// `EvalResult`'s own `Display` cannot see engine settings, so [`Notation`] only takes
    /// effect here (and in [`Abacus::eval_to_string`]).
    #[must_use]
    pub fn format_result(&self, res: &EvalResult) -> String {
        let mut res = res.clone();
        if let Some(style) = self.interval_style {
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
        self.changed();
        self.units.insert_unit(alias, Arc::from(unit));
    }

    pub fn register_binop_token(&mut self, op: BinaryOp) {
        self.changed();
        self.tokens.register_binary_operator(op.alias, op);
    }

    pub fn register_unop_token(&mut self, op: UnaryOp) {
        self.changed();
        self.tokens.register_unary_operator(op.alias, op);
    }

    pub fn register_function_token(&mut self, op: FunctionOp) {
        self.changed();
        self.tokens.register_function_operator(op.name, op);
    }

    /// Returns a reference to the globally shared standard `Abacus` instance.
    #[must_use]
    pub fn shared() -> &'static Self {
        &STANDARD_ABACUS
    }
}

/// An expression parsed by [`Abacus::compile`], for repeated evaluation with
/// [`Abacus::eval_compiled`]. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Expr {
    source: Box<str>,
    compiled: Arc<crate::evaluation::parser::Compiled>,
    engine_id: u64,
    generation: u64,
}

impl Expr {
    /// The expression's source text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.source)
    }
}

fn diagnostic(located: crate::evaluation::parser::ast::Located, source: &str) -> Diagnostic {
    let inner = located.into_inner();
    Diagnostic::new(inner.error, inner.span, source).with_note(inner.note)
}

/// The process-wide standard `Abacus` instance used by [`eval`] and [`Abacus::shared`].
pub static STANDARD_ABACUS: std::sync::LazyLock<Abacus> =
    std::sync::LazyLock::new(Abacus::standard);

/// The standard engine, as [`Abacus::standard`]. Use [`Abacus::empty`] for bare registries.
impl Default for Abacus {
    fn default() -> Self {
        Abacus::standard()
    }
}

/// Evaluates a physical unit or mathematical expression using the standard unit and token registry.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "units")] {
/// use abacus::eval;
///
/// let res = eval("10 N * 5 m").unwrap();
/// assert_eq!(res.to_display(), "50 J");
///
/// let converted = eval("1 km in m").unwrap();
/// assert_eq!(converted.to_display(), "1000 m");
/// # }
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

    #[cfg(feature = "units")]
    #[test]
    fn looks_up_units_by_symbol() {
        assert_eq!(unit("km").unwrap().display.render(), "km");
        assert_eq!(unit("in").unwrap().display.render(), "in");
        assert_eq!(unit("h").unwrap().display.render(), "h");
    }

    #[cfg(feature = "units")]
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

    #[cfg(feature = "units")]
    #[test]
    fn global_units_is_initialized_once() {
        let first = global_units() as *const _;
        let second = global_units() as *const _;

        assert_eq!(first, second);
        assert!(!global_units().is_empty());
    }

    #[cfg(feature = "units")]
    #[test]
    fn global_units_contains_expected_metric_units() {
        let units = global_units();

        assert!(units.contains("km"));
        assert!(units.contains("in"));
        assert!(units.contains("h"));
        assert!(units.contains("hour"));
    }

    #[cfg(feature = "units")]
    #[test]
    fn repeated_unit_lookups_return_equivalent_units() {
        let first = unit("km").unwrap();
        let second = unit("km").unwrap();

        assert_eq!(first.display.render(), second.display.render());
        assert_eq!(first.dimensions, second.dimensions);
    }

    #[cfg(feature = "units")]
    #[test]
    fn repeated_unit_lookups_share_the_same_unit() {
        let first = unit("km").unwrap();
        let second = unit("km").unwrap();

        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }

    #[cfg(feature = "units")]
    #[test]
    fn constructs_values_from_amount_and_unit_symbol() {
        let distance = value(5.0, "km").unwrap();

        assert_eq!(distance.to_display(), "5 km");
        assert_eq!(distance.unit.display.render(), "km");
        assert_eq!(distance.unit.dimensions, Dimensions::LENGTH);
    }

    #[cfg(feature = "units")]
    #[test]
    fn constructs_zero_values() {
        let distance = value(0.0, "km").unwrap();

        assert_eq!(distance.to_display(), "0 km");
    }

    #[cfg(feature = "units")]
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

    #[cfg(feature = "units")]
    #[test]
    fn divides_values_with_different_dimensions() {
        let distance = value(5.0, "km").unwrap();
        let duration = value(1.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "5 km/h");
        assert_eq!(speed.unit.dimensions, Dimensions::LENGTH - Dimensions::TIME);
    }

    #[cfg(feature = "units")]
    #[test]
    fn division_preserves_fractional_amounts() {
        let distance = value(10.0, "km").unwrap();
        let duration = value(4.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "2.5 km/h");
    }

    #[cfg(feature = "units")]
    #[test]
    fn division_by_zero_returns_infinity() {
        let distance = value(5.0, "km").unwrap();
        let zero = value(0.0, "h").unwrap();

        assert_eq!((distance / zero).unwrap().canonical, f64::INFINITY);
    }

    #[cfg(feature = "units")]
    #[test]
    fn division_by_negative_value_is_supported() {
        let distance = value(5.0, "km").unwrap();
        let duration = value(-2.0, "h").unwrap();
        let speed = (distance / duration).unwrap();

        assert_eq!(speed.to_display(), "-2.5 km/h");
    }

    #[cfg(feature = "units")]
    #[test]
    fn division_with_unknown_left_unit_fails() {
        let invalid = value(5.0, "wat");
        let duration = value(1.0, "h").unwrap();

        assert!(invalid.is_err());
        assert!(invalid.and_then(|distance| distance / duration).is_err());
    }

    #[cfg(feature = "units")]
    #[test]
    fn division_with_unknown_right_unit_fails() {
        let distance = value(5.0, "km").unwrap();
        let invalid = value(1.0, "wat");

        assert!(invalid.is_err());
        assert!(invalid.and_then(|duration| distance / duration).is_err());
    }

    #[cfg(feature = "units")]
    #[test]
    fn consolidates_mixed_units_of_same_dimension() {
        assert_eq!(
            crate::eval("2342km * m^2").unwrap().to_display(),
            "0.002342 km^3"
        );
        assert_eq!(
            crate::eval("2342km * m").unwrap().to_display(),
            "2.342 km^2"
        );
        assert_eq!(
            crate::eval("1 m^2 * 2342km").unwrap().to_display(),
            "2342000 m^3"
        );
        assert_eq!(crate::eval("5 m * 2 km").unwrap().to_display(), "10000 m^2");
        assert_eq!(
            crate::eval("10 km * 5 m").unwrap().to_display(),
            "0.05 km^2"
        );
        assert_eq!(crate::eval("2 ft * 6 in").unwrap().to_display(), "1 ft^2");
        assert_eq!(crate::eval("5 h * 30 min").unwrap().to_display(), "2.5 h^2");
    }

    #[cfg(feature = "units")]
    fn display_and_amount(expr: &str) -> (String, f64) {
        match crate::eval(expr).unwrap() {
            EvalResult::Scalar(v) => (v.to_display(), v.amount()),
            other => panic!("expected scalar for {expr}, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "units")]
    fn explicit_conversion_target_is_not_consolidated() {
        assert_eq!(
            crate::eval("1 km * 1 m to km*m").unwrap().to_display(),
            "1 km*m"
        );
        assert_eq!(
            crate::eval("100 km/h / 5 s to km/h/s")
                .unwrap()
                .to_display(),
            "20 km/h*s"
        );
    }

    #[test]
    #[cfg(feature = "units")]
    fn denominator_only_units_are_kept() {
        assert_eq!(crate::eval("2 / 1 km").unwrap().to_display(), "2 1/km");
        assert_eq!(
            crate::eval("1 / 5 s to 1/s").unwrap().to_display(),
            "0.2 1/s"
        );
        assert_eq!(
            crate::eval("1 / (1 km * 1 m)").unwrap().to_display(),
            "1000 1/km^2"
        );
    }

    #[test]
    #[cfg(feature = "units")]
    fn consolidation_uses_first_written_unit_after_cancellation() {
        assert_eq!(
            crate::eval("2 s * 3 km * 4 m / 1 s").unwrap().to_display(),
            "0.024 km^2"
        );
        assert_eq!(
            crate::eval("2 km * 3 s * 4 m / 1 s").unwrap().to_display(),
            "0.024 km^2"
        );
        assert_eq!(
            crate::eval("1 m * 1 km * 1 cm / 1 mm")
                .unwrap()
                .to_display(),
            "0.01 km^2"
        );
    }

    #[test]
    #[cfg(feature = "units")]
    fn units_with_very_different_scales_are_not_consolidated() {
        assert_eq!(crate::eval("1 au * 1 cm").unwrap().to_display(), "1 au*cm");
        assert_eq!(crate::eval("1 mm * 1 ly").unwrap().to_display(), "1 mm*ly");
        assert_eq!(
            crate::eval("1 km * 1 mm").unwrap().to_display(),
            "0.000001 km^2"
        );
    }

    #[test]
    #[cfg(feature = "units")]
    fn consolidation_does_not_add_float_noise() {
        assert_eq!(
            display_and_amount("1 L * 1 mL"),
            ("0.001 L^2".to_string(), 0.001)
        );
        assert_eq!(
            display_and_amount("2 ft * 6 in"),
            ("1 ft^2".to_string(), 1.0)
        );
        assert_eq!(display_and_amount("1 ft * 1 in").1, 1.0 / 12.0);
    }

    #[test]
    #[cfg(all(feature = "units", feature = "currencies", feature = "date"))]
    fn currencies_and_business_days_are_not_consolidated() {
        assert_eq!(
            crate::eval("5 USD * 3 EUR").unwrap().to_display(),
            "15 $*EUR"
        );
        assert_eq!(
            crate::eval("1 h * 2 bdays").unwrap().to_display(),
            "2 h*bdays"
        );
    }

    #[test]
    #[cfg(feature = "units")]
    fn integer_powers_consolidate_like_products() {
        let mut abacus = Abacus::standard();
        abacus.eval_mut("x = 5 km").unwrap();
        let power = abacus.eval_mut("x^2 * 3 m").unwrap().to_display();
        let product = abacus.eval_mut("x * x * 3 m").unwrap().to_display();
        assert_eq!(power, "0.075 km^3");
        assert_eq!(power, product);
        assert_eq!(abacus.eval_mut("x^-1").unwrap().to_display(), "0.2 1/km");
    }
}
