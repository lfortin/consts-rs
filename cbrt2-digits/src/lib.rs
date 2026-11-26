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

//! Significant digits of the cube root of 2.
//!
//! [`Cbrt2Algo::Newton`] iterates Newton's method for `x^3 - 2 = 0`.
//! [`Cbrt2Algo::Halley`] iterates Halley's method, which converges cubically.

mod resume;

pub use resume::format_cbrt2_resumable;

use num_bigint::BigUint;
use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 1_000_000_000;

const GUARD_DIGITS: u32 = 20;

/// `10^GUARD_DIGITS`. Small enough to live in a `u128`, so rounding does not
/// rebuild it with `BigUint::pow` on every call.
const GUARD_SCALE: u128 = 10_u128.pow(GUARD_DIGITS);

/// Algorithm used to compute significant digits of ∛2.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Cbrt2Algo {
    /// Newton-Raphson iteration for ∛2.
    Newton,
    /// Halley's method for ∛2.
    Halley,
}

#[allow(clippy::derivable_impls)]
impl Default for Cbrt2Algo {
    fn default() -> Self {
        Self::Newton
    }
}

impl std::fmt::Display for Cbrt2Algo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Cbrt2Algo::Newton => "newton",
            Cbrt2Algo::Halley => "halley",
        })
    }
}

/// Format `digits` significant digits of ∛2.
///
/// One digit is `1`. Two digits are `1.3`, because 1.259… rounds half up.
/// Larger counts include a decimal point after the leading `1`. The last digit
/// is rounded half up.
///
/// Uses [`Cbrt2Algo::default`].
pub fn format_cbrt2(digits: u32) -> Result<String, &'static str> {
    format_cbrt2_with_algo(digits, Cbrt2Algo::default())
}

/// Format `digits` significant digits of ∛2 with `algo`.
///
/// Output matches [`format_cbrt2`]: one digit is `1`, two digits are `1.3`,
/// and larger counts include a decimal point after the leading `1`. The last
/// digit is rounded half up.
pub fn format_cbrt2_with_algo(digits: u32, algo: Cbrt2Algo) -> Result<String, &'static str> {
    let scale = scale_for_digits(digits)?;
    let one = BigUint::from(10u32).pow(scale);
    let cbrt2_scaled = match algo {
        Cbrt2Algo::Newton => cbrt2_newton(&one),
        Cbrt2Algo::Halley => cbrt2_halley(&one),
    };
    format_scaled_cbrt2(cbrt2_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 1000000000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_cbrt2(cbrt2_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(GUARD_SCALE);
    let half = BigUint::from(GUARD_SCALE / 2);
    let (mut kept, remainder) = cbrt2_scaled.div_rem(&guard);
    if remainder >= half {
        kept += 1u32;
    }

    let mut text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: cube root of 2 rounding produced an unexpected length");
    }

    if digits != 1 {
        text.insert(1, '.');
    }
    Ok(text)
}

/// `floor(∛2 * 10^scale)` by Newton-Raphson.
fn cbrt2_newton(one: &BigUint) -> BigUint {
    // floor(∛2 * 10^scale) = icbrt(2 * 10^(3*scale))
    let radicand = BigUint::from(2u32) * one * one * one;
    icbrt_newton(&radicand)
}

/// Floor of the cube root of `n`, by precision-doubling Newton steps.
fn icbrt_newton(n: &BigUint) -> BigUint {
    icbrt_doubling(n, false)
}

/// `floor(∛2 * 10^scale)` by Halley's method.
fn cbrt2_halley(one: &BigUint) -> BigUint {
    let radicand = BigUint::from(2u32) * one * one * one;
    icbrt_halley(&radicand)
}

/// Floor of the cube root of `n`, by precision-doubling Halley steps.
fn icbrt_halley(n: &BigUint) -> BigUint {
    icbrt_doubling(n, true)
}

/// Floor of the cube root of `n`.
///
/// Values that fit in `u128` use [`icbrt_u128`]. Larger values take the cube
/// root of the high third, shift it back, and apply one Newton or Halley
/// correction, then walk at most one step to the exact floor.
fn icbrt_doubling(n: &BigUint, halley: bool) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }
    if let Some(small) = n.to_u128() {
        return BigUint::from(icbrt_u128(small));
    }

    let root_bits = n.bits().div_ceil(3);
    let m = (root_bits / 2).max(1).min(root_bits - 1);
    let s = icbrt_doubling(&(n >> (3 * m)), halley);
    let mut x = s << m;
    if x.is_zero() {
        x = BigUint::from(1u32);
    }
    x = if halley {
        halley_step(&x, n)
    } else {
        newton_step(&x, n)
    };
    finish_icbrt(x, n)
}

/// The half-precision seed leaves an O(1) error after one correction.
/// Another Newton step, then a single ±1, lands on the floor.
fn finish_icbrt(mut x: BigUint, n: &BigUint) -> BigUint {
    if x.is_zero() {
        return BigUint::from(u32::from(!n.is_zero()));
    }
    if cube_gt(&x, n) {
        let y = newton_step(&x, n);
        if !y.is_zero() && y < x {
            x = y;
        }
    } else {
        let next = &x + 1u32;
        if !cube_gt(&next, n) {
            let y = newton_step(&x, n);
            x = if y > x { y } else { next };
        }
    }
    if cube_gt(&x, n) {
        x -= 1u32;
    } else {
        let next = &x + 1u32;
        if !cube_gt(&next, n) {
            x = next;
        }
    }
    x
}

/// `x_{k+1} = (2 x_k + n / x_k^2) / 3`.
fn newton_step(x: &BigUint, n: &BigUint) -> BigUint {
    let square = x * x;
    ((x << 1) + n / square) / 3u32
}

/// `x_{k+1} = x_k (x_k^3 + 2 n) / (2 x_k^3 + n)`.
fn halley_step(x: &BigUint, n: &BigUint) -> BigUint {
    let cube = x * x * x;
    let numerator = x * (&cube + (n << 1));
    let denominator = (&cube << 1) + n;
    numerator / denominator
}

/// `x^3 > n`.
fn cube_gt(x: &BigUint, n: &BigUint) -> bool {
    !x.is_zero() && n / x / x < *x
}

fn icbrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let bits = 128 - n.leading_zeros();
    let coarse = 1u128 << bits.div_ceil(3);
    let approx = (n as f64).cbrt();
    let mut x = if approx.is_finite() && approx >= 1.0 {
        (approx as u128).saturating_add(2)
    } else {
        coarse
    };
    // `coarse` is an upper bound. A low `f64` estimate would make Newton stop early.
    if x < 2 || n / x / x >= x {
        x = coarse;
    }
    loop {
        let y = ((x << 1) + n / (x * x)) / 3;
        if y >= x {
            break;
        }
        x = y;
    }
    while x > 1 && n / x / x < x {
        x -= 1;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prefixes() {
        assert_eq!(format_cbrt2(1).unwrap(), "1");
        assert_eq!(format_cbrt2(2).unwrap(), "1.3");
        assert_eq!(format_cbrt2(5).unwrap(), "1.2599");
        assert_eq!(format_cbrt2(10).unwrap(), "1.259921050");
        assert_eq!(format_cbrt2(20).unwrap(), "1.2599210498948731648");
        assert_eq!(
            format_cbrt2(50).unwrap(),
            "1.2599210498948731647672106072782283505702514647015"
        );
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(format_cbrt2(0).is_err());
        assert!(format_cbrt2(MAX_DIGITS + 1).is_err());
        for algo in [Cbrt2Algo::Newton, Cbrt2Algo::Halley] {
            assert!(format_cbrt2_with_algo(0, algo).is_err());
            assert!(format_cbrt2_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn cube_roots_match_builtin() {
        for value in [0u32, 1, 2, 7, 8, 9, 26, 27, 1_000] {
            let n = BigUint::from(value);
            assert_eq!(icbrt_newton(&n), n.cbrt());
            assert_eq!(icbrt_halley(&n), n.cbrt());
        }
        assert_eq!(icbrt_u128(0), 0);
        assert_eq!(icbrt_u128(1), 1);
        assert_eq!(icbrt_u128(8), 2);
        let root = icbrt_u128(u128::MAX);
        assert!(u128::MAX / root / root >= root);
        assert!(u128::MAX / (root + 1) / (root + 1) < root + 1);

        let perfect = BigUint::from(1u32) << 80;
        let cube = &perfect * &perfect * &perfect;
        assert_eq!(icbrt_newton(&cube), perfect);
        assert_eq!(icbrt_halley(&cube), perfect);
        let below = &cube - 1u32;
        let one_less = &perfect - 1u32;
        assert_eq!(icbrt_newton(&below), one_less);
        assert_eq!(icbrt_halley(&below), one_less);

        for scale in [1u32, 20, 40, 80] {
            let wide = BigUint::from(2u32) * BigUint::from(10u32).pow(scale);
            assert_eq!(icbrt_newton(&wide), wide.cbrt(), "newton 10^{scale}");
            assert_eq!(icbrt_halley(&wide), wide.cbrt(), "halley 10^{scale}");
        }

        let root = (BigUint::from(1u32) << 1_500) + 3u32;
        let cube = &root * &root * &root;
        let below = &cube - 1u32;
        let one_less = &root - 1u32;
        assert_eq!(icbrt_newton(&cube), root);
        assert_eq!(icbrt_halley(&cube), root);
        assert_eq!(icbrt_newton(&below), one_less);
        assert_eq!(icbrt_halley(&below), one_less);
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(Cbrt2Algo::default(), Cbrt2Algo::Newton);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_cbrt2(digits).unwrap();
            for algo in [Cbrt2Algo::Newton, Cbrt2Algo::Halley] {
                assert_eq!(
                    format_cbrt2_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
