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

//! Significant digits of pi.
//!
//! [`PiAlgo::Machin`] and [`PiAlgo::BinarySplit`] both use Machin's formula,
//! π/4 = 4·arctan(1/5) − arctan(1/239). `Machin` sums each arctangent Taylor
//! series term by term. `BinarySplit` sums that same series with binary
//! splitting. [`PiAlgo::Chudnovsky`] uses the Chudnovsky series for 1/π and
//! also sums it with binary splitting.

mod resume;

pub use resume::format_pi_resumable;

use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000_000;

const GUARD_DIGITS: u32 = 20;

/// `10^GUARD_DIGITS`. Small enough to live in a `u128`, so rounding does not
/// rebuild it with `BigUint::pow` on every call.
const GUARD_SCALE: u128 = 10_u128.pow(GUARD_DIGITS);

/// Algorithm used to compute significant digits of pi.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum PiAlgo {
    /// Machin's formula, with each arctangent Taylor series summed term by term.
    Machin,
    /// Machin's formula, with each arctangent series summed by binary splitting.
    BinarySplit,
    /// Chudnovsky series for 1/π, summed by binary splitting.
    #[default]
    Chudnovsky,
}

impl std::fmt::Display for PiAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PiAlgo::Machin => "machin",
            PiAlgo::BinarySplit => "binary-split",
            PiAlgo::Chudnovsky => "chudnovsky",
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
        PiAlgo::Chudnovsky => compute_pi_chudnovsky(&one, scale),
    };
    format_scaled_pi(pi_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 1000000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_pi(pi_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(GUARD_SCALE);
    let half = BigUint::from(GUARD_SCALE / 2);
    let (mut kept, remainder) = pi_scaled.div_rem(&guard);
    if remainder >= half {
        kept += 1u32;
    }

    let mut text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: pi rounding produced an unexpected length");
    }

    if digits != 1 {
        text.insert(1, '.');
    }
    Ok(text)
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

/// Chudnovsky coefficients. Each term adds a little more than 14 decimal digits.
const CHUD_A: u128 = 13_591_409;
const CHUD_B: u128 = 545_140_134;
const CHUD_C: u32 = 640_320;

/// Scaled integer approximation of pi from the Chudnovsky series.
///
/// `426880 √10005 / π = Σ (-1)^k (6k)! (A + Bk) / ((3k)! (k!)^3 C^(3k))`.
/// Binary splitting computes that sum as `t/q`, then one division yields pi.
fn compute_pi_chudnovsky(one: &BigUint, scale: u32) -> BigUint {
    let c = BigUint::from(CHUD_C);
    let q_factor = (&c * &c * c) / 24u32;
    let split = split_chudnovsky(0, chudnovsky_terms(scale), &q_factor);
    let t = split
        .t
        .to_biguint()
        .expect("Chudnovsky sum starting at zero is positive");
    // π * 10^scale = 426880 * √(10005) * 10^scale * q / t
    let sqrt_scaled = (BigUint::from(10005u32) * one * one).sqrt();
    split.q * 426_880u32 * sqrt_scaled / t
}

fn chudnovsky_terms(scale: u32) -> u32 {
    // 14 digits per term underestimates the real 14.18, so the count is high.
    scale.div_ceil(14).max(1) + 1
}

/// Binary split of the Chudnovsky sum for k in `[start, end)`.
///
/// `t/q` is the partial sum. `q_factor` is `C^3 / 24`.
struct ChudSplit {
    p: BigUint,
    q: BigUint,
    t: BigInt,
}

fn split_chudnovsky(start: u32, end: u32, q_factor: &BigUint) -> ChudSplit {
    const LEAF: u32 = 32;
    if end - start <= LEAF {
        let mut acc = chudnovsky_term(start, q_factor);
        for k in start + 1..end {
            acc = merge_chudnovsky(acc, chudnovsky_term(k, q_factor));
        }
        return acc;
    }

    let mid = start.midpoint(end);
    let left = split_chudnovsky(start, mid, q_factor);
    let right = split_chudnovsky(mid, end, q_factor);
    merge_chudnovsky(left, right)
}

fn merge_chudnovsky(left: ChudSplit, right: ChudSplit) -> ChudSplit {
    let p = &left.p * &right.p;
    let q = &left.q * &right.q;
    // `t = left.t * right.q + left.p * right.t`. Move the factors after the products.
    let t = left.t * BigInt::from(right.q) + BigInt::from(left.p) * right.t;
    ChudSplit { p, q, t }
}

fn chudnovsky_term(k: u32, q_factor: &BigUint) -> ChudSplit {
    if k == 0 {
        return ChudSplit {
            p: BigUint::from(1u32),
            q: BigUint::from(1u32),
            t: BigInt::from(CHUD_A),
        };
    }

    let k = u128::from(k);
    let p = (6 * k - 5) * (2 * k - 1) * (6 * k - 1);
    let linear = CHUD_A + CHUD_B * k;
    let k_bi = BigUint::from(k);
    let q = &k_bi * &k_bi * &k_bi * q_factor;
    let magnitude = BigUint::from(p) * BigUint::from(linear);
    let t = if k.is_multiple_of(2) {
        BigInt::from(magnitude)
    } else {
        -BigInt::from(magnitude)
    };
    ChudSplit {
        p: BigUint::from(p),
        q,
        t,
    }
}

fn split_arctan(start: u32, end: u32, y: &BigUint) -> AtanSplit {
    const LEAF: u32 = 32;
    if end - start <= LEAF {
        let mut acc = arctan_term(start, y);
        for k in start + 1..end {
            acc = merge_arctan(acc, arctan_term(k, y));
        }
        return acc;
    }

    let mid = start.midpoint(end);
    let left = split_arctan(start, mid, y);
    let right = split_arctan(mid, end, y);
    merge_arctan(left, right)
}

fn arctan_term(k: u32, y: &BigUint) -> AtanSplit {
    let t = if k.is_multiple_of(2) {
        BigInt::from(1)
    } else {
        BigInt::from(-1)
    };
    AtanSplit {
        p: y.clone(),
        q: BigUint::from(2 * u64::from(k) + 1),
        t,
    }
}

fn merge_arctan(left: AtanSplit, right: AtanSplit) -> AtanSplit {
    let right_scale = &right.p * &right.q;
    let p = &left.p * &right.p;
    let q = &left.q * &right.q;
    // `t = left.t * (right.p * right.q) + right.t * left.q`.
    let t = left.t * BigInt::from(right_scale) + right.t * BigInt::from(left.q);
    AtanSplit { p, q, t }
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
        for algo in [PiAlgo::Machin, PiAlgo::BinarySplit, PiAlgo::Chudnovsky] {
            assert!(format_pi_with_algo(0, algo).is_err());
            assert!(format_pi_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(PiAlgo::default(), PiAlgo::Chudnovsky);
        for digits in [1, 2, 5, 10, 50, 200, 2_000] {
            let expected = format_pi(digits).unwrap();
            for algo in [PiAlgo::Machin, PiAlgo::BinarySplit, PiAlgo::Chudnovsky] {
                assert_eq!(
                    format_pi_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
