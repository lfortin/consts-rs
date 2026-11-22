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
//! [`GoldenAlgo::Newton`] finds √5 with Newton-Raphson. [`GoldenAlgo::Isqrt`]
//! uses integer square root. Both then evaluate (10^scale + √5 × 10^scale) / 2.

use num_bigint::BigUint;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 100_000;

const GUARD_DIGITS: u32 = 20;

/// Algorithm used to compute significant digits of the golden ratio.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum GoldenAlgo {
    /// Newton-Raphson square root of 5, then (1 + √5) / 2.
    Newton,
    /// Integer square root of 5 × 10^(2·scale), then (1 + √5) / 2.
    Isqrt,
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
    let sqrt5_scaled = match algo {
        GoldenAlgo::Newton => sqrt5_newton(scale),
        GoldenAlgo::Isqrt => sqrt5_isqrt(scale),
    };
    // φ * 10^scale = (10^scale + √5 * 10^scale) / 2
    let phi_scaled = (&one + sqrt5_scaled) / 2u32;
    format_scaled_golden(phi_scaled, digits)
}

fn scale_for_digits(digits: u32) -> Result<u32, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 100000");
    }
    Ok(digits - 1 + GUARD_DIGITS)
}

fn format_scaled_golden(phi_scaled: BigUint, digits: u32) -> Result<String, &'static str> {
    let guard = BigUint::from(10u32).pow(GUARD_DIGITS);
    let half = &guard / 2u32;
    let mut kept = &phi_scaled / &guard;
    if &phi_scaled % &guard >= half {
        kept += 1u32;
    }

    let text = kept.to_string();
    if text.len() != digits as usize {
        return Err("internal error: golden ratio rounding produced an unexpected length");
    }

    if digits == 1 {
        Ok(text)
    } else {
        Ok(format!("{}.{}", &text[..1], &text[1..]))
    }
}

/// `floor(√5 * 10^scale)` by Newton-Raphson.
fn sqrt5_newton(scale: u32) -> BigUint {
    let radicand = BigUint::from(5u32) * BigUint::from(10u32).pow(scale * 2);
    isqrt_newton(&radicand)
}

/// `floor(√5 * 10^scale)` by integer square root.
fn sqrt5_isqrt(scale: u32) -> BigUint {
    let radicand = BigUint::from(5u32) * BigUint::from(10u32).pow(scale * 2);
    radicand.sqrt()
}

/// Floor of the square root of `n`, by the Newton (Heron) iteration.
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
        for algo in [GoldenAlgo::Newton, GoldenAlgo::Isqrt] {
            assert!(format_golden_with_algo(0, algo).is_err());
            assert!(format_golden_with_algo(MAX_DIGITS + 1, algo).is_err());
        }
    }

    #[test]
    fn algorithms_agree() {
        assert_eq!(GoldenAlgo::default(), GoldenAlgo::Newton);
        for digits in [1, 2, 5, 10, 50, 200] {
            let expected = format_golden(digits).unwrap();
            for algo in [GoldenAlgo::Newton, GoldenAlgo::Isqrt] {
                assert_eq!(
                    format_golden_with_algo(digits, algo).unwrap(),
                    expected,
                    "{algo:?} disagreed at {digits} digits"
                );
            }
        }
    }
}
