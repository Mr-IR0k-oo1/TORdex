use std::ops::{Add, Mul};

use serde::{Deserialize, Serialize};

/// A typed confidence score in the range [0.0, 1.0].
///
/// Confidence is **immutable** — every operation produces a new `Confidence`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    /// Absolute certainty.
    pub const CERTAIN: Confidence = Confidence(1.0);
    /// Absolute uncertainty.
    pub const NONE: Confidence = Confidence(0.0);

    /// Create a new `Confidence`, clamping the raw value to [0.0, 1.0].
    #[must_use]
    pub fn new(raw: f64) -> Self {
        Self(raw.clamp(0.0, 1.0))
    }

    /// Return the inner `f64`.
    #[must_use]
    pub fn raw(self) -> f64 {
        self.0
    }

    /// Combine two confidence scores using noisy-AND: `a * b`.
    ///
    /// Both pieces of evidence must agree, so the combined confidence is
    /// the product (multiplicative conjunction).
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self::new(self.0 * other.0)
    }

    /// Combine two confidence scores using noisy-OR: `a + b - a * b`.
    ///
    /// Either piece of evidence suffices, so the combined confidence is
    /// higher than either alone (disjunction).
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self::new(self.0 + other.0 - self.0 * other.0)
    }

    /// Weighted average with another confidence.
    ///
    /// `self` has weight `w`, `other` has weight `1.0 - w`.
    #[must_use]
    pub fn weighted(self, other: Self, weight: f64) -> Self {
        let w = weight.clamp(0.0, 1.0);
        Self::new(self.0 * w + other.0 * (1.0 - w))
    }

    /// Decay confidence by a factor, e.g. based on age.
    #[must_use]
    pub fn decay(self, factor: f64) -> Self {
        Self::new(self.0 * factor.clamp(0.0, 1.0))
    }
}

impl Add for Confidence {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        self.or(other)
    }
}

impl Mul for Confidence {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        self.and(other)
    }
}

impl From<f64> for Confidence {
    fn from(raw: f64) -> Self {
        Self::new(raw)
    }
}

impl From<Confidence> for f64 {
    fn from(c: Confidence) -> Self {
        c.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_range() {
        assert_eq!(Confidence::new(-0.5), Confidence::NONE);
        assert_eq!(Confidence::new(1.5), Confidence::CERTAIN);
        assert_eq!(Confidence::new(0.5).raw(), 0.5);
    }

    #[test]
    fn and_combines_multiplicatively() {
        let a = Confidence::new(0.8);
        let b = Confidence::new(0.7);
        assert!((a.and(b).raw() - 0.56).abs() < 1e-10);
    }

    #[test]
    fn or_combines_additively() {
        let a = Confidence::new(0.5);
        let b = Confidence::new(0.5);
        assert!((a.or(b).raw() - 0.75).abs() < 1e-10);
    }

    #[test]
    fn weighted_average() {
        let a = Confidence::new(1.0);
        let b = Confidence::new(0.0);
        assert!((a.weighted(b, 0.75).raw() - 0.75).abs() < 1e-10);
    }

    #[test]
    fn decay_reduces() {
        let c = Confidence::new(0.9);
        assert!((c.decay(0.5).raw() - 0.45).abs() < 1e-10);
    }

    #[test]
    fn operator_overloads() {
        let a = Confidence::new(0.8);
        let b = Confidence::new(0.5);
        assert!((a * b).raw() - 0.4 < 1e-10);
        assert!((a + b).raw() - (0.8 + 0.5 - 0.4) < 1e-10);
    }
}
