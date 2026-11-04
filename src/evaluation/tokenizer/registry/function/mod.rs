#[cfg(feature = "stats")]
pub mod ci;
pub mod combinatorics;
#[cfg(feature = "date")]
pub mod date;
// Always present: `special` provides `factorial` for `!` and `nCr`.
pub mod distributions;
#[cfg(feature = "financial")]
pub mod financial;
#[cfg(feature = "stats")]
pub mod hypothesis;
pub mod math;
pub mod math_helpers;
pub mod operators;
#[cfg(feature = "stats")]
pub mod regression;
#[cfg(feature = "stats")]
pub mod stats;
pub mod trig;

pub use math_helpers::check_dimensionless;
