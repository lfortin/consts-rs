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

//! Significant digits of pi, computed with Machin's formula.
//!
//! [`PiAlgo::Machin`] sums the arctangent Taylor series directly.
//! [`PiAlgo::BinarySplit`] evaluates the same series with binary splitting.

use num_bigint::{BigInt, BigUint};
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 100_000;

const GUARD_DIGITS: u32 = 20;

/// Algorithm used to compute significant digits of pi.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum PiAlgo {
    /// Machin's formula using alternating Taylor series on scaled integers.
    Machin,
    /// Binary splitting over Machin's arctangent series.
    BinarySplit,
}

#[allow(clippy::derivable_impls)]
impl Default for PiAlgo {
    fn default() -> Self {
        Self::Machin
    }
}

impl std::fmt::Display for PiAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PiAlgo::Machin => "machin",
            PiAlgo::BinarySplit => "binary-split",
        })
    }
}

/// Format `digits` significant digits of pi.
///
/// One digit is `3`. Larger counts include a decimal point after the leading
/// `3`. The last digit is rounded half up.
///
/// Uses [`PiAlgo::default`].
pub fn format_pi(digits: u32) -> Result<String, &'static str> {
    format_pi_with_algo(digits, PiAlgo::default())
}

/// Format `digits` significant digits of pi with `algo`.
///
/// Output matches [`format_pi`]: one digit is `3`, larger counts include a
/// decimal point after the leading `3`, and the last digit is rounded half up.
pub fn format_pi_with_algo(digits: u32, algo: PiAlgo) -> Result<String, &'static str> {
    let scale = scale_for_digits(digits)?;
    let one = BigUint::from(10u32).pow(scale);
    let pi_scaled = match algo {
        PiAlgo::Machin => pi_machin(&one),
        PiAlgo::BinarySplit => pi_binary_split(&one, scale),
    };
    format_scaled_pi(pi_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 100000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_pi(pi_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &pi_scaled / &guard;
    if &pi_scaled % &guard >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: pi rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

fn pi_machin(one: &BigUint) -> BigUint {
    let atan5 = arctan_reciprocal(5, one);
    let atan239 = arctan_reciprocal(239, one);
    // pi/4 = 4*arctan(1/5) - arctan(1/239)
    (atan5 * 4u32 - atan239) * 4u32
}

/// Integer approximation of arctan(1/x) * `one`.
fn arctan_reciprocal(x: u32, one: &BigUint) -> BigUint {
    let x_bi = BigUint::from(x);
    let x_squared = &x_bi * &x_bi;
    let mut term = one / &x_bi;
    let mut total = term.clone();
    let mut subtract = true;
    let mut k = 3u32;

    loop {
        term /= &x_squared;
        if term.is_zero() {
            break;
        }
        let delta = &term / k;
        if delta.is_zero() {
            break;
        }
        if subtract {
            total -= delta;
        } else {
            total += delta;
        }
        subtract = !subtract;
        k += 2;
    }

    total
}

fn pi_binary_split(one: &BigUint, scale: u32) -> BigUint {
    let atan5 = arctan_binary_split(5, one, scale);
    let atan239 = arctan_binary_split(239, one, scale);
    // pi/4 = 4*arctan(1/5) - arctan(1/239)
    (atan5 * 4u32 - atan239) * 4u32
}

/// Integer approximation of arctan(1/x) * `one` via binary splitting.
fn arctan_binary_split(x: u32, one: &BigUint, scale: u32) -> BigUint {
    let terms = terms_needed(x, scale);
    let x_int = BigUint::from(x);
    let y = &x_int * &x_int;
    let split = split_arctan(0, terms, &y);
    let t = split
        .t
        .to_biguint()
        .expect("arctan series starting at zero is positive");
    // Partial sum = t / (q * x^(2*terms-1)) = (t * x) / (q * y^terms).
    let numerator = t * one * &x_int;
    let denominator = split.q * split.p;
    numerator / denominator
}

/// Number of arctan terms so the omitted tail is below one unit at `scale`.
fn terms_needed(x: u32, scale: u32) -> u32 {
    let log10_x = f64::from(x).log10();
    let exponent = (f64::from(scale) / log10_x).ceil() + 2.0;
    let terms = (exponent / 2.0).ceil();
    if !terms.is_finite() || terms < 1.0 {
        return 1;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (terms as u32).max(1)
    }
}

/// Binary split of Σ (-1)^k / ((2k+1) y^k) for k in `[start, end)`.
///
/// `p` is y^(end-start) and `q` is the product of (2k+1). `t/q` equals
/// Σ (-1)^k * y^(end-1-k) / (2k+1).
struct AtanSplit {
    p: BigUint,
    q: BigUint,
    t: BigInt,
}

fn split_arctan(start: u32, end: u32, y: &BigUint) -> AtanSplit {
    if end - start == 1 {
        let t = if start.is_multiple_of(2) {
            BigInt::from(1)
        } else {
            BigInt::from(-1)
        };
        return AtanSplit {
            p: y.clone(),
            q: BigUint::from(2 * u64::from(start) + 1),
            t,
        };
    }

    let mid = start.midpoint(end);
    let left = split_arctan(start, mid, y);
    let right = split_arctan(mid, end, y);
    let right_scale = &right.p * &right.q;
    let t = &left.t * BigInt::from(right_scale) + &right.t * BigInt::from(left.q.clone());
    AtanSplit {
        p: left.p * right.p,
        q: left.q * right.q,
        t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_pi(1).unwrap(), "3");
        assert_eq!(format_pi(2).unwrap(), "3.1");
        assert_eq!(format_pi(5).unwrap(), "3.1416");
        assert_eq!(format_pi(10).unwrap(), "3.141592654");
        assert_eq!(
            format_pi(50).unwrap(),
            "3.1415926535897932384626433832795028841971693993751"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_pi(0).is_err());
        assert!(format_pi(MAX_DIGITS + 1).is_err());
        for algo in [PiAlgo::Machin, PiAlgo::BinarySplit] {
            assert!(format_pi_with_algo(0, algo).is_err());
            assert!(format_pi_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(PiAlgo::default(), PiAlgo::Machin);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_pi(digits).unwrap();
            for algo in [PiAlgo::Machin, PiAlgo::BinarySplit] {
                assert_eq!(
                    format_pi_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
