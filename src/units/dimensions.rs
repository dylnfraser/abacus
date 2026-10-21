use std::ops::{Add, Mul, Sub};

pub const DIMENSION_COUNT: usize = 9;

/// Fixed-point denominator for dimension exponents.
///
/// `27720 = lcm(1..=12)`, so every rational exponent whose denominator is at most 12
/// (halves, thirds, quarters, sevenths, ...) is represented exactly and round-trips
/// (e.g. `((2 m)^(1/7))^7` is exactly `m`).
pub const SCALE: i32 = 27_720;

/// Largest exponent magnitude representable in a single dimension slot (≈ 77 471).
pub const MAX_REPRESENTABLE_EXPONENT: f64 = (i32::MAX / SCALE) as f64;

/// Dimensions represented as fixed-point integers (scaled by [`SCALE`]) across 9 base dimensions:
/// [length, mass, time, current, temperature, amount, luminous intensity, information, currency].
///
/// The representation is `[i32; 9]` (36 bytes). The `+`, `-` and `* f64` operators saturate
/// on overflow; use [`Dimensions::checked_add`], [`Dimensions::checked_sub`] and
/// [`Dimensions::checked_mul_f64`] wherever the operands are derived from user input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dimensions(pub [i32; DIMENSION_COUNT]);

const fn round_to_fixed(x: f64) -> i32 {
    let scaled = x * SCALE as f64;
    // `f64::round` is not const; emulate round-half-away-from-zero.
    if scaled >= 0.0 {
        (scaled + 0.5) as i32
    } else {
        (scaled - 0.5) as i32
    }
}

impl Dimensions {
    /// Create a `Dimensions` instance from the 8 physical base dimensions (length, mass, time,
    /// current, temperature, amount, luminous intensity, information). Currency is set to 0.
    #[must_use]
    pub const fn from_f64(arr: [f64; 8]) -> Self {
        let mut out = [0i32; DIMENSION_COUNT];
        let mut i = 0;
        while i < 8 {
            out[i] = round_to_fixed(arr[i]);
            i += 1;
        }
        Self(out)
    }

    /// Create a `Dimensions` instance from all 9 dimensions including currency.
    #[must_use]
    pub const fn from_f64_9(arr: [f64; DIMENSION_COUNT]) -> Self {
        let mut out = [0i32; DIMENSION_COUNT];
        let mut i = 0;
        while i < DIMENSION_COUNT {
            out[i] = round_to_fixed(arr[i]);
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
        std::array::from_fn(|i| f64::from(self.0[i]) / f64::from(SCALE))
    }

    /// Adds exponents (multiplying quantities), returning `None` on overflow.
    #[must_use]
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        let mut out = [0i32; DIMENSION_COUNT];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.0[i].checked_add(rhs.0[i])?;
        }
        Some(Self(out))
    }

    /// Subtracts exponents (dividing quantities), returning `None` on overflow.
    #[must_use]
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        let mut out = [0i32; DIMENSION_COUNT];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.0[i].checked_sub(rhs.0[i])?;
        }
        Some(Self(out))
    }

    /// Scales every exponent by `rhs` (raising a quantity to a power), rounding to the
    /// nearest representable fixed-point value. Returns `None` if `rhs` is not finite
    /// or a scaled exponent does not fit.
    #[must_use]
    pub fn checked_mul_f64(self, rhs: f64) -> Option<Self> {
        if !rhs.is_finite() {
            return None;
        }
        let mut out = [0i32; DIMENSION_COUNT];
        for (i, slot) in out.iter_mut().enumerate() {
            let scaled = (f64::from(self.0[i]) * rhs).round();
            if scaled > f64::from(i32::MAX) || scaled < f64::from(i32::MIN) {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = scaled as i32;
            }
        }
        Some(Self(out))
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

/// Saturating exponent addition. Prefer [`Dimensions::checked_add`] for user-derived input.
impl Add for Dimensions {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(std::array::from_fn(|i| self.0[i].saturating_add(rhs.0[i])))
    }
}

/// Saturating exponent subtraction. Prefer [`Dimensions::checked_sub`] for user-derived input.
impl Sub for Dimensions {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(std::array::from_fn(|i| self.0[i].saturating_sub(rhs.0[i])))
    }
}

/// Saturating exponent scaling. Prefer [`Dimensions::checked_mul_f64`] for user-derived input.
impl Mul<f64> for Dimensions {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        #[allow(clippy::cast_possible_truncation)]
        Self(std::array::from_fn(|i| {
            (f64::from(self.0[i]) * rhs).round() as i32
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_ops_detect_overflow() {
        let big = Dimensions::LENGTH * 70_000.0;
        assert!(big.checked_add(big).is_none());
        assert!(Dimensions::LENGTH.checked_mul_f64(1e6).is_none());
        assert!(Dimensions::LENGTH.checked_mul_f64(f64::NAN).is_none());
    }

    #[test]
    fn seventh_root_round_trips() {
        let d = Dimensions::LENGTH
            .checked_mul_f64(1.0 / 7.0)
            .and_then(|d| d.checked_mul_f64(7.0));
        assert_eq!(d, Some(Dimensions::LENGTH));
    }
}
