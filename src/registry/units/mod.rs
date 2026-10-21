pub mod angle_units;
pub mod astronomical_units;
pub mod cgs_physics_units;
pub mod computing_niche_units;
#[cfg(feature = "currencies")]
pub mod currency_units;
pub mod derived_units;
pub mod engineering_units;
pub mod humorous_units;
pub mod imperial_units;
pub mod metric_units;
pub mod nautical_units;
#[cfg(feature = "number-scales")]
pub mod number_scale_units;
pub mod speed_units;
pub mod storage_units;
pub mod temporal_units;
pub mod trade_historical_units;
pub mod typography_units;
pub mod volume_units;

/// Unit keys that contain a space (or a trailing symbol, like `R$`) and therefore cannot be
/// produced by the ordinary identifier scanner. The lexer matches these (ASCII
/// case-insensitively, longest first) before scanning identifiers, and only emits them when
/// the registry actually contains the key (so feature-gated entries are inert).
pub const MULTI_WORD_UNITS: &[&str] = &[
    "imperial fluid ounces",
    "imperial fluid ounce",
    "imperial gallons",
    "imperial gallon",
    "imperial quarts",
    "imperial quart",
    "imperial pints",
    "imperial pint",
    "imperial gills",
    "imperial gill",
    "imperial tons",
    "imperial ton",
    "pounds sterling",
    "pound sterling",
    "business days",
    "business day",
    "working days",
    "working day",
    "fluid ounces",
    "fluid ounce",
    "metric tons",
    "metric ton",
    "short tons",
    "short ton",
    "us gallons",
    "us gallon",
    "long tons",
    "long ton",
    "work days",
    "work day",
    "fl oz",
    "R$",
];
