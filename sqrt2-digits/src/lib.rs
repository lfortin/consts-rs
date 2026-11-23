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

//! Significant digits of the square root of 2.
//!
//! [`Sqrt2Algo::Newton`] iterates Heron's method. [`Sqrt2Algo::Cf`] uses the
//! continued fraction √2 = [1; 2, 2, 2, …].

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000;

const GUARD_DIGITS: u32 = 20;

/// `10^GUARD_DIGITS`. Small enough to live in a `u128`, so rounding does not
/// rebuild it with `BigUint::pow` on every call.
const GUARD_SCALE: u128 = 10_u128.pow(GUARD_DIGITS);

/// Algorithm used to compute significant digits of √2.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Sqrt2Algo {
    /// Newton-Raphson (Heron) iteration for √2.
    Newton,
    /// Continued-fraction convergents of √2 = [1; 2, 2, 2, …].
    Cf,
}

#[allow(clippy::derivable_impls)]
impl Default for Sqrt2Algo {
    fn default() -> Self {
        Self::Newton
    }
}

impl std::fmt::Display for Sqrt2Algo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Sqrt2Algo::Newton => "newton",
            Sqrt2Algo::Cf => "cf",
        })
    }
}

/// Format `digits` significant digits of √2.
///
/// One digit is `1`. Larger counts include a decimal point after the leading
/// `1`. The last digit is rounded half up.
///
/// Uses [`Sqrt2Algo::default`].
pub fn format_sqrt2(digits: u32) -> Result<String, &'static str> {
    format_sqrt2_with_algo(digits, Sqrt2Algo::default())
}

/// Format `digits` significant digits of √2 with `algo`.
///
/// Output matches [`format_sqrt2`]: one digit is `1`. Larger counts include a
/// decimal point after the leading `1`. The last digit is rounded half up.
pub fn format_sqrt2_with_algo(digits: u32, algo: Sqrt2Algo) -> Result<String, &'static str> {
    let scale = scale_for_digits(digits)?;
    let one = BigUint::from(10u32).pow(scale);
    let sqrt2_scaled = match algo {
        Sqrt2Algo::Newton => sqrt2_newton(&one),
        Sqrt2Algo::Cf => sqrt2_cf(&one, scale),
    };
    format_scaled_sqrt2(sqrt2_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 1000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_sqrt2(sqrt2_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(GUARD_SCALE);
    let half = BigUint::from(GUARD_SCALE / 2);
    let (mut kept, remainder) = sqrt2_scaled.div_rem(&guard);
    if remainder >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: square root of 2 rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

/// `floor(√2 * 10^scale)` by Newton-Raphson.
fn sqrt2_newton(one: &BigUint) -> BigUint {
    // floor(√2 * 10^scale) = isqrt(2 * 10^(2*scale))
    let radicand = BigUint::from(2u32) * one * one;
    isqrt_newton(&radicand)
}

/// Floor of the square root of `n`, by `x_{k+1} = (x_k + n / x_k) / 2`.
fn isqrt_newton(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = isqrt_guess(n);
    loop {
        let y = (&x + n / &x) >> 1;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// Upper bound on `floor(sqrt(n))` from the top one or two `u64` limbs.
///
/// `n < (top + 1) * 2^shift` with `shift` a multiple of 64, so
/// `sqrt(n) < (floor(sqrt(top)) + 1) * 2^(shift/2)`.
fn isqrt_guess(n: &BigUint) -> BigUint {
    let bits = n.bits();
    let coarse = BigUint::from(1u32) << bits.div_ceil(2);
    if bits <= 64 {
        return coarse;
    }
    let digits = n.to_u64_digits();
    let Some(last) = digits.len().checked_sub(1) else {
        return coarse;
    };
    let mut top = u128::from(digits[last]);
    let mut shift = 64 * (last as u64);
    if last >= 1 {
        top = (top << 64) | u128::from(digits[last - 1]);
        shift -= 64;
    }
    let root = isqrt_u128(top) + 1;
    BigUint::from(root) << (shift / 2)
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

/// `floor(√2 * 10^scale)` from continued-fraction convergents.
///
/// √2 = [1; 2, 2, 2, …], so for k ≥ 1 the convergents obey
/// `p_k = 2 p_{k-1} + p_{k-2}` and `q_k = 2 q_{k-1} + q_{k-2}`.
fn sqrt2_cf(one: &BigUint, scale: u32) -> BigUint {
    let mut p_prev = BigUint::from(1u32);
    let mut p = BigUint::from(1u32);
    let mut q_prev = BigUint::from(0u32);
    let mut q = BigUint::from(1u32);
    let needed_bits = convergent_bits(scale);
    while q.bits() < needed_bits {
        let p_next = (&p << 1) + p_prev;
        let q_next = (&q << 1) + q_prev;
        p_prev = p;
        q_prev = q;
        p = p_next;
        q = q_next;
    }
    (&p * one) / q
}

/// Bit length of a convergent denominator past which the error is far below
/// one unit at `scale`.
fn convergent_bits(scale: u32) -> u64 {
    // q > 10^((scale + 8) / 2). log2(10) / 2 < 3322/2000.
    (u64::from(scale) + 8) * 3322 / 2000 + 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_sqrt2(1).unwrap(), "1");
        assert_eq!(format_sqrt2(2).unwrap(), "1.4");
        assert_eq!(format_sqrt2(5).unwrap(), "1.4142");
        assert_eq!(format_sqrt2(10).unwrap(), "1.414213562");
        assert_eq!(format_sqrt2(20).unwrap(), "1.4142135623730950488");
        assert_eq!(
            format_sqrt2(50).unwrap(),
            "1.4142135623730950488016887242096980785696718753769"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_sqrt2(0).is_err());
        assert!(format_sqrt2(MAX_DIGITS + 1).is_err());
        for algo in [Sqrt2Algo::Newton, Sqrt2Algo::Cf] {
            assert!(format_sqrt2_with_algo(0, algo).is_err());
            assert!(format_sqrt2_with_algo(MAX_DIGITS + 1, algo).is_err());
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

        for scale in [1u32, 20, 40, 80] {
            let wide = BigUint::from(2u32) * BigUint::from(10u32).pow(scale);
            assert_eq!(isqrt_newton(&wide), wide.sqrt(), "10^{scale}");
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(Sqrt2Algo::default(), Sqrt2Algo::Newton);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_sqrt2(digits).unwrap();
            for algo in [Sqrt2Algo::Newton, Sqrt2Algo::Cf] {
                assert_eq!(
                    format_sqrt2_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
