use std::ops::{Add, Mul, Sub};

pub const DIMENSION_COUNT: usize = 9;
pub const SCALE: i16 = 120;

/// Dimensions represented as fixed-point integers (scaled by 120) across 9 base dimensions:
/// [length, mass, time, current, temperature, amount, luminous intensity, information, currency].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dimensions(pub [i16; DIMENSION_COUNT]);

impl Dimensions {
    /// Create a `Dimensions` instance from 8 physical base dimensions (length, mass, time, current, temperature, amount, luminous intensity, information).
    /// Currency dimension is set to 0.
    #[must_use]
    pub const fn from_f64(arr: [f64; 8]) -> Self {
        let mut out = [0i16; DIMENSION_COUNT];
        let mut i = 0;
        while i < 8 {
            out[i] = (arr[i] * SCALE as f64) as i16;
            i += 1;
        }
        Self(out)
    }

    /// Create a `Dimensions` instance from all 9 dimensions including currency.
    #[must_use]
    pub const fn from_f64_9(arr: [f64; DIMENSION_COUNT]) -> Self {
        let mut out = [0i16; DIMENSION_COUNT];
        let mut i = 0;
        while i < DIMENSION_COUNT {
            out[i] = (arr[i] * SCALE as f64) as i16;
            i += 1;
        }
        Self(out)
    }

    /// Dimensionless unit (all dimensions are 0)
    pub const DIMENSIONLESS: Self = Self([0; DIMENSION_COUNT]);

    pub const LENGTH: Self = Self::from_f64([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    pub const MASS: Self = Self::from_f64([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    pub const TIME: Self = Self::from_f64([0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    pub const CURRENT: Self = Self::from_f64([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    pub const TEMPERATURE: Self = Self::from_f64([0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    pub const AMOUNT: Self = Self::from_f64([0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    pub const LUMINOUS_INTENSITY: Self = Self::from_f64([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    pub const INFORMATION: Self = Self::from_f64([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    pub const CURRENCY: Self = Self::from_f64_9([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);

    pub const AREA: Self = Self::from_f64([2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    pub const VOLUME: Self = Self::from_f64([3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

    #[must_use]
    pub fn is_dimensionless(self) -> bool {
        self == Self::DIMENSIONLESS
    }

    #[must_use]
    pub fn to_f64(self) -> [f64; DIMENSION_COUNT] {
        std::array::from_fn(|i| self.0[i] as f64 / SCALE as f64)
    }
}

impl From<[f64; 8]> for Dimensions {
    fn from(arr: [f64; 8]) -> Self {
        Self::from_f64(arr)
    }
}

impl From<[f64; DIMENSION_COUNT]> for Dimensions {
    fn from(arr: [f64; DIMENSION_COUNT]) -> Self {
        Self::from_f64_9(arr)
    }
}

impl Add for Dimensions {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(std::array::from_fn(|i| self.0[i] + rhs.0[i]))
    }
}

impl Sub for Dimensions {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(std::array::from_fn(|i| self.0[i] - rhs.0[i]))
    }
}

impl Mul<f64> for Dimensions {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self(std::array::from_fn(|i| {
            (self.0[i] as f64 * rhs).round() as i16
        }))
    }
}
