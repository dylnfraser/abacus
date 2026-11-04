use crate::{AngleMode, IntervalStyle, TimeZone, WeekendDays};

/// Configuration settings controlling evaluation semantics.
#[derive(Debug, Clone)]
pub struct EvalConfig {
    pub auto_derived: bool,
    pub angle_mode: AngleMode,
    pub strict_dimensions: bool,
    pub default_interval_style: Option<IntervalStyle>,
    pub default_timezone: Option<TimeZone>,
    pub weekend: WeekendDays,
    pub max_recursion_depth: usize,
    pub max_exponent: f64,
    pub implicit_multiplication: bool,
    pub number_scales: bool,
    pub anchor_date: Option<crate::Date>,
    /// Display format for dates; `MMDDYYYY` also makes ambiguous `NN-NN-YYYY` input month-first.
    pub date_format: crate::DateFormat,
    /// Snap `+`/`-` results that are pure floating-point cancellation noise to 0
    /// (`0.1 + 0.2 - 0.3` = 0).
    pub snap_cancellation: bool,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            auto_derived: true,
            angle_mode: AngleMode::Radians,
            strict_dimensions: false,
            default_interval_style: None,
            default_timezone: None,
            weekend: WeekendDays::SaturdaySunday,
            max_recursion_depth: crate::evaluation::parser::MAX_RECURSION_DEPTH,
            max_exponent: 1_000.0,
            implicit_multiplication: true,
            number_scales: cfg!(feature = "number-scales"),
            anchor_date: None,
            date_format: crate::DateFormat::default(),
            snap_cancellation: true,
        }
    }
}

impl From<&crate::Abacus> for EvalConfig {
    fn from(abacus: &crate::Abacus) -> Self {
        Self {
            auto_derived: abacus.auto_derived_units(),
            angle_mode: abacus.angle_mode(),
            strict_dimensions: abacus.strict_dimensions(),
            default_interval_style: abacus.interval_style(),
            default_timezone: abacus.default_timezone().cloned(),
            weekend: abacus.weekend(),
            max_recursion_depth: abacus.max_recursion_depth(),
            max_exponent: abacus.max_exponent(),
            implicit_multiplication: abacus.implicit_multiplication(),
            number_scales: abacus.number_scales(),
            anchor_date: abacus.anchor_date().cloned(),
            date_format: abacus.date_format(),
            snap_cancellation: abacus.snap_cancellation(),
        }
    }
}
