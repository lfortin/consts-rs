// Copyright (C) 2026 pi-digits authors
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
// You should have received copies of the GNU Lesser General Public License
// and the GNU General Public License along with this program. If not, see
// <https://www.gnu.org/licenses/>.

//! Significant digits of pi, computed with Machin's formula.

use num_bigint::BigUint;
use num_traits::Zero;

/// Largest digit count the CLI and library will compute.
pub const MAX_DIGITS: u32 = 100_000;

const GUARD_DIGITS: u32 = 20;

/// Format `digits` significant digits of pi.
///
/// One digit is `3`. Larger counts include a decimal point after the leading
/// `3`. The last digit is rounded half up.
pub fn format_pi(digits: u32) -> Result<String, &'static str> {
    if !(1..=MAX_DIGITS).contains(&digits) {
        return Err("digit count must be from 1 to 100000");
    }

    let scale = digits - 1 + GUARD_DIGITS;
    let one = BigUint::from(10u32).pow(scale);
    let atan5 = arctan_reciprocal(5, &one);
    let atan239 = arctan_reciprocal(239, &one);
    // pi/4 = 4*arctan(1/5) - arctan(1/239)
    let pi_scaled = (atan5 * 4u32 - atan239) * 4u32;

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
    }
}
