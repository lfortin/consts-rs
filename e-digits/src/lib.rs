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

//! Significant digits of Euler's number.
//!
//! [`EAlgo::Taylor`] and [`EAlgo::BinarySplit`] both use the series e = Σ 1/n!.
//! `Taylor` divides by each integer in turn. `BinarySplit` multiplies those
//! factors in a tree and divides once. [`EAlgo::Cf`] uses the continued
//! fraction e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …].

use num_bigint::BigUint;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000;

const GUARD_DIGITS: u32 = 20;

/// Algorithm used to compute significant digits of e.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum EAlgo {
    /// Series e = Σ 1/n!, divided by each integer in turn.
    Taylor,
    /// Series e = Σ 1/n!, summed by binary splitting.
    BinarySplit,
    /// Continued fraction e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …].
    Cf,
}

#[allow(clippy::derivable_impls)]
impl Default for EAlgo {
    fn default() -> Self {
        Self::Taylor
    }
}

impl std::fmt::Display for EAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EAlgo::Taylor => "taylor",
            EAlgo::BinarySplit => "binary-split",
            EAlgo::Cf => "cf",
        })
    }
}

/// Format `digits` significant digits of e.
///
/// One digit is `3`, because 2.718… rounds half up. Larger counts include a
/// decimal point after the leading `2`. The last digit is rounded half up.
///
/// Uses [`EAlgo::default`].
pub fn format_e(digits: u32) -> Result<String, &'static str> {
    format_e_with_algo(digits, EAlgo::default())
}

/// Format `digits` significant digits of e with `algo`.
///
/// Output matches [`format_e`]: one digit is `3`, because 2.718… rounds half
/// up. Larger counts include a decimal point after the leading `2`. The last
/// digit is rounded half up.
pub fn format_e_with_algo(digits: u32, algo: EAlgo) -> Result<String, &'static str> {
    let scale = scale_for_digits(digits)?;
    let one = BigUint::from(10u32).pow(scale);
    let e_scaled = match algo {
        EAlgo::Taylor => e_taylor(&one),
        EAlgo::BinarySplit => e_binary_split(&one, scale),
        EAlgo::Cf => e_continued_fraction(&one, scale),
    };
    format_scaled_e(e_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 1000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_e(e_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &e_scaled / &guard;
    if &e_scaled % &guard >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: Euler's number rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

fn e_taylor(one: &BigUint) -> BigUint {
    let mut term = one.clone();
    let mut total = term.clone();
    let mut k = 1u32;
    loop {
        term /= k;
        if term.is_zero() {
            break;
        }
        total += &term;
        k += 1;
    }
    total
}

fn e_binary_split(one: &BigUint, scale: u32) -> BigUint {
    let split = split_factorial(0, term_count(scale));
    // Partial sum of Σ 1/k! is t/q. `p` is the product of numerator factors (all 1).
    (split.t * one) / split.q
}

/// `floor(e * 10^scale)` from continued-fraction convergents.
///
/// e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …], so `a_0 = 2` and, for m ≥ 1,
/// `a_{3m-2} = 1`, `a_{3m-1} = 2m`, `a_{3m} = 1`. The convergents obey
/// `p_k = a_k p_{k-1} + p_{k-2}` and `q_k = a_k q_{k-1} + q_{k-2}`,
/// starting from `p_{-2} = 0`, `p_{-1} = 1`, `q_{-2} = 1`, `q_{-1} = 0`.
fn e_continued_fraction(one: &BigUint, scale: u32) -> BigUint {
    let mut p_prev = BigUint::from(0u32);
    let mut p = BigUint::from(1u32);
    let mut q_prev = BigUint::from(1u32);
    let mut q = BigUint::from(0u32);
    let needed_bits = convergent_bits(scale);
    let mut k = 0u32;
    loop {
        let a = BigUint::from(partial_quotient(k));
        let p_next = &a * &p + &p_prev;
        let q_next = &a * &q + &q_prev;
        p_prev = p;
        q_prev = q;
        p = p_next;
        q = q_next;
        k += 1;
        if q.bits() >= needed_bits {
            break;
        }
    }
    (&p * one) / q
}

/// `a_0 = 2`; later terms are 1, except `a_{3m-1} = 2m`.
fn partial_quotient(k: u32) -> u64 {
    if k == 0 {
        return 2;
    }
    if k % 3 == 2 {
        let m = u64::from((k + 1) / 3);
        2 * m
    } else {
        1
    }
}

/// Bit length of a convergent denominator past which the error is far below
/// one unit at `scale`.
fn convergent_bits(scale: u32) -> u64 {
    // q > 10^((scale + 8) / 2). log2(10) / 2 < 3322/2000.
    (u64::from(scale) + 8) * 3322 / 2000 + 2
}

/// Number of factorial terms so the omitted tail is below one unit at `scale`.
fn term_count(scale: u32) -> u32 {
    let mut log10_factorial = 0.0;
    let mut n = 0u32;
    let target = f64::from(scale) + 2.0;
    while log10_factorial < target {
        n += 1;
        log10_factorial += f64::from(n).log10();
    }
    // Terms k = 0..=n.
    n + 1
}

/// Binary split of Σ 1/k! for k in `[start, end)`.
///
/// `q` is Π q(k) with q(0) = 1 and q(k) = k otherwise, and `p` is the product
/// of the numerator factors. `t/q` equals Σ_{k=start}^{end-1} start_scale / k!,
/// where the scale is the product of q over the interval.
struct FactorialSplit {
    p: BigUint,
    q: BigUint,
    t: BigUint,
}

fn split_factorial(start: u32, end: u32) -> FactorialSplit {
    if end - start == 1 {
        let q = if start == 0 {
            BigUint::from(1u32)
        } else {
            BigUint::from(start)
        };
        return FactorialSplit {
            p: BigUint::from(1u32),
            q,
            t: BigUint::from(1u32),
        };
    }

    let mid = start.midpoint(end);
    let left = split_factorial(start, mid);
    let right = split_factorial(mid, end);
    FactorialSplit {
        p: &left.p * &right.p,
        q: &left.q * &right.q,
        t: &left.t * &right.q + &right.t * &left.p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_e(1).unwrap(), "3");
        assert_eq!(format_e(2).unwrap(), "2.7");
        assert_eq!(format_e(5).unwrap(), "2.7183");
        assert_eq!(format_e(10).unwrap(), "2.718281828");
        assert_eq!(format_e(20).unwrap(), "2.7182818284590452354");
        assert_eq!(
            format_e(50).unwrap(),
            "2.7182818284590452353602874713526624977572470937000"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_e(0).is_err());
        assert!(format_e(MAX_DIGITS + 1).is_err());
        for algo in [EAlgo::Taylor, EAlgo::BinarySplit, EAlgo::Cf] {
            assert!(format_e_with_algo(0, algo).is_err());
            assert!(format_e_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(EAlgo::default(), EAlgo::Taylor);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_e(digits).unwrap();
            for algo in [EAlgo::Taylor, EAlgo::BinarySplit, EAlgo::Cf] {
                assert_eq!(
                    format_e_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
