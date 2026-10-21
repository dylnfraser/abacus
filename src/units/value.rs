use crate::registry::UnitRegistry;
use crate::units::dimensions::Dimensions;
use crate::units::unit::UnitExpr;
use crate::{error::AbacusError, units::unit::Unit};

use std::{
    fmt::{self, Write as _},
    ops::{Add, Div, Mul, Sub},
    sync::Arc,
};

/// A dimensional quantity consisting of a canonical SI scalar and an associated physical unit.
///
/// # Affine units (°C, °F)
///
/// Affine temperatures follow these rules:
/// - `-10 °C`, `2 * 10 °C`, `20 °C / 2` scale the displayed amount (`-10 °C`, `20 °C`, `10 °C`).
/// - `°C - °C` yields a temperature *difference* in kelvin (`°F - °F` in degrees Rankine).
/// - `°C ± K` (or `°C ± n`) shifts the temperature by a difference and stays in `°C`.
/// - `°C + °C`, `°C * m`, and powers of affine units are errors.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "units")] {
/// use abacus::Abacus;
///
/// let abacus = Abacus::standard();
/// let length = abacus.units.value(5.0, "km").unwrap();
/// assert_eq!(length.to_display(), "5 km");
/// assert_eq!(length.canonical, 5000.0);
/// # }
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Value {
    pub canonical: f64,
    pub unit: Arc<Unit>,
}

/// Values are equal when their dimensions match and their canonical magnitudes agree to a
/// symmetric relative tolerance of `1e-12` (equal infinities compare equal; NaN never does).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.canonical, other.canonical);
        self.unit.dimensions == other.unit.dimensions
            && (a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs()))
    }
}

/// Shared dimensionless unit; equivalent to [`Unit::dimensionless_arc`].
pub static DIMENSIONLESS_UNIT: std::sync::LazyLock<Arc<Unit>> =
    std::sync::LazyLock::new(Unit::dimensionless_arc);

/// Results of addition/subtraction smaller than this (relative to the operands) are
/// floating-point cancellation noise and are snapped to zero (`0.1 + 0.2 - 0.3` → `0`).
const CANCELLATION_EPSILON: f64 = 4.0 * f64::EPSILON;

fn snap_cancellation(result: f64, a: f64, b: f64) -> f64 {
    if result != 0.0 && result.abs() <= CANCELLATION_EPSILON * a.abs().max(b.abs()) {
        0.0
    } else {
        result
    }
}

impl Value {
    #[must_use]
    pub fn new(value: f64, unit: Arc<Unit>) -> Self {
        Self {
            canonical: value * unit.scalar + unit.offset,
            unit,
        }
    }

    /// Creates a dimensionless `Value` with the given scalar.
    #[must_use]
    pub fn dimensionless(val: f64) -> Self {
        Self {
            canonical: val,
            unit: Unit::dimensionless_arc(),
        }
    }

    /// Returns the display-unit amount: `(canonical − offset) / scalar`.
    #[inline]
    #[must_use]
    pub fn amount(&self) -> f64 {
        (self.canonical - self.unit.offset) / self.unit.scalar
    }

    /// Constructs a `Value` directly from a pre-computed canonical value.
    #[inline]
    #[must_use]
    pub fn from_canonical(canonical: f64, unit: Arc<Unit>) -> Self {
        Self { canonical, unit }
    }

    pub fn convert_to(&self, unit: Arc<Unit>) -> Result<Self, AbacusError> {
        if !self.unit.is_compatible_with(&unit) {
            if self.unit.is_dimensionless() {
                let amount = self.amount();
                return Ok(Self::new(amount, unit));
            }
            return Err(AbacusError::IncompatibleDimensions);
        }

        Ok(Self {
            canonical: self.canonical,
            unit,
        })
    }

    pub fn to(&self, registry: &UnitRegistry, symbol: &str) -> Result<Self, AbacusError> {
        self.convert_to(registry.unit(symbol)?)
    }

    #[deprecated(since = "0.1.0", note = "use `Value::to`")]
    pub fn as_unit(&self, registry: &UnitRegistry, symbol: &str) -> Result<Self, AbacusError> {
        self.to(registry, symbol)
    }

    /// Reduces a composite unit to a matching derived SI unit (`kg*m/s^2` → `N`).
    ///
    /// Units the user wrote as a single symbol (`1 atm`, `3 GHz`) are left alone, as are
    /// composites containing dimensionless angle-like units (`rad/s` stays `rad/s`, not `Hz`).
    pub fn to_derived(&self, registry: &UnitRegistry) -> Result<Self, AbacusError> {
        if self.unit.is_dimensionless() || self.unit.display.is_atomic() || self.unit.is_affine() {
            return Ok(self.clone());
        }
        let has_dimensionless_symbol = self
            .unit
            .display
            .numerator
            .iter()
            .chain(self.unit.display.denominator.iter())
            .any(|sym| registry.get(sym).is_some_and(|u| u.is_dimensionless()));
        if has_dimensionless_symbol {
            return Ok(self.clone());
        }
        if let Some(derived_unit) = registry.find_unit_by_dimensions(&self.unit.dimensions) {
            self.convert_to(derived_unit)
        } else {
            Ok(self.clone())
        }
    }

    #[must_use]
    pub fn to_display(&self) -> String {
        // Format on the stack, then allocate the result once at its exact size.
        let mut buf = StackStr::new();
        let _ = write!(buf, "{self}");
        buf.as_str().to_string()
    }

    #[must_use]
    pub fn to_units_display(&self) -> String {
        self.unit.display.render()
    }

    pub fn simplify_unit_display(&mut self, unit_registry: &UnitRegistry) {
        self.simplify_unit_display_with_options(unit_registry, true);
    }

    /// Simplifies the unit display; `consolidate` controls whether same-dimension units
    /// are merged (disabled when the user explicitly requested a target unit).
    pub(crate) fn simplify_unit_display_with_options(
        &mut self,
        unit_registry: &UnitRegistry,
        consolidate: bool,
    ) {
        if self.unit.is_dimensionless() && self.unit.display.is_empty() {
            return;
        }
        let unit = self
            .unit
            .simplify_display_with_options(|sym| unit_registry.get(sym), consolidate);
        if unit != *self.unit {
            let old_amount = self.amount();
            let factor = unit.scalar / self.unit.scalar;
            self.unit = Arc::new(unit);
            if factor != 1.0 {
                self.align_amount(snap_float_noise(old_amount / snap_float_noise(factor)));
            }
        }
    }

    /// Re-expressing an amount in a different display unit divides by a product of
    /// inexact conversion factors, which leaves noise in the last bits (e.g. `0.0009999999999999998`
    /// instead of `0.001`). Nudge `canonical` by at most a few ULPs so `amount()` equals `target`,
    /// the amount computed directly from the previous display. Leaves `canonical` unchanged if
    /// no nearby value round-trips.
    fn align_amount(&mut self, target: f64) {
        const MAX_NUDGE_ULPS: usize = 8;
        if !target.is_finite() || !self.canonical.is_finite() || self.amount() == target {
            return;
        }
        let amount_of = |canonical: f64| (canonical - self.unit.offset) / self.unit.scalar;
        let start = target * self.unit.scalar + self.unit.offset;
        if (start - self.canonical).abs() > 1e-14 * self.canonical.abs() {
            return;
        }
        let (mut up, mut down) = (start, start);
        for _ in 0..=MAX_NUDGE_ULPS {
            for candidate in [up, down] {
                if amount_of(candidate) == target {
                    self.canonical = candidate;
                    return;
                }
            }
            up = up.next_up();
            down = down.next_down();
        }
    }

    #[must_use]
    pub fn to_human_display(&self) -> String {
        if self.unit.dimensions == Dimensions::TIME {
            format_human_duration(self.canonical)
        } else {
            self.to_display()
        }
    }

    /// Returns a new `Value` with the displayed quantity rounded to `sig_figs` significant figures.
    #[must_use]
    pub fn round_to_sig_figs(&self, sig_figs: usize) -> Self {
        let amt = self.amount();
        let rounded_amt = round_f64_sig_figs(amt, sig_figs);
        let new_canonical = rounded_amt * self.unit.scalar + self.unit.offset;
        Self {
            canonical: new_canonical,
            unit: Arc::clone(&self.unit),
        }
    }

    /// Renders this `Value` formatted to `sig_figs` significant figures.
    #[must_use]
    pub fn to_display_with_sig_figs(&self, sig_figs: usize) -> String {
        let formatted_amt = format_f64_sig_figs(self.amount(), sig_figs);
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Returns a new `Value` with the displayed quantity rounded to `decimals` decimal places.
    ///
    /// Values too large for the requested precision to matter (or that would overflow when
    /// scaled) are returned unchanged.
    #[must_use]
    pub fn round_to_decimals(&self, decimals: usize) -> Self {
        let amt = self.amount();
        let Ok(exp) = i32::try_from(decimals) else {
            return self.clone();
        };
        let scale = 10.0f64.powi(exp);
        let scaled = amt * scale;
        // Beyond 2^53 every f64 is already an integer, so rounding is a no-op.
        if !scale.is_finite() || !scaled.is_finite() || scaled.abs() >= 9_007_199_254_740_992.0 {
            return self.clone();
        }
        let rounded_amt = scaled.round() / scale;
        let new_canonical = rounded_amt * self.unit.scalar + self.unit.offset;
        Self {
            canonical: new_canonical,
            unit: Arc::clone(&self.unit),
        }
    }

    /// Renders this `Value` formatted to `decimals` decimal places.
    #[must_use]
    pub fn to_display_with_decimals(&self, decimals: usize) -> String {
        let formatted_amt = format!("{:.precision$}", self.amount(), precision = decimals);
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Renders this `Value` in scientific notation.
    #[must_use]
    pub fn to_display_scientific(&self) -> String {
        let formatted_amt = format!("{:e}", self.amount());
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Renders this `Value` in engineering notation (powers of 10 that are multiples of 3).
    #[must_use]
    pub fn to_display_engineering(&self) -> String {
        let formatted_amt = format_engineering(self.amount());
        let unit_str = self.unit.display.render();
        format_with_unit(&formatted_amt, &unit_str)
    }

    /// Returns a copy of this `Value` with its rendered unit replaced if it matches
    /// any entry in `overrides` (e.g. `"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`).
    #[must_use]
    pub fn with_display_override(
        &self,
        overrides: &std::collections::HashMap<String, String>,
    ) -> Self {
        if overrides.is_empty() || self.unit.display.is_empty() {
            return self.clone();
        }
        let current_unit_str = self.unit.display.render();
        if let Some(replacement) = overrides.get(&current_unit_str) {
            let mut new_unit = (*self.unit).clone();
            new_unit.display = UnitExpr::single(replacement);
            Self {
                canonical: self.canonical,
                unit: Arc::new(new_unit),
            }
        } else {
            self.clone()
        }
    }
}

/// Returns true if `sym` should be rendered as a prefix before the number (e.g. `$`, `€`, `£`, `¥`).
#[must_use]
pub fn is_prefix_symbol(sym: &str) -> bool {
    matches!(
        sym,
        "$" | "€" | "£" | "¥" | "₹" | "₩" | "₺" | "₪" | "฿" | "R$"
    )
}

/// Returns the standard number of decimal places for a currency (0 for JPY, KRW, etc., 2 for USD, EUR, GBP, etc.).
#[must_use]
pub fn currency_decimal_places(unit_str: &str) -> usize {
    match unit_str {
        "JPY" | "jpy" | "yen" | "¥" | "KRW" | "krw" | "won" | "₩" | "HUF" | "huf" | "IDR"
        | "idr" | "rupiah" | "ISK" | "isk" => 0,
        _ => 2,
    }
}

/// Helper to format a value with its unit.
#[must_use]
pub fn format_with_unit(amount: &str, unit: &str) -> String {
    let mut s = String::with_capacity(amount.len() + unit.len() + 2);
    let _ = write_with_unit(&mut s, amount, unit);
    s
}

/// Writes `amount` with its `unit` as [`format_with_unit`] does, without allocating.
fn write_with_unit<W: fmt::Write>(out: &mut W, amount: &str, unit: &str) -> fmt::Result {
    if unit.is_empty() {
        out.write_str(amount)
    } else if is_prefix_symbol(unit) {
        if let Some(stripped) = amount.strip_prefix('-') {
            write!(out, "-{unit}{stripped}")
        } else {
            write!(out, "{unit}{amount}")
        }
    } else if unit == "%" {
        write!(out, "{amount}%")
    } else if unit == "+%" {
        if amount.starts_with('-') {
            write!(out, "{amount}%")
        } else {
            write!(out, "+{amount}%")
        }
    } else {
        write!(out, "{amount} {unit}")
    }
}

/// Formats a number for display.
///
/// Values within `1e-12` (relative) of an integer print as that integer; magnitudes below
/// `1e-6` or at/above `1e21` use scientific notation (`1.6e-19`, `1e300`); everything else
/// uses the shortest round-trip decimal representation, except that float noise in the last
/// bits is hidden: when the shortest representation needs 16+ significant digits and a
/// rendering of at most 12 significant digits is within 2 ulps of the value, the shorter one is
/// printed (`2.2679618500000003` → `2.26796185`, `0.1 + 0.2` → `0.3`). Genuine 16–17 digit
/// values such as `27.77777777777778` or `78.53981633974483` are unaffected.
#[must_use]
pub fn format_number(v: f64) -> String {
    format_number_with(v, true)
}

/// Like [`format_number`], but always prints the exact shortest round-trip representation
/// (no float-noise suppression). Used for interval bounds, where outward rounding by one ulp
/// is meaningful and must stay visible.
#[must_use]
pub fn format_number_exact(v: f64) -> String {
    format_number_with(v, false)
}

fn format_number_with(v: f64, denoise: bool) -> String {
    let mut s = String::new();
    let _ = write_number(&mut s, v, denoise);
    s
}

/// Writes `v` as [`format_number`] / [`format_number_exact`] would, without allocating.
fn write_number<W: fmt::Write>(out: &mut W, v: f64, denoise: bool) -> fmt::Result {
    if !v.is_finite() {
        return write!(out, "{v}");
    }
    if v == 0.0 {
        return out.write_str("0");
    }
    let a = v.abs();
    let v = if denoise { denoised(v) } else { v };
    if !(1e-6..1e21).contains(&a) {
        return write!(out, "{v:e}");
    }
    let r = v.round();
    if (v - r).abs() <= 1e-12 * a {
        write!(out, "{r}")
    } else {
        write!(out, "{v}")
    }
}

/// Hides float noise: when `v`'s shortest round-trip form needs 16+ significant digits but a
/// rendering with at most [`NOISE_MAX_DIGITS`] digits is within 2 ulps of `v`, returns the float
/// nearest that short rendering; otherwise `v` itself.
///
/// The digit cap is what separates noise from real precision: a genuinely 16–17 digit value
/// (like `pi * 25`) almost never sits within 2 ulps of a 12-digit decimal, while accumulated
/// rounding error on a short decimal (`0.1 + 0.2`, `5 lb in kg`) always does.
fn denoised(v: f64) -> f64 {
    let mut sci = StackStr::new();
    let _ = write!(sci, "{v:e}");
    let mantissa = sci.as_str().split('e').next().unwrap_or("");
    let sig_digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
    if sig_digits < 16 {
        return v;
    }
    for precision in 0..NOISE_MAX_DIGITS {
        // Cheap arithmetic screen: if a `precision + 1` digit decimal is within 2 ulps of `v`,
        // `v * 10^k` is within a few ulps of an integer, so rounding arithmetically lands within
        // a few ulps of `v` too. Candidates far outside that can't pass the exact check below.
        let approx = round_f64_sig_figs(v, precision + 1);
        if !approx.is_finite() || ulp_distance(approx, v) > NOISE_SCREEN_ULPS {
            continue;
        }
        let mut buf = StackStr::new();
        let _ = write!(buf, "{v:.precision$e}");
        let Ok(shorter) = buf.as_str().parse::<f64>() else {
            return v;
        };
        if shorter.is_finite() && shorter.signum() == v.signum() && ulp_distance(shorter, v) <= 2 {
            return shorter;
        }
    }
    v
}

/// Maximum significant digits of a "clean" decimal that float noise is snapped back to.
const NOISE_MAX_DIGITS: usize = 12;

/// Screening tolerance for [`denoised`]; generous compared to the few ulps of error in
/// [`round_f64_sig_figs`], so it never rejects a candidate the exact check would accept.
const NOISE_SCREEN_ULPS: u64 = 64;

/// A small stack buffer implementing [`fmt::Write`], for formatting numbers and unit symbols
/// without heap allocation. Content that doesn't fit spills to a `String`.
pub(crate) struct StackStr {
    buf: [u8; 64],
    len: usize,
    spill: Option<String>,
}

impl StackStr {
    pub(crate) const fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
            spill: None,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match &self.spill {
            Some(s) => s,
            // Only whole `&str`s are ever copied in, so the bytes are valid UTF-8.
            None => std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default(),
        }
    }
}

impl fmt::Write for StackStr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(spill) = &mut self.spill {
            spill.push_str(s);
        } else if self.len + s.len() <= self.buf.len() {
            self.buf[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
            self.len += s.len();
        } else {
            let mut spill = String::with_capacity(self.len + s.len() + 16);
            spill.push_str(self.as_str());
            spill.push_str(s);
            self.spill = Some(spill);
        }
        Ok(())
    }
}

/// Number of representable doubles between two finite values of the same sign.
fn ulp_distance(a: f64, b: f64) -> u64 {
    a.abs().to_bits().abs_diff(b.abs().to_bits())
}

/// Formats a number in engineering notation where exponents are multiples of 3.
#[must_use]
pub fn format_engineering(val: f64) -> String {
    if !val.is_finite() || val == 0.0 {
        return val.to_string();
    }
    let abs_val = val.abs();
    #[allow(clippy::cast_possible_truncation)]
    let exp = abs_val.log10().floor() as i32;
    let eng_exp = if exp >= 0 {
        (exp / 3) * 3
    } else {
        ((exp - 2) / 3) * 3
    };
    let mantissa = val / 10.0f64.powi(eng_exp);
    let mantissa_rounded = (mantissa * 1e12).round() / 1e12;
    if eng_exp == 0 {
        format!("{mantissa_rounded}")
    } else {
        format!("{mantissa_rounded}e{eng_exp}")
    }
}

/// Snaps a value that is within float noise of a 12-significant-figure value
/// (e.g. `ft/in` = `12.000000000000002`) to that value. Genuine long fractions such as
/// `1/12` differ from their 12-figure rounding by far more than the tolerance and are kept.
fn snap_float_noise(value: f64) -> f64 {
    let snapped = round_f64_sig_figs(value, 12);
    if (snapped - value).abs() <= 1e-14 * value.abs() {
        snapped
    } else {
        value
    }
}

/// Rounds a floating-point number to a specified number of significant figures.
#[must_use]
pub fn round_f64_sig_figs(val: f64, sig_figs: usize) -> f64 {
    if sig_figs == 0 || !val.is_finite() || val == 0.0 {
        return val;
    }
    let magnitude = val.abs().log10().floor();
    let scale = 10.0f64.powf(sig_figs as f64 - 1.0 - magnitude);
    if !scale.is_finite() || scale == 0.0 {
        return val;
    }
    let scaled = val * scale;
    if !scaled.is_finite() {
        return val;
    }
    scaled.round() / scale
}

/// Formats a floating-point number to a specified number of significant figures,
/// preserving trailing zeros.
#[must_use]
pub fn format_f64_sig_figs(val: f64, sig_figs: usize) -> String {
    if sig_figs == 0 || !val.is_finite() {
        return val.to_string();
    }
    if val == 0.0 {
        if sig_figs > 1 {
            return format!("0.{:0<width$}", "", width = sig_figs - 1);
        }
        return "0".to_string();
    }

    let rounded = round_f64_sig_figs(val, sig_figs);
    if rounded == 0.0 {
        if sig_figs > 1 {
            return format!("0.{:0<width$}", "", width = sig_figs - 1);
        }
        return "0".to_string();
    }

    #[allow(clippy::cast_possible_truncation)]
    let magnitude = rounded.abs().log10().floor() as i32;
    let sig_figs_i32 = i32::try_from(sig_figs).unwrap_or(i32::MAX);

    if magnitude >= sig_figs_i32 - 1 {
        if rounded.abs() >= 1e15 {
            format!("{:.precision$e}", rounded, precision = sig_figs - 1)
        } else {
            format!("{rounded:.0}")
        }
    } else {
        #[allow(clippy::cast_sign_loss)]
        let decimals = (sig_figs_i32 - 1 - magnitude) as usize;
        if rounded.abs() < 1e-4 {
            format!("{:.precision$e}", rounded, precision = sig_figs - 1)
        } else {
            format!("{rounded:.decimals$}")
        }
    }
}

/// Formats a duration in seconds as `"1 day, 2 hours, 3 minutes"`.
///
/// Years are counted as 365 days. Non-finite inputs render as `"NaN seconds"`,
/// `"inf seconds"` or `"-inf seconds"`.
#[must_use]
pub fn format_human_duration(seconds_f64: f64) -> String {
    const DURATION_UNITS: &[(&str, i64)] = &[
        ("year", 31_536_000),
        ("day", 86_400),
        ("hour", 3_600),
        ("minute", 60),
        ("second", 1),
    ];

    if !seconds_f64.is_finite() {
        return format!("{seconds_f64} seconds");
    }
    let is_negative = seconds_f64 < 0.0;
    #[allow(clippy::cast_possible_truncation)]
    let mut total = seconds_f64.abs().round() as i64;
    if total == 0 {
        return "0 seconds".to_string();
    }

    let mut parts = Vec::new();
    for &(name, divisor) in DURATION_UNITS {
        let count = total / divisor;
        total %= divisor;
        if count > 0 {
            parts.push(format!(
                "{count} {name}{suffix}",
                suffix = if count == 1 { "" } else { "s" }
            ));
        }
    }

    let result = parts.join(", ");
    if is_negative {
        format!("-{result}")
    } else {
        result
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with(f, true)
    }
}

/// Displays a [`Value`] with its exact shortest round-trip number (see [`format_number_exact`]).
pub(crate) struct ExactDisplay<'a>(pub &'a Value);

impl fmt::Display for ExactDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_with(f, false)
    }
}

impl Value {
    fn fmt_with(&self, f: &mut fmt::Formatter<'_>, denoise: bool) -> fmt::Result {
        let value = self.amount();
        let mut unit_buf = StackStr::new();
        write!(unit_buf, "{}", self.unit.display)?;
        let unit_str = unit_buf.as_str();
        let is_integer = value.is_finite() && (value - value.round()).abs() <= 1e-12 * value.abs();
        let mut num = StackStr::new();
        if !is_integer
            && value.is_finite()
            && value.abs() < 1e21
            && self.unit.dimensions == Dimensions::CURRENCY
        {
            let dec = currency_decimal_places(unit_str);
            write!(num, "{value:.dec$}")?;
        } else {
            write_number(&mut num, value, denoise)?;
        }
        write_with_unit(f, num.as_str(), unit_str)
    }
}

/// A temperature-difference unit matching the degree size of `affine` (K for °C, °R for °F).
fn temperature_delta_unit(affine: &Unit) -> Arc<Unit> {
    let (scalar, display) = if (affine.scalar - 5.0 / 9.0).abs() < 1e-12 {
        (affine.scalar, "°R")
    } else {
        (1.0, "K")
    };
    Arc::new(Unit {
        dimensions: affine.dimensions,
        scalar,
        offset: 0.0,
        display: UnitExpr::single(display),
    })
}

impl Value {
    /// Resolves canonical operand values and target unit for additive operations (+ and -),
    /// handling dimensionless promotion and percentage unit normalization.
    fn resolve_additive_operands(&self, rhs: &Value) -> Result<(f64, f64, Arc<Unit>), AbacusError> {
        if !self.unit.is_compatible_with(&rhs.unit) {
            if rhs.unit.is_dimensionless() && !self.unit.is_dimensionless() {
                let rhs_promoted = Value::new(rhs.amount(), Arc::clone(&self.unit));
                return Ok((
                    self.canonical,
                    rhs_promoted.canonical,
                    Arc::clone(&self.unit),
                ));
            } else if self.unit.is_dimensionless() && !rhs.unit.is_dimensionless() {
                let self_promoted = Value::new(self.amount(), Arc::clone(&rhs.unit));
                return Ok((
                    self_promoted.canonical,
                    rhs.canonical,
                    Arc::clone(&rhs.unit),
                ));
            }
            return Err(AbacusError::IncompatibleDimensions);
        }

        let unit = if self.unit.display.is_single("+%") && rhs.unit.display.is_single("%") {
            Arc::clone(&rhs.unit)
        } else {
            Arc::clone(&self.unit)
        };

        Ok((self.canonical, rhs.canonical, unit))
    }

    /// Shifts an affine temperature by `delta` (a temperature difference or a bare number of
    /// degrees), keeping the affine unit.
    fn shift_affine(affine: &Value, delta: &Value, sign: f64) -> Result<Value, AbacusError> {
        let shift = if delta.unit.dimensions == affine.unit.dimensions {
            delta.canonical
        } else if delta.unit.is_dimensionless() && !delta.unit.is_percent() {
            delta.canonical * affine.unit.scalar
        } else {
            return Err(AbacusError::IncompatibleDimensions);
        };
        Ok(Value {
            canonical: affine.canonical + sign * shift,
            unit: Arc::clone(&affine.unit),
        })
    }

    /// Scales the displayed amount of an affine value by a dimensionless factor.
    fn scale_affine(affine: &Value, factor: f64) -> Value {
        Value::new(affine.amount() * factor, Arc::clone(&affine.unit))
    }
}

// Add implementations
impl Add<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn add(self, rhs: &Value) -> Self::Output {
        match (self.unit.is_affine(), rhs.unit.is_affine()) {
            (true, true) => return Err(AbacusError::AffineUnitOperation("add")),
            (true, false) => return Value::shift_affine(self, rhs, 1.0),
            (false, true) => return Value::shift_affine(rhs, self, 1.0),
            (false, false) => {}
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * (1.0 + rhs.canonical),
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_percent() && !rhs.unit.is_percent() {
            return Ok(Value {
                canonical: rhs.canonical * (1.0 + self.canonical),
                unit: Arc::clone(&rhs.unit),
            });
        }

        let (lhs_can, rhs_can, unit) = self.resolve_additive_operands(rhs)?;
        Ok(Value {
            canonical: snap_cancellation(lhs_can + rhs_can, lhs_can, rhs_can),
            unit,
        })
    }
}

// Sub implementations
impl Sub<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn sub(self, rhs: &Value) -> Self::Output {
        match (self.unit.is_affine(), rhs.unit.is_affine()) {
            (true, true) => {
                if !self.unit.is_compatible_with(&rhs.unit) {
                    return Err(AbacusError::IncompatibleDimensions);
                }
                let unit = temperature_delta_unit(&self.unit);
                return Ok(Value {
                    canonical: snap_cancellation(
                        self.canonical - rhs.canonical,
                        self.canonical,
                        rhs.canonical,
                    ),
                    unit,
                });
            }
            (true, false) => return Value::shift_affine(self, rhs, -1.0),
            (false, true) => return Err(AbacusError::AffineUnitOperation("subtract")),
            (false, false) => {}
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical * (1.0 - rhs.canonical),
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_percent() && !rhs.unit.is_percent() {
            // `10% - 3` is plain arithmetic (0.1 - 3), mirroring `10% + 3` which yields a plain number.
            if !rhs.unit.is_dimensionless() {
                return Err(AbacusError::IncompatibleDimensions);
            }
            return Ok(Value {
                canonical: snap_cancellation(
                    self.canonical - rhs.canonical,
                    self.canonical,
                    rhs.canonical,
                ),
                unit: Arc::clone(&rhs.unit),
            });
        }

        let (lhs_can, rhs_can, unit) = self.resolve_additive_operands(rhs)?;
        Ok(Value {
            canonical: snap_cancellation(lhs_can - rhs_can, lhs_can, rhs_can),
            unit,
        })
    }
}

// Mul implementations
impl Mul<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn mul(self, rhs: &Value) -> Self::Output {
        match (self.unit.is_affine(), rhs.unit.is_affine()) {
            (true, false) if rhs.unit.is_dimensionless() => {
                return Ok(Value::scale_affine(self, rhs.canonical));
            }
            (false, true) if self.unit.is_dimensionless() => {
                return Ok(Value::scale_affine(rhs, self.canonical));
            }
            (false, false) => {}
            _ => return Err(AbacusError::AffineUnitOperation("multiply")),
        }

        match (self.unit.is_percent(), rhs.unit.is_percent()) {
            (true, true) => {
                return Ok(Value {
                    canonical: self.canonical * rhs.canonical,
                    unit: Arc::clone(&self.unit),
                });
            }
            (true, false) => {
                return Ok(Value {
                    canonical: self.canonical * rhs.canonical,
                    unit: Arc::clone(&rhs.unit),
                });
            }
            (false, true) => {
                return Ok(Value {
                    canonical: self.canonical * rhs.canonical,
                    unit: Arc::clone(&self.unit),
                });
            }
            (false, false) => {}
        }

        // Multiplying by a bare number keeps the other operand's unit without allocating.
        if rhs.unit.is_plain_number() {
            return Ok(Value {
                canonical: self.canonical * rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_plain_number() {
            return Ok(Value {
                canonical: self.canonical * rhs.canonical,
                unit: Arc::clone(&rhs.unit),
            });
        }

        let dimensions = self
            .unit
            .dimensions
            .checked_add(rhs.unit.dimensions)
            .ok_or(AbacusError::ExponentLimitExceeded)?;
        let unit = Unit {
            scalar: self.unit.scalar * rhs.unit.scalar,
            offset: 0.0,
            dimensions,
            display: self.unit.display.multiply(&rhs.unit.display),
        };

        Ok(Value {
            canonical: self.canonical * rhs.canonical,
            unit: Arc::new(unit),
        })
    }
}

// Div implementations
impl Div<&Value> for &Value {
    type Output = Result<Value, AbacusError>;

    fn div(self, rhs: &Value) -> Self::Output {
        match (self.unit.is_affine(), rhs.unit.is_affine()) {
            (true, false) if rhs.unit.is_dimensionless() => {
                return Ok(Value::scale_affine(self, 1.0 / rhs.canonical));
            }
            (false, false) => {}
            _ => return Err(AbacusError::AffineUnitOperation("divide")),
        }

        if rhs.unit.is_percent() && !self.unit.is_percent() {
            return Ok(Value {
                canonical: self.canonical / rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }
        if self.unit.is_percent() && !rhs.unit.is_percent() && rhs.unit.is_dimensionless() {
            return Ok(Value {
                canonical: self.canonical / rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }

        if rhs.unit.is_plain_number() {
            return Ok(Value {
                canonical: self.canonical / rhs.canonical,
                unit: Arc::clone(&self.unit),
            });
        }

        // A percentage divided by a dimensional quantity is a plain ratio per that unit
        // (`50% / 2 s` = 0.25 1/s), so drop the percent display.
        let no_display;
        let lhs_display = if self.unit.is_percent() {
            no_display = UnitExpr::dimensionless();
            &no_display
        } else {
            &self.unit.display
        };
        let lhs_scalar = if self.unit.is_percent() {
            1.0
        } else {
            self.unit.scalar
        };

        let dimensions = self
            .unit
            .dimensions
            .checked_sub(rhs.unit.dimensions)
            .ok_or(AbacusError::ExponentLimitExceeded)?;
        let unit = Unit {
            scalar: lhs_scalar / rhs.unit.scalar,
            offset: 0.0,
            dimensions,
            display: lhs_display.divide(&rhs.unit.display),
        };

        Ok(Value {
            canonical: self.canonical / rhs.canonical,
            unit: Arc::new(unit),
        })
    }
}

/// Generates the three ownership-coercing forwarding impls for a binary operator whose
/// canonical implementation is `&Value op &Value`.
macro_rules! impl_op_forwarding {
    ($trait:ident, $method:ident) => {
        impl $trait<Value> for Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: Self) -> Self::Output {
                <&Value as $trait<&Value>>::$method(&self, &rhs)
            }
        }
        impl $trait<&Value> for Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: &Value) -> Self::Output {
                <&Value as $trait<&Value>>::$method(&self, rhs)
            }
        }
        impl $trait<Value> for &Value {
            type Output = Result<Value, AbacusError>;
            fn $method(self, rhs: Value) -> Self::Output {
                <&Value as $trait<&Value>>::$method(self, &rhs)
            }
        }
    };
}

impl_op_forwarding!(Add, add);
impl_op_forwarding!(Sub, sub);
impl_op_forwarding!(Mul, mul);
impl_op_forwarding!(Div, div);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{dimensions::Dimensions, unit::UnitExpr};

    fn unit(scalar: f64, dimensions: Dimensions, display: &str) -> Arc<Unit> {
        Arc::new(Unit {
            scalar,
            dimensions,
            offset: 0f64,
            display: UnitExpr::single(display),
        })
    }

    /// The original (allocating, unscreened) noise-suppression rule, kept as a reference.
    fn denoised_reference(v: f64) -> f64 {
        let sci = format!("{v:e}");
        let mantissa = sci.split('e').next().unwrap_or("");
        if mantissa.bytes().filter(u8::is_ascii_digit).count() < 16 {
            return v;
        }
        for precision in 0..NOISE_MAX_DIGITS {
            let Ok(shorter) = format!("{v:.precision$e}").parse::<f64>() else {
                return v;
            };
            if shorter.is_finite()
                && shorter.signum() == v.signum()
                && ulp_distance(shorter, v) <= 2
            {
                return shorter;
            }
        }
        v
    }

    #[test]
    fn screened_denoise_matches_reference() {
        // xorshift64 for reproducible pseudo-random doubles across the display range.
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let (mut checked, mut snapped) = (0, 0);
        for i in 0..200_000_u64 {
            let r = next();
            let v = match i % 4 {
                // Arbitrary doubles in roughly [1e-7, 1e22].
                0 => {
                    #[allow(clippy::cast_precision_loss)]
                    let mantissa = (r >> 11) as f64 / (1u64 << 53) as f64 + 1.0;
                    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
                    let exp = (r % 97) as i32 - 24;
                    mantissa * 2f64.powi(exp)
                }
                // Short decimals with a few ulps of noise (the case denoising exists for).
                1 | 2 => {
                    #[allow(clippy::cast_precision_loss)]
                    let short = (r % 1_000_000_007) as f64 / 10f64.powi((r % 13) as i32);
                    let mut x = short * 3.0 / 3.0 + 0.0;
                    for _ in 0..(r >> 60) % 4 {
                        x = x.next_up();
                    }
                    if r & 1 == 1 { -x } else { x }
                }
                // Products/quotients of short decimals (unit conversions).
                _ => {
                    #[allow(clippy::cast_precision_loss)]
                    let a = (r % 100_000) as f64 / 1000.0;
                    #[allow(clippy::cast_precision_loss)]
                    let b = ((r >> 20) % 100_000 + 1) as f64 / 7.0;
                    a * b / 3.0
                }
            };
            if !v.is_finite() || v == 0.0 {
                continue;
            }
            let expected = denoised_reference(v);
            assert_eq!(
                denoised(v).to_bits(),
                expected.to_bits(),
                "denoise mismatch for {v:e}"
            );
            checked += 1;
            if expected.to_bits() != v.to_bits() {
                snapped += 1;
            }
        }
        assert!(checked > 150_000);
        // Both the pass-through and the snapping paths must be exercised.
        assert!(snapped > 10_000, "only {snapped} snapped values");
    }

    #[test]
    fn converts_display_values_to_canonical_values() {
        let distance = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let duration = Value::new(1.0, unit(3_600.0, Dimensions::TIME, "h"));

        assert_eq!(distance.canonical, 5_000.0);
        assert_eq!(duration.canonical, 3_600.0);
    }

    #[test]
    fn divides_values_and_units() {
        let distance = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let duration = Value::new(1.0, unit(3_600.0, Dimensions::TIME, "h"));
        let speed = (distance / duration).unwrap();

        assert!((speed.canonical - 5_000.0 / 3_600.0).abs() < f64::EPSILON);
        assert!((speed.unit.scalar - 1_000.0 / 3_600.0).abs() < f64::EPSILON);
        assert_eq!(speed.unit.dimensions, Dimensions::LENGTH - Dimensions::TIME);
        assert_eq!(speed.to_display(), "5 km/h");
    }

    #[test]
    fn adds_and_subtracts_only_compatible_values() {
        let sum = (Value::new(2.0, unit(1_000.0, Dimensions::LENGTH, "km"))
            + Value::new(500.0, unit(1.0, Dimensions::LENGTH, "m")))
        .unwrap();
        assert_eq!(sum.to_display(), "2.5 km");

        let difference = (sum - Value::new(1.0, unit(1_000.0, Dimensions::LENGTH, "km"))).unwrap();
        assert_eq!(difference.to_display(), "1.5 km");

        let incompatible = Value::new(1.0, unit(1.0, Dimensions::LENGTH, "m"))
            + Value::new(1.0, unit(1.0, Dimensions::TIME, "s"));
        assert!(incompatible.is_err());
    }

    #[test]
    fn multiplies_values_with_different_dimensions() {
        let area = (Value::new(2.0, unit(1.0, Dimensions::LENGTH, "m"))
            * Value::new(3.0, unit(1.0, Dimensions::LENGTH, "m")))
        .unwrap();

        assert_eq!(area.canonical, 6.0);
        assert_eq!(
            area.unit.dimensions,
            Dimensions::LENGTH + Dimensions::LENGTH
        );
        assert_eq!(area.to_display(), "6 m^2");
    }

    #[test]
    fn simplifies_compound_unit_displays() {
        let speed = (Value::new(5.0, unit(1.0, Dimensions::LENGTH, "m"))
            / Value::new(1.0, unit(1.0, Dimensions::TIME, "s")))
        .unwrap();
        assert_eq!(speed.to_display(), "5 m/s");

        let mut distance = (speed * Value::new(5.0, unit(1.0, Dimensions::TIME, "s"))).unwrap();
        let std_unit_reg = UnitRegistry::standard();
        distance.simplify_unit_display(&std_unit_reg);

        assert_eq!(distance.canonical, 25.0);
        assert_eq!(distance.unit.dimensions, Dimensions::LENGTH);
        assert_eq!(distance.to_display(), "25 m");
    }

    #[test]
    fn supports_reference_arithmetic() {
        let a = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let b = Value::new(500.0, unit(1.0, Dimensions::LENGTH, "m"));

        let sum = (&a + &b).unwrap();
        assert_eq!(sum.to_display(), "5.5 km");
        // Verify a and b are not consumed
        assert_eq!(a.to_display(), "5 km");
        assert_eq!(b.to_display(), "500 m");
    }

    #[test]
    fn supports_partial_eq_and_display() {
        let a = Value::new(5.0, unit(1_000.0, Dimensions::LENGTH, "km"));
        let b = Value::new(5000.0, unit(1.0, Dimensions::LENGTH, "m"));

        assert_eq!(a, b);
        assert_eq!(format!("{a}"), "5 km");
    }

    #[cfg(feature = "units")]
    #[test]
    fn automatically_converts_to_matching_derived_units() {
        let registry = UnitRegistry::standard();

        let mass = registry.value(2.0, "kg").unwrap();
        let accel = (registry.value(9.8, "m").unwrap()
            / (registry.value(1.0, "s").unwrap() * registry.value(1.0, "s").unwrap()).unwrap())
        .unwrap();

        let force = (&mass * &accel).unwrap();
        let force_derived = force.to_derived(&registry).unwrap();
        assert_eq!(force_derived.to_display(), "19.6 N");

        let distance = registry.value(5.0, "m").unwrap();
        let work = (&force_derived * &distance).unwrap();
        let work_derived = work.to_derived(&registry).unwrap();
        assert_eq!(work_derived.to_display(), "98 J");
    }

    #[cfg(feature = "units")]
    #[test]
    fn converts_dimensionless_values_to_target_units() {
        let registry = UnitRegistry::standard();
        let dimless = Value::new(10.0, Arc::new(Unit::dimensionless()));
        let converted = dimless.to(&registry, "m").unwrap();
        assert_eq!(converted.to_display(), "10 m");
    }
}
