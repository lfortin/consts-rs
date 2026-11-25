// Copyright (C) 2026 lfortin
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program in the file LICENSE. If not, see
// <https://www.gnu.org/licenses/>.

//! Significant digits of the golden ratio, φ = (1 + √5) / 2.
//!
//! [`GoldenAlgo::Newton`] and [`GoldenAlgo::Isqrt`] both evaluate
//! (10^scale + √5 × 10^scale) / 2. `Newton` finds √5 with Newton-Raphson.
//! `Isqrt` uses integer square root. [`GoldenAlgo::Cf`] uses the continued
//! fraction φ = [1; 1, 1, …], whose convergents are consecutive Fibonacci ratios.

use num_bigint::BigUint;
use num_traits::{Euclid, ToPrimitive, Zero};

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000_000;

const GUARD_DIGITS: u32 = 20;

/// `10^GUARD_DIGITS`. Small enough to live in a `u128`, so rounding does not
/// rebuild it with `BigUint::pow` on every call.
const GUARD_SCALE: u128 = 10_u128.pow(GUARD_DIGITS);

/// Algorithm used to compute significant digits of the golden ratio.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum GoldenAlgo {
    /// (1 + √5) / 2, with √5 from Newton-Raphson.
    Newton,
    /// (1 + √5) / 2, with √5 from integer square root.
    Isqrt,
    /// Continued fraction φ = [1; 1, 1, …], the Fibonacci ratios.
    Cf,
}

#[allow(clippy::derivable_impls)]
impl Default for GoldenAlgo {
    fn default() -> Self {
        Self::Newton
    }
}

impl std::fmt::Display for GoldenAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            GoldenAlgo::Newton => "newton",
            GoldenAlgo::Isqrt => "isqrt",
            GoldenAlgo::Cf => "cf",
        })
    }
}

/// Format `digits` significant digits of the golden ratio.
///
/// One digit is `2`, because 1.618… rounds half up. Larger counts include a
/// decimal point after the leading `1`. The last digit is rounded half up.
///
/// Uses [`GoldenAlgo::default`].
pub fn format_golden(digits: u32) -> Result<String, &'static str> {
    format_golden_with_algo(digits, GoldenAlgo::default())
}

/// Format `digits` significant digits of the golden ratio with `algo`.
///
/// Output matches [`format_golden`]: one digit is `2`, because 1.618… rounds
/// half up. Larger counts include a decimal point after the leading `1`. The
/// last digit is rounded half up.
pub fn format_golden_with_algo(digits: u32, algo: GoldenAlgo) -> Result<String, &'static str> {
    let scale = scale_for_digits(digits)?;
    let one = BigUint::from(10u32).pow(scale);
    let phi_scaled = match algo {
        GoldenAlgo::Newton => phi_from_sqrt5(&one, sqrt5_newton(&one)),
        GoldenAlgo::Isqrt => phi_from_sqrt5(&one, sqrt5_isqrt(&one)),
        GoldenAlgo::Cf => golden_continued_fraction(&one),
    };
    format_scaled_golden(phi_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 1000000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_golden(phi_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(GUARD_SCALE);
    let half = BigUint::from(GUARD_SCALE / 2);
    let (mut kept, remainder) = phi_scaled.div_rem_euclid(&guard);
    if remainder >= half {
        kept += 1u32;
    }

    let mut text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: golden ratio rounding produced an unexpected length");
    }

    if digits != 1 {
        text.insert(1, '.');
    }
    Ok(text)
}

fn phi_from_sqrt5(one: &BigUint, sqrt5_scaled: BigUint) -> BigUint {
    // φ * 10^scale = (10^scale + √5 * 10^scale) / 2
    (one + sqrt5_scaled) >> 1
}

/// `floor(√5 * 10^scale)` by Newton-Raphson.
fn sqrt5_newton(one: &BigUint) -> BigUint {
    let radicand = BigUint::from(5u32) * one * one;
    isqrt_newton(&radicand)
}

/// `floor(√5 * 10^scale)` by integer square root.
fn sqrt5_isqrt(one: &BigUint) -> BigUint {
    let radicand = BigUint::from(5u32) * one * one;
    isqrt_newton(&radicand)
}

/// Floor of the square root of `n`.
///
/// Values that fit in `u128` use [`isqrt_u128`]. Larger values take the square
/// root of the high half, shift it back, and apply one Heron correction:
/// `q = floor((n - x^2) / (2x))`, then `x + q`, with a final decrement when
/// that lands one above the root.
fn isqrt_newton(n: &BigUint) -> BigUint {
    let Some(small) = n.to_u128() else {
        return isqrt_wide(n);
    };
    BigUint::from(isqrt_u128(small))
}

fn isqrt_wide(n: &BigUint) -> BigUint {
    let bits = n.bits();
    let m = (bits / 4).max(1);
    let s = isqrt_newton(&(n >> (2 * m)));
    let mut x = s << m;
    if x.is_zero() {
        x = BigUint::from(1u32);
    }
    let square = &x * &x;
    if &square <= n {
        let q = (n - &square) / (&x << 1);
        x += q;
    }
    if &x * &x > *n {
        x -= 1u32;
    } else {
        let next = &x + 1u32;
        if &next * &next <= *n {
            x = next;
        }
    }
    x
}

fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let bits = 128 - n.leading_zeros();
    let mut x = 1u128 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) >> 1;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// `floor(φ * 10^scale)` from continued-fraction convergents.
///
/// φ = [1; 1, 1, …], so the convergents are consecutive Fibonacci ratios
/// `F_{n+1}/F_n`. `F_n` comes from fast doubling. The error of `p/q` is less
/// than `1/(q q_next)`, and `q_next > q`, so a denominator whose bit length
/// reaches `bit_limit` is already closer than half a unit at this scale.
fn golden_continued_fraction(one: &BigUint) -> BigUint {
    // half_unit is 2 * 10^scale. Once q.bits() reaches half_unit.bits()/2 + 2,
    // 2^(2*(q.bits()-1)) exceeds half_unit and the error is under half a unit.
    let bit_limit = (one * 2u32).bits() / 2 + 2;
    // F_n ≈ φ^n / √5, and 1/log2(φ) < 14405/10000.
    let n = (bit_limit + 2) * 14_405 / 10_000 + 1;
    let (mut q, mut p) = fib_pair(n.max(2));
    while q.bits() < bit_limit {
        let p_next = &p + &q;
        q = std::mem::replace(&mut p, p_next);
    }
    (p * one) / q
}

/// `(F(n), F(n+1))` by fast doubling.
fn fib_pair(n: u64) -> (BigUint, BigUint) {
    if n == 0 {
        return (BigUint::from(0u32), BigUint::from(1u32));
    }
    let (a, b) = fib_pair(n / 2);
    // F(2k) = F(k) (2 F(k+1) - F(k)), F(2k+1) = F(k+1)^2 + F(k)^2.
    let c = &a * ((&b << 1) - &a);
    let d = &a * &a + &b * &b;
    if n.is_multiple_of(2) {
        (c, d)
    } else {
        let sum = &c + &d;
        (d, sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_golden(1).unwrap(), "2");
        assert_eq!(format_golden(2).unwrap(), "1.6");
        assert_eq!(format_golden(5).unwrap(), "1.6180");
        assert_eq!(format_golden(10).unwrap(), "1.618033989");
        assert_eq!(format_golden(20).unwrap(), "1.6180339887498948482");
        assert_eq!(
            format_golden(50).unwrap(),
            "1.6180339887498948482045868343656381177203091798058"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_golden(0).is_err());
        assert!(format_golden(MAX_DIGITS + 1).is_err());
        for algo in [GoldenAlgo::Newton, GoldenAlgo::Isqrt, GoldenAlgo::Cf] {
            assert!(format_golden_with_algo(0, algo).is_err());
            assert!(format_golden_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn newton_isqrt_matches_builtin() {
        for value in [0u32, 1, 2, 3, 10, 255, 256, 1_000] {
            let n = BigUint::from(value);
            assert_eq!(isqrt_newton(&n), n.sqrt());
        }
        assert_eq!(isqrt_u128(0), 0);
        assert_eq!(isqrt_u128(1), 1);
        assert_eq!(isqrt_u128(u128::MAX), (1u128 << 64) - 1);

        let mut wide = BigUint::from(5u32);
        for scale in [1u32, 20, 40, 80] {
            wide *= BigUint::from(10u32).pow(scale);
            assert_eq!(isqrt_newton(&wide), wide.sqrt(), "scale {scale}");
        }

        let root = (BigUint::from(1u32) << 2_000) + 3u32;
        let square = &root * &root;
        assert_eq!(isqrt_newton(&square), root);
        assert_eq!(isqrt_newton(&(&square - 1u32)), &root - 1u32);
        let wide_bits = (BigUint::from(1u32) << 4_096) - 1u32;
        assert_eq!(isqrt_newton(&wide_bits), wide_bits.sqrt());
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(GoldenAlgo::default(), GoldenAlgo::Newton);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_golden(digits).unwrap();
            for algo in [GoldenAlgo::Newton, GoldenAlgo::Isqrt, GoldenAlgo::Cf] {
                assert_eq!(
                    format_golden_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
