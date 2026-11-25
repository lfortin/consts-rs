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
use num_traits::{Euclid, Zero};

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000_000;

const GUARD_DIGITS: u32 = 20;

/// `10^GUARD_DIGITS`. Small enough to live in a `u128`, so rounding does not
/// rebuild it with `BigUint::pow` on every call.
const GUARD_SCALE: u128 = 10_u128.pow(GUARD_DIGITS);

/// Algorithm used to compute significant digits of e.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum EAlgo {
    /// Series e = Σ 1/n!, divided by each integer in turn.
    Taylor,
    /// Series e = Σ 1/n!, summed by binary splitting.
    #[default]
    BinarySplit,
    /// Continued fraction e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …].
    Cf,
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
        return Err("digit count must be from 1 to 1000000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_e(e_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(GUARD_SCALE);
    let half = BigUint::from(GUARD_SCALE / 2);
    let (mut kept, remainder) = e_scaled.div_rem_euclid(&guard);
    if remainder >= half {
        kept += 1u32;
    }

    let mut text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: Euler's number rounding produced an unexpected length");
    }

    if digits != 1 {
        text.insert(1, '.');
    }
    Ok(text)
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
    // Partial sum of Σ 1/k! is t/q. Numerator factors are all 1.
    (split.t * one) / split.q
}

/// `floor(e * 10^scale)` from continued-fraction convergents.
///
/// e = [2; 1, 2, 1, 1, 4, 1, 1, 6, …], so `a_0 = 2` and, for m ≥ 1,
/// `a_{3m-2} = 1`, `a_{3m-1} = 2m`, `a_{3m} = 1`. The product of the transfer
/// matrices `[[a_k, 1], [1, 0]]` is computed by binary splitting. Most partial
/// quotients are 1, so those factors stay additions.
fn e_continued_fraction(one: &BigUint, scale: u32) -> BigUint {
    let needed_bits = convergent_bits(scale);
    let mut terms = 4u32;
    let mat = loop {
        let mat = split_e_cf(0, terms);
        if mat.q.bits() >= needed_bits {
            break mat;
        }
        if terms > needed_bits as u32 {
            break extend_e_cf(mat, terms, needed_bits);
        }
        terms = terms.saturating_mul(2);
    };
    (mat.p * one) / mat.q
}

/// Product of transfer matrices for partial quotients `k` in `[start, end)`.
///
/// The first column is the convergent numerator and denominator when `start`
/// is 0.
struct CfMat {
    p: BigUint,
    r: BigUint,
    q: BigUint,
    s: BigUint,
}

fn split_e_cf(start: u32, end: u32) -> CfMat {
    const LEAF: u32 = 32;
    if end - start <= LEAF {
        let mut mat = CfMat {
            p: BigUint::from(1u32),
            r: BigUint::from(0u32),
            q: BigUint::from(0u32),
            s: BigUint::from(1u32),
        };
        for k in start..end {
            apply_quotient(&mut mat, partial_quotient(k));
        }
        return mat;
    }

    let mid = start.midpoint(end);
    let left = split_e_cf(start, mid);
    let right = split_e_cf(mid, end);
    // [[p, r], [q, s]] = left * right.
    CfMat {
        p: &left.p * &right.p + &left.r * &right.q,
        r: &left.p * &right.r + &left.r * &right.s,
        q: &left.q * &right.p + &left.s * &right.q,
        s: &left.q * &right.r + &left.s * &right.s,
    }
}

/// `P = P * [[a, 1], [1, 0]]`.
fn apply_quotient(mat: &mut CfMat, a: u64) {
    let new_p = if a == 1 {
        &mat.p + &mat.r
    } else {
        &mat.p * a + &mat.r
    };
    let new_q = if a == 1 {
        &mat.q + &mat.s
    } else {
        &mat.q * a + &mat.s
    };
    mat.r = std::mem::replace(&mut mat.p, new_p);
    mat.s = std::mem::replace(&mut mat.q, new_q);
}

fn extend_e_cf(mut mat: CfMat, mut k: u32, needed_bits: u64) -> CfMat {
    while mat.q.bits() < needed_bits {
        apply_quotient(&mut mat, partial_quotient(k));
        k += 1;
    }
    mat
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
///
/// Stirling's series without the `1/(12n)` term is strictly below `log10(n!)`.
/// The smallest `n` with that estimate at least `scale + 2` therefore still
/// has `n! > 10^(scale+2)`, and the sum through `k = n` leaves a tail under
/// `10^-2` at this scale.
fn term_count(scale: u32) -> u32 {
    let target = f64::from(scale) + 2.0;
    // For n >= 32, n*log10(n) - n*log10(e) already exceeds `scale + 2` once
    // n >= 8*scale + 32. That is a hard upper bound, not the term count.
    let mut hi = scale.saturating_mul(8).saturating_add(32);
    if stirling_log10_factorial(hi) < target {
        return term_count_linear(scale);
    }
    let mut lo = 1u32;
    while lo < hi {
        let mid = lo.midpoint(hi);
        if stirling_log10_factorial(mid) < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    while stirling_log10_factorial(lo) < target {
        lo = lo.saturating_add(1);
    }
    // Terms k = 0..=lo.
    lo + 1
}

/// `log10(sqrt(2πn) (n/e)^n)`, which is less than `log10(n!)` for every n >= 1.
fn stirling_log10_factorial(n: u32) -> f64 {
    let n = f64::from(n);
    (n + 0.5).mul_add(n.log10(), -n * std::f64::consts::LOG10_E)
        + 0.5 * (2.0 * std::f64::consts::PI).log10()
}

fn term_count_linear(scale: u32) -> u32 {
    let mut log10_factorial = 0.0;
    let mut n = 0u32;
    let target = f64::from(scale) + 2.0;
    while log10_factorial < target {
        n += 1;
        log10_factorial += f64::from(n).log10();
    }
    n + 1
}

/// Binary split of Σ 1/k! for k in `[start, end)`.
///
/// `q` is the product of the local factors (`1` when k is 0, otherwise k).
/// Every numerator factor is 1, so that product is not stored. The merge is
/// `t = left.t * right.q + right.t` and `q = left.q * right.q`. For an
/// interval that starts at 0, `t/q` is the partial sum.
struct FactorialSplit {
    q: BigUint,
    t: BigUint,
}

fn split_factorial(start: u32, end: u32) -> FactorialSplit {
    const LEAF: u32 = 32;
    if end - start <= LEAF {
        let mut q = BigUint::from(1u32);
        let mut t = BigUint::from(0u32);
        for k in start..end {
            let factor = if k == 0 {
                BigUint::from(1u32)
            } else {
                BigUint::from(k)
            };
            t = t * &factor + 1u32;
            q *= &factor;
        }
        return FactorialSplit { q, t };
    }

    let mid = start.midpoint(end);
    let left = split_factorial(start, mid);
    let right = split_factorial(mid, end);
    let t = left.t * &right.q + right.t;
    FactorialSplit {
        q: left.q * right.q,
        t,
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
    fn term_count_covers_the_linear_bound() {
        for scale in [1, 2, 20, 100, 1_000, 10_000] {
            let fast = term_count(scale);
            let linear = term_count_linear(scale);
            assert!(
                fast >= linear,
                "scale {scale}: estimate {fast} is below the linear bound {linear}"
            );
            assert!(
                fast - linear <= 2,
                "scale {scale}: estimate {fast} is far above the linear bound {linear}"
            );
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(EAlgo::default(), EAlgo::BinarySplit);
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
