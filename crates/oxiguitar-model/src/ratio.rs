// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Exact fractions for musical time.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Mul, Sub};

/// An exact fraction, always stored in lowest terms with a positive denominator.
///
/// Musical time in OxiGuitar is measured in whole notes: a quarter note lasts
/// `1/4`, an eighth-note triplet `1/12`. Because every duration is exact,
/// tuplets, nested tuplets and grace notes never drift through rounding.
///
/// The operators (`+`, `-`, `*`) panic if the result does not fit, which cannot
/// happen with durations built from real note values. Code that combines
/// numbers taken straight from a file uses the `checked_*` methods instead.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    num: i64,
    den: i64,
}

const fn gcd(mut a: i128, mut b: i128) -> i128 {
    if a < 0 {
        a = -a;
    }
    if b < 0 {
        b = -b;
    }
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Ratio {
    /// Zero.
    pub const ZERO: Ratio = Ratio { num: 0, den: 1 };
    /// One whole note.
    pub const ONE: Ratio = Ratio { num: 1, den: 1 };

    /// Builds `num / den` in lowest terms. Returns `None` if `den` is zero.
    pub const fn new(num: i64, den: i64) -> Option<Ratio> {
        Self::from_i128(num as i128, den as i128)
    }

    /// A whole number.
    pub const fn from_integer(n: i64) -> Ratio {
        Ratio { num: n, den: 1 }
    }

    const fn from_i128(num: i128, den: i128) -> Option<Ratio> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den);
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        if n < i64::MIN as i128 || n > i64::MAX as i128 || d > i64::MAX as i128 {
            return None;
        }
        Some(Ratio {
            num: n as i64,
            den: d as i64,
        })
    }

    /// The numerator, in lowest terms.
    pub const fn numer(self) -> i64 {
        self.num
    }

    /// The denominator, in lowest terms; always positive.
    pub const fn denom(self) -> i64 {
        self.den
    }

    /// True if this is zero.
    pub const fn is_zero(self) -> bool {
        self.num == 0
    }

    /// `self + rhs`, or `None` on overflow.
    pub fn checked_add(self, rhs: Ratio) -> Option<Ratio> {
        let (a, b, c, d) = (
            self.num as i128,
            self.den as i128,
            rhs.num as i128,
            rhs.den as i128,
        );
        Self::from_i128(
            a.checked_mul(d)?.checked_add(c.checked_mul(b)?)?,
            b.checked_mul(d)?,
        )
    }

    /// `self - rhs`, or `None` on overflow.
    pub fn checked_sub(self, rhs: Ratio) -> Option<Ratio> {
        self.checked_add(Ratio {
            num: rhs.num.checked_neg()?,
            den: rhs.den,
        })
    }

    /// `self * rhs`, or `None` on overflow.
    pub fn checked_mul(self, rhs: Ratio) -> Option<Ratio> {
        let n = (self.num as i128).checked_mul(rhs.num as i128)?;
        let d = (self.den as i128).checked_mul(rhs.den as i128)?;
        Self::from_i128(n, d)
    }

    /// `self / rhs`, or `None` if `rhs` is zero or the result overflows.
    pub fn checked_div(self, rhs: Ratio) -> Option<Ratio> {
        let n = (self.num as i128).checked_mul(rhs.den as i128)?;
        let d = (self.den as i128).checked_mul(rhs.num as i128)?;
        Self::from_i128(n, d)
    }

    /// An approximation as a floating-point number, for display and audio only.
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

impl Default for Ratio {
    fn default() -> Self {
        Ratio::ZERO
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.num as i128 * other.den as i128).cmp(&(other.num as i128 * self.den as i128))
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for Ratio {
    type Output = Ratio;
    fn add(self, rhs: Ratio) -> Ratio {
        self.checked_add(rhs).expect("Ratio addition overflowed")
    }
}

impl Sub for Ratio {
    type Output = Ratio;
    fn sub(self, rhs: Ratio) -> Ratio {
        self.checked_sub(rhs).expect("Ratio subtraction overflowed")
    }
}

impl Mul for Ratio {
    type Output = Ratio;
    fn mul(self, rhs: Ratio) -> Ratio {
        self.checked_mul(rhs)
            .expect("Ratio multiplication overflowed")
    }
}

impl std::iter::Sum for Ratio {
    fn sum<I: Iterator<Item = Ratio>>(iter: I) -> Ratio {
        iter.fold(Ratio::ZERO, |a, b| a + b)
    }
}

impl fmt::Debug for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(n: i64, d: i64) -> Ratio {
        Ratio::new(n, d).unwrap()
    }

    #[test]
    fn lowest_terms_and_sign() {
        assert_eq!(r(2, 8), r(1, 4));
        assert_eq!(r(3, -6).numer(), -1);
        assert_eq!(r(3, -6).denom(), 2);
        assert_eq!(Ratio::new(1, 0), None);
    }

    #[test]
    fn triplets_sum_exactly() {
        let third_of_quarter = r(1, 12);
        let sum: Ratio = std::iter::repeat_n(third_of_quarter, 3).sum();
        assert_eq!(sum, r(1, 4));
    }

    #[test]
    fn ordering() {
        assert!(r(1, 3) > r(1, 4));
        assert!(r(-1, 2) < Ratio::ZERO);
    }

    #[test]
    fn overflow_is_reported() {
        let huge = Ratio::from_integer(i64::MAX);
        assert_eq!(huge.checked_add(huge), None);
        assert_eq!(huge.checked_mul(huge), None);
        assert_eq!(r(1, 2).checked_div(Ratio::ZERO), None);
    }

    #[test]
    fn display() {
        assert_eq!(r(3, 4).to_string(), "3/4");
        assert_eq!(Ratio::from_integer(2).to_string(), "2");
    }
}
