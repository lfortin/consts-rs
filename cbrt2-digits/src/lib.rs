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

use num_bigint::BigUint;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 100_000;

const GUARD_DIGITS: u32 = 20;

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
    let cbrt2_scaled = match algo {
        Cbrt2Algo::Newton => cbrt2_newton(scale),
        Cbrt2Algo::Halley => cbrt2_halley(scale),
    };
    format_scaled_cbrt2(cbrt2_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 100000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_cbrt2(cbrt2_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &cbrt2_scaled / &guard;
    if &cbrt2_scaled % &guard >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: cube root of 2 rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

/// `floor(∛2 * 10^scale)` by Newton-Raphson.
fn cbrt2_newton(scale: u32) -> BigUint {
    // floor(∛2 * 10^scale) = icbrt(2 * 10^(3*scale))
    let radicand = BigUint::from(2u32) * BigUint::from(10u32).pow(scale * 3);
    icbrt_newton(&radicand)
}

/// Floor of the cube root of `n`, by `x_{k+1} = (2 x_k + n / x_k^2) / 3`.
fn icbrt_newton(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = BigUint::from(1u32) << n.bits().div_ceil(3);
    loop {
        let y = ((&x << 1) + n / (&x * &x)) / 3u32;
        if y >= x {
            break;
        }
        x = y;
    }
    while &x * &x * &x > *n {
        x -= 1u32;
    }
    x
}

/// `floor(∛2 * 10^scale)` by Halley's method.
fn cbrt2_halley(scale: u32) -> BigUint {
    let radicand = BigUint::from(2u32) * BigUint::from(10u32).pow(scale * 3);
    icbrt_halley(&radicand)
}

/// Floor of the cube root of `n`, by `x_{k+1} = x_k (x_k^3 + 2 n) / (2 x_k^3 + n)`.
fn icbrt_halley(n: &BigUint) -> BigUint {
    if n.is_zero() {
        return BigUint::zero();
    }

    let mut x = BigUint::from(1u32) << n.bits().div_ceil(3);
    loop {
        let cube = &x * &x * &x;
        let numerator = &x * (&cube + (n << 1));
        let denominator = (&cube << 1) + n;
        let y = numerator / denominator;
        if y >= x {
            break;
        }
        x = y;
    }
    while &x * &x * &x > *n {
        x -= 1u32;
    }
    loop {
        let next = &x + 1u32;
        if &next * &next * &next > *n {
            break;
        }
        x = next;
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
