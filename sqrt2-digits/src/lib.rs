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
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000;

const GUARD_DIGITS: u32 = 20;

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
    let sqrt2_scaled = match algo {
        Sqrt2Algo::Newton => sqrt2_newton(scale),
        Sqrt2Algo::Cf => sqrt2_cf(scale),
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
    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &sqrt2_scaled / &guard;
    if &sqrt2_scaled % &guard >= half {
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
fn sqrt2_newton(scale: u32) -> BigUint {
    // floor(√2 * 10^scale) = isqrt(2 * 10^(2*scale))
    let radicand = BigUint::from(2u32) * BigUint::from(10u32).pow(scale * 2);
    isqrt_newton(&radicand)
}

/// Floor of the square root of `n`, by `x_{k+1} = (x_k + n / x_k) / 2`.
fn isqrt_newton(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = BigUint::from(1u32) << n.bits().div_ceil(2);
    loop {
        let y = (&x + n / &x) >> 1;
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
fn sqrt2_cf(scale: u32) -> BigUint {
    let mut p_prev = BigUint::from(1u32);
    let mut p = BigUint::from(1u32);
    let mut q_prev = BigUint::from(0u32);
    let mut q = BigUint::from(1u32);
    let needed_bits = convergent_bits(scale);
    while q.bits() < needed_bits {
        let p_next = (&p * 2u32) + &p_prev;
        let q_next = (&q * 2u32) + &q_prev;
        p_prev = p;
        q_prev = q;
        p = p_next;
        q = q_next;
    }
    let one = BigUint::from(10u32).pow(scale);
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
